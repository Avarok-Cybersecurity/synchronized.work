//! `NOTIFY` fan-out (design §4.7): each significant event becomes one `NOTIFY` per live
//! session of each recipient Person — the site's toast, `sw-mcp`'s log, and the Citadel App's
//! `Notify` host call (the App's session is the member's agent's C2S session, CW-6). A
//! notification is information only: it is never a trigger for an agent action.

use std::collections::BTreeMap;

use sw_crypto::sha3_256;
use sw_model::visibility::{Viewer, notify_subject};
use sw_model::{Cid, IdFp};
use sw_protocol::WireError;
use sw_protocol::payload::{Notify, Payload};
use sw_protocol::signed::SwMessage;

use super::significant::{Kind, Significant};
use super::task::TaskState;

/// "At most one NOTIFY per de-duplication key per project per hour" (§4.7): a protocol rule,
/// not a tunable.
pub const DEDUP_WINDOW_MS: u64 = 60 * 60 * 1000;

/// The de-duplication key, which is also the `event_id`: one event about one Person on one
/// Task, of one kind.
pub fn event_id(event: &Significant) -> [u8; 32] {
    let mut input = b"sw-notify-v1\0".to_vec();
    for part in [event.kind.name(), &event.task, event.about.as_str()] {
        input.extend_from_slice(part.as_bytes());
        input.push(0);
    }
    sha3_256(&input)
}

#[derive(Debug, Default)]
pub struct Notifier {
    sent: BTreeMap<[u8; 32], u64>,
}

impl Notifier {
    /// The `(session, bytes)` pairs to send for `event`, at most once per window.
    pub fn fan_out(
        &mut self,
        event: &Significant,
        task: &TaskState,
        sessions_of: &dyn Fn(&IdFp) -> Vec<Cid>,
        now_ms: u64,
    ) -> Result<Vec<(Cid, Vec<u8>)>, WireError> {
        self.sent
            .retain(|_, at| now_ms.saturating_sub(*at) < DEDUP_WINDOW_MS);
        let id = event_id(event);
        if self.sent.contains_key(&id) {
            return Ok(Vec::new());
        }
        self.sent.insert(id, now_ms);
        let mut out = Vec::new();
        for person in &event.recipients {
            let viewer = if *person == task.coop.owner {
                Viewer::TaskOwner
            } else {
                Viewer::ProjectMember
            };
            let Some(subject) = notify_subject(task.visibility, viewer, &task.title) else {
                continue;
            };
            let notify = Notify {
                event_id: id,
                kind: event.kind.name().to_owned(),
                text: text(event.kind, &event.about, &subject),
                subject,
                at: now_ms,
            };
            let bytes = SwMessage::Unsigned(Payload::Notify(notify)).to_bytes()?;
            for cid in sessions_of(person) {
                out.push((cid, bytes.clone()));
            }
        }
        Ok(out)
    }
}

fn text(kind: Kind, about: &IdFp, subject: &str) -> String {
    let who = about.as_str();
    match kind {
        Kind::PersonJoinedTask => format!("{who} joined {subject}"),
        Kind::PersonLeftTask => format!("{who} left {subject}"),
        Kind::JoinRequested => format!("{who} asked to join {subject}"),
        Kind::TierChanged => format!("{subject} changed its cooperation tier"),
        Kind::TaskClaimed => format!("{who} started work on {subject}"),
        Kind::TaskDone => format!("{subject} is done"),
        Kind::TaskCancelled => format!("{subject} was cancelled"),
    }
}
