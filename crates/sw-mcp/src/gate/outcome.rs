//! What the gate decides, and the reason codes a refusal carries (in the log, and in the
//! `ACK{refused, reason}` the Controller sees).

use sw_crypto::Kid;
use sw_model::Cid;
use sw_protocol::gate::Refusal;
use sw_protocol::payload::{ControlInner, PromptInner};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalKind {
    Prompt(PromptInner),
    Control(ControlInner),
}

/// A signal that passed every check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admitted {
    pub signal_id: [u8; 32],
    pub kid: Kid,
    pub controller_cid: Cid,
    pub controller_label: String,
    pub iat: u64,
    pub exp: u64,
    pub kind: SignalKind,
    /// The Controller is inside its approval window: a person approves it first.
    pub needs_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    Malformed,
    /// Not a `PROMPT` or `CONTROL`.
    NotASignal,
    /// The kid is not in this Device's allowlist.
    UnknownController,
    Paused,
    /// The shared gate refused it (signature, audience, time, replay, seq, …).
    Verify(Refusal),
    /// The sealed payload did not open with this Device's key.
    Unseal,
    /// The sealed inner `aud` is another Device's CID.
    WrongDevice,
    /// `set_tier` / `approve_join` / `refuse_join`: the per-agent opt-in is not built.
    UnsupportedOp,
    RateLimited,
    InboxFull,
    NoAttachedAgent,
}

impl Reason {
    /// The reason code: lower snake case, no key material, no payload text.
    pub fn code(&self) -> String {
        match self {
            Self::Malformed => "malformed".into(),
            Self::NotASignal => "not_a_signal".into(),
            Self::UnknownController => "unknown_controller".into(),
            Self::Paused => "controller_paused".into(),
            Self::Verify(r) => format!("verify_{}", refusal_code(r)),
            Self::Unseal => "unseal_failed".into(),
            Self::WrongDevice => "wrong_device".into(),
            Self::UnsupportedOp => "unsupported_op".into(),
            Self::RateLimited => "rate_limited".into(),
            Self::InboxFull => "inbox_full".into(),
            Self::NoAttachedAgent => "no_attached_agent".into(),
        }
    }
}

fn refusal_code(r: &Refusal) -> &'static str {
    match r {
        Refusal::Wire(_) => "wire",
        Refusal::UnknownAlg => "unknown_alg",
        Refusal::UnknownKid => "unknown_kid",
        Refusal::BadSignature => "bad_signature",
        Refusal::WrongSigner => "wrong_signer",
        Refusal::IssuerMismatch => "issuer_mismatch",
        Refusal::FromMismatch => "from_mismatch",
        Refusal::WrongAudience => "wrong_audience",
        Refusal::FromFuture => "from_future",
        Refusal::Expired => "expired",
        Refusal::LifetimeTooLong => "lifetime_too_long",
        Refusal::Replay => "replay",
        Refusal::SeqNotIncreasing => "seq_not_increasing",
        Refusal::SigningMismatch => "signing_mismatch",
        Refusal::TypMismatch => "typ_mismatch",
        Refusal::NotP2pEligible => "not_p2p_eligible",
        Refusal::Assertion(_) => "assertion",
    }
}

/// Where a refusal is acknowledged: only once the signature verified, so the gate never
/// signs an `ACK` towards a CID that unauthenticated bytes named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckTo {
    pub controller_cid: Cid,
    pub signal_id: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub reason: Reason,
    pub kid: Option<Kid>,
    pub ack: Option<AckTo>,
    /// This refusal reached the per-minute limit: pause the kid until `sw-mcp trust`.
    pub pause: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Admitted(Admitted),
    Refused(Refused),
}

impl Outcome {
    pub(super) fn refused(reason: Reason, kid: Option<Kid>) -> Self {
        Self::Refused(Refused {
            reason,
            kid,
            ack: None,
            pause: false,
        })
    }
}

/// One accepted `(kid, nonce, seq, exp)`, persisted so a restart keeps refusing replays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayEntry {
    pub kid: Kid,
    pub nonce: [u8; 16],
    pub seq: u64,
    pub exp: u64,
}
