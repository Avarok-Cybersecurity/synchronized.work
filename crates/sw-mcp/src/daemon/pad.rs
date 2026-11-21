//! The scratchpad's socket requests: post (completed when the node's directory answers, see
//! `pad_in`), members, inbox, ack, wait, policy and trust. `PadWait` is the one wake-up path:
//! `serve`'s channel push and `sw-mcp pad wait` both use it.

use serde_json::json;
use sw_protocol::payload::{PadDirectory, PadText, Payload};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Wire as _;
use tokio::sync::oneshot;

use super::{Daemon, now_ms};
use crate::home::{Home, read_private, write_private};
use crate::ipc::Reply;
use crate::pad::event::render;
use crate::pad::state::{PadLimits, PadState};

/// How long a post or members request waits for the node's directory.
pub const ASK_TIMEOUT_MS: u64 = 30_000;
/// Seconds a client waits for a post or members answer (the daemon times it out first).
pub fn pad_ask_timeout_s() -> u64 {
    ASK_TIMEOUT_MS / 1000
}

/// The longest a single `PadWait` blocks; a waiter re-arms after it.
pub const PAD_WAIT_MAX_S: u64 = 24 * 60 * 60;

pub(crate) enum AskKind {
    Post {
        to: Option<Vec<String>>,
        text: PadText,
    },
    Members,
}

pub(crate) struct PadAsk {
    pub(crate) project: String,
    pub(crate) kind: AskKind,
    pub(crate) deadline_ms: u64,
    pub(crate) reply: oneshot::Sender<Reply>,
}

pub(crate) struct PadWaiter {
    pub(crate) after: u64,
    pub(crate) deadline_ms: u64,
    pub(crate) reply: oneshot::Sender<Reply>,
}

pub(super) fn load_state(home: &Home) -> Result<PadState, String> {
    match read_private(&home.pad_state()).map_err(|e| e.to_string())? {
        Some(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("pad.json: {e}")),
        None => Ok(PadState::default()),
    }
}

pub(crate) fn hex32(text: &str) -> Result<[u8; 32], String> {
    let bytes = data_encoding::HEXLOWER
        .decode(text.as_bytes())
        .map_err(|_| "a note id is 64 lowercase hex characters".to_owned())?;
    bytes
        .try_into()
        .map_err(|_| "a note id is 32 bytes".to_owned())
}

impl Daemon {
    pub(crate) fn pad_limits(&self) -> PadLimits {
        PadLimits {
            per_sender_per_min: self.config.rate.per_kid_per_min as usize,
            pending_max: self.config.rate.pending_max as usize,
        }
    }

    pub(crate) fn save_pad(&self) {
        match serde_json::to_vec(&self.pad) {
            Ok(bytes) => {
                if let Err(e) = write_private(&self.home.pad_state(), &bytes) {
                    log::error!("saving pad.json: {e}");
                }
            }
            Err(e) => log::error!("encoding pad.json: {e}"),
        }
    }

    pub(super) fn pad_connected(&mut self) {
        if self.pad.prune(now_ms()) {
            self.save_pad();
        }
    }

    pub(super) fn own_id(&self) -> String {
        self.signed_in
            .as_ref()
            .map(|s| s.id_fp.as_str().to_owned())
            .unwrap_or_default()
    }

    /// Answers every waiter that has an event, or whose time is up.
    pub(crate) fn wake_pad(&mut self) {
        let now = now_ms();
        let own = self.own_id();
        let mut changed = false;
        self.pad_waiters.retain(|w| !w.reply.is_closed());
        for w in std::mem::take(&mut self.pad_waiters) {
            let event = self.pad.next_event(w.after);
            changed |= event.is_some();
            match event.and_then(|rec| render(&self.pad, &rec, &own)) {
                Some(e) => match serde_json::to_value(&e) {
                    Ok(v) => drop(w.reply.send(Reply::Ok(v))),
                    Err(err) => drop(w.reply.send(Reply::Error(err.to_string()))),
                },
                None if now >= w.deadline_ms => {
                    let _ = w
                        .reply
                        .send(Reply::Ok(json!({"timeout": true, "cursor": w.after})));
                }
                None => self.pad_waiters.push(w),
            }
        }
        for ask in std::mem::take(&mut self.pad_asks) {
            if now >= ask.deadline_ms {
                let _ = ask.reply.send(Reply::Error(
                    "the node did not answer the directory request".into(),
                ));
            } else {
                self.pad_asks.push(ask);
            }
        }
        if changed {
            self.save_pad();
        }
    }

    /// The requests answered later: a post or members list (after the directory) and a wait.
    pub(super) fn pad_deferred(
        &mut self,
        request: crate::ipc::Request,
        reply: oneshot::Sender<Reply>,
    ) {
        use crate::ipc::Request as R;
        let (project, kind) = match request {
            R::PadWait { after, max_wait_s } => {
                let wait = max_wait_s.min(PAD_WAIT_MAX_S);
                let deadline_ms = now_ms() + wait * 1000;
                self.pad_waiters.push(PadWaiter {
                    after,
                    deadline_ms,
                    reply,
                });
                return self.wake_pad();
            }
            R::PadPost {
                project,
                to,
                body,
                refs,
            } => (
                project,
                AskKind::Post {
                    to,
                    text: PadText { body, refs },
                },
            ),
            R::PadMembers { project } => (project, AskKind::Members),
            _ => return drop(reply.send(Reply::Error("not a deferred request".into()))),
        };
        if !self.online || self.signed_in.is_none() {
            let _ = reply.send(Reply::Error(
                "not connected to the node; try again shortly".into(),
            ));
            return;
        }
        if let AskKind::Post { text, .. } = &kind
            && let Err(e) = PadText::from_value(text.to_value())
        {
            let _ = reply.send(Reply::Error(format!(
                "the note is over the scratchpad's limits: {e:?}"
            )));
            return;
        }
        let ask = PadDirectory {
            project: project.clone(),
            members: None,
            retention_ms: None,
        };
        match SwMessage::Unsigned(Payload::PadDirectory(ask)).to_bytes() {
            Ok(bytes) => self.send(bytes),
            Err(e) => return drop(reply.send(Reply::Error(format!("{e:?}")))),
        }
        let deadline_ms = now_ms() + ASK_TIMEOUT_MS;
        self.pad_asks.push(PadAsk {
            project,
            kind,
            deadline_ms,
            reply,
        });
    }
}
