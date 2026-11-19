//! The local verification gate (design §6.1 invariant 1, §9.2). Pure: bytes and state in, a
//! decision out; the daemon does the I/O. In order:
//!
//! 1. decode; only `PROMPT` / `CONTROL` are signals;
//! 2. the kid is in the **local** allowlist and not paused;
//! 3. `sw_protocol::gate::verify` with this Device as the verifier (signature over the exact
//!    body bytes, signer role, `aud == own CID`, `iat`/`exp`, `(kid, nonce)` unseen, `seq`
//!    increasing) — the same code the node and the P2P path run;
//! 4. unseal with this Device's ML-KEM key; the inner `aud` is this Device's CID;
//! 5. the per-kid rate and the pending bound;
//! 6. the target Task has an agent attached here;
//! 7. approval mode: a Controller trusted less than the configured hours ago needs a person
//!    to approve each signal in the terminal (`sw-mcp gate approve`).
//!
//! Every refusal from a known kid is counted; enough of them inside a minute pause the kid.

mod outcome;
mod rate;

pub use outcome::{AckTo, Admitted, Outcome, Reason, Refused, ReplayEntry, SignalKind};
pub use rate::{RateWindows, WINDOW_MS};

use sw_crypto::KemSecretKey;
use sw_model::Cid;
use sw_protocol::gate::{Verifier, verify};
use sw_protocol::payload::{ControlInner, ControlOp, Payload, PromptInner};
use sw_protocol::ports::IdentityPort;
use sw_protocol::replay::ReplayGuard;
use sw_protocol::signed::SwMessage;

use crate::config::Config;
use crate::trust::TrustList;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateLimits {
    pub per_kid_per_min: u32,
    pub pending_max: u32,
    pub refusals_before_pause: u32,
    pub approval_window_ms: u64,
}

impl GateLimits {
    pub fn from_config(c: &Config) -> Self {
        Self {
            per_kid_per_min: c.rate.per_kid_per_min,
            pending_max: c.rate.pending_max,
            refusals_before_pause: c.rate.refusals_per_min_before_pause,
            approval_window_ms: c
                .approval_mode_new_controller_hours
                .saturating_mul(3_600_000),
        }
    }
}

/// Whether an agent here can take a signal: `(task_id, agent_id)` for a prompt, `(None, None)`
/// for a control (any attached agent).
pub type AttachedFn<'a> = &'a dyn Fn(Option<&str>, Option<&str>) -> bool;

pub struct GateContext<'a> {
    pub trust: &'a TrustList,
    pub kem: &'a KemSecretKey,
    pub identity: &'a dyn IdentityPort,
    pub own_cid: Cid,
    pub now_ms: u64,
    /// Verified signals already waiting here.
    pub pending: usize,
    pub attached: AttachedFn<'a>,
}

impl std::fmt::Debug for GateContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GateContext")
            .field("own_cid", &self.own_cid)
            .field("now_ms", &self.now_ms)
            .field("pending", &self.pending)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct DeviceGate {
    limits: GateLimits,
    replay: ReplayGuard,
    rate: RateWindows,
    log: Vec<ReplayEntry>,
}

impl DeviceGate {
    /// Rebuilds the replay state from the accepted log the daemon persisted, so a restart
    /// does not reopen a window for replays.
    pub fn new(limits: GateLimits, log: Vec<ReplayEntry>) -> Self {
        let mut replay = ReplayGuard::empty();
        for e in &log {
            replay.record(&e.kid, &e.nonce, e.seq, e.exp);
        }
        Self {
            limits,
            replay,
            rate: RateWindows::empty(),
            log,
        }
    }

    pub fn replay_log(&self) -> &[ReplayEntry] {
        &self.log
    }

    /// Drops expired nonces, keeping each kid's highest `seq` so `seq` stays monotonic.
    pub fn prune(&mut self, now_ms: u64) {
        self.replay.prune(now_ms);
        let last = |kid| self.replay.last_seq(kid);
        let keep: Vec<ReplayEntry> = self
            .log
            .iter()
            .filter(|e| e.exp > now_ms || last(&e.kid) == Some(e.seq))
            .copied()
            .collect();
        self.log = keep;
    }

    /// `sw-mcp trust` re-confirmed the kid: its refusal count starts over.
    pub fn forgive(&mut self, kid: &sw_crypto::Kid) {
        self.rate.forgive(kid);
    }

    pub fn admit(&mut self, bytes: &[u8], ctx: &GateContext<'_>) -> Outcome {
        let message = match SwMessage::from_bytes(bytes) {
            Ok(m) => m,
            Err(_) => return Outcome::refused(Reason::Malformed, None),
        };
        let SwMessage::Signed { typ: 4 | 5, signed } = &message else {
            return Outcome::refused(Reason::NotASignal, None);
        };
        let kid = signed.kid;
        let Some(controller) = ctx.trust.get(&kid) else {
            return self.count(Reason::UnknownController, kid, None, ctx.now_ms);
        };
        if controller.paused {
            return Outcome::refused(Reason::Paused, Some(kid));
        }
        let verifier = Verifier::Endpoint { cid: ctx.own_cid };
        let verified = match verify(
            &message,
            ctx.trust,
            ctx.identity,
            &mut self.replay,
            verifier,
            ctx.now_ms,
        ) {
            Ok(v) => v,
            Err(r) => return self.count(Reason::Verify(r), kid, None, ctx.now_ms),
        };
        let b = &verified.body;
        self.log.push(ReplayEntry {
            kid,
            nonce: b.nonce,
            seq: b.seq,
            exp: b.exp,
        });
        let ack = Some(AckTo {
            controller_cid: b.from,
            signal_id: verified.signal_id,
        });
        let kind = match &b.payload {
            Payload::Prompt(sealed) => sealed.open::<PromptInner>(ctx.kem).map(SignalKind::Prompt),
            Payload::Control(sealed) => sealed
                .open::<ControlInner>(ctx.kem)
                .map(SignalKind::Control),
            _ => return Outcome::refused(Reason::NotASignal, Some(kid)),
        };
        let Ok(kind) = kind else {
            return self.count(Reason::Unseal, kid, ack, ctx.now_ms);
        };
        let reason = match &kind {
            SignalKind::Prompt(p) if p.aud != ctx.own_cid => Some(Reason::WrongDevice),
            SignalKind::Control(c) if c.aud != ctx.own_cid => Some(Reason::WrongDevice),
            SignalKind::Control(c)
                if !matches!(c.op, ControlOp::Pause | ControlOp::Resume | ControlOp::Stop) =>
            {
                Some(Reason::UnsupportedOp)
            }
            _ => None,
        };
        if let Some(reason) = reason {
            return self.count(reason, kid, ack, ctx.now_ms);
        }
        if !self
            .rate
            .admit(&kid, ctx.now_ms, self.limits.per_kid_per_min)
        {
            return self.count(Reason::RateLimited, kid, ack, ctx.now_ms);
        }
        if ctx.pending >= self.limits.pending_max as usize {
            return self.count(Reason::InboxFull, kid, ack, ctx.now_ms);
        }
        let target = match &kind {
            SignalKind::Prompt(p) => (ctx.attached)(Some(&p.task_id), p.agent_id.as_deref()),
            SignalKind::Control(_) => (ctx.attached)(None, None),
        };
        if !target {
            return self.count(Reason::NoAttachedAgent, kid, ack, ctx.now_ms);
        }
        let window_ends = controller
            .trusted_at_ms
            .saturating_add(self.limits.approval_window_ms);
        Outcome::Admitted(Admitted {
            signal_id: verified.signal_id,
            kid,
            controller_cid: b.from,
            controller_label: controller.label.clone(),
            iat: b.iat,
            exp: b.exp,
            kind,
            needs_approval: ctx.now_ms < window_ends,
        })
    }

    fn count(
        &mut self,
        reason: Reason,
        kid: sw_crypto::Kid,
        ack: Option<AckTo>,
        now_ms: u64,
    ) -> Outcome {
        let pause = self
            .rate
            .refuse(&kid, now_ms, self.limits.refusals_before_pause);
        Outcome::Refused(Refused {
            reason,
            kid: Some(kid),
            ack,
            pause,
        })
    }
}
