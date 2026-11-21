//! The Device's own scratchpad (pure): notes it received and verified, notes it sent and the
//! acks they got, the ids it has seen, the locally trusted senders, and the event log that
//! wakes agents. Only the adapter reads and writes `pad.json`; nothing here does I/O.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A sender as its proof bound it: Person, Device, and the Device's sealing key (base64url).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sender {
    pub id_fp: String,
    pub device_cid: u64,
    pub seal_pk: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Received {
    pub project: String,
    pub from: Sender,
    pub body: String,
    pub refs: Vec<String>,
    /// The note's `iat` and `exp`.
    pub at: u64,
    pub exp: u64,
    /// When this Device accepted it (the local rate counts these).
    pub received_at: u64,
    pub acked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AckRec {
    pub by: String,
    pub at: u64,
    pub reply: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sent {
    pub project: String,
    /// The Persons it was sealed to.
    pub to: Vec<String>,
    pub body: String,
    pub refs: Vec<String>,
    pub at: u64,
    pub exp: u64,
    pub acks: Vec<AckRec>,
    /// The node's receipt: `None` until it arrives.
    pub stored: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// A verified note for this Device's Person.
    Note,
    /// A verified ack of a note this Device sent.
    Ack,
    /// The node refused a note or ack this Device sent.
    Refused,
    /// Someone spent an invite to one of this Person's projects (a `NOTIFY` from the node).
    JoinRequested,
    /// A person was admitted to a project: this Person, or someone joining theirs.
    Joined,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRec {
    pub cursor: u64,
    pub kind: EventKind,
    pub note_id: String,
    /// The acker (`Ack`), the refusal reason (`Refused`) or the node's text (a join event).
    pub detail: Option<String>,
    /// A join event's subject: the project or Task name, as the node reports it.
    pub subject: Option<String>,
    /// An `Ack` or `Refused` already handed to a waiter. A `Note` stays live until acked.
    pub consumed: bool,
    pub exp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PadLimits {
    pub per_sender_per_min: usize,
    pub pending_max: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadReject {
    Duplicate,
    PendingFull,
    RateLimited,
    UnknownNote,
    NotATarget,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PadState {
    pub received: BTreeMap<String, Received>,
    pub sent: BTreeMap<String, Sent>,
    /// Note and ack ids already accepted, until their `exp`.
    pub seen: BTreeMap<String, u64>,
    pub events: Vec<EventRec>,
    pub next_cursor: u64,
    /// Locally trusted senders: `id_fp → label`.
    pub trusted: BTreeMap<String, String>,
    /// Acks this Device sent and the note each acknowledges, until the node's receipt.
    pub acks_out: BTreeMap<String, String>,
}

impl PadState {
    pub(super) fn append(
        &mut self,
        kind: EventKind,
        note_id: &str,
        detail: Option<String>,
        exp: u64,
    ) -> u64 {
        self.next_cursor += 1;
        let cursor = self.next_cursor;
        self.events.push(EventRec {
            cursor,
            kind,
            note_id: note_id.to_owned(),
            detail,
            subject: None,
            consumed: false,
            exp,
        });
        cursor
    }

    /// Accepts a verified, opened note; returns its event's cursor.
    pub fn note_in(&mut self, id: &str, r: Received, limits: PadLimits) -> Result<u64, PadReject> {
        if self.seen.contains_key(id) {
            return Err(PadReject::Duplicate);
        }
        let now = r.received_at;
        let pending = self
            .received
            .values()
            .filter(|n| !n.acked && n.exp > now)
            .count();
        if pending >= limits.pending_max {
            return Err(PadReject::PendingFull);
        }
        let recent = self
            .received
            .values()
            .filter(|n| n.from.id_fp == r.from.id_fp && now.saturating_sub(n.received_at) < 60_000)
            .count();
        if recent >= limits.per_sender_per_min {
            return Err(PadReject::RateLimited);
        }
        self.seen.insert(id.to_owned(), r.exp);
        let exp = r.exp;
        self.received.insert(id.to_owned(), r);
        Ok(self.append(EventKind::Note, id, None, exp))
    }

    /// Accepts a verified ack of a sent note, once per acker.
    pub fn ack_in(
        &mut self,
        ack_id: &str,
        note_id: &str,
        ack: AckRec,
        exp: u64,
    ) -> Result<u64, PadReject> {
        if self.seen.contains_key(ack_id) {
            return Err(PadReject::Duplicate);
        }
        let sent = self.sent.get_mut(note_id).ok_or(PadReject::UnknownNote)?;
        if !sent.to.contains(&ack.by) {
            return Err(PadReject::NotATarget);
        }
        if sent.acks.iter().any(|a| a.by == ack.by) {
            return Err(PadReject::Duplicate);
        }
        let by = ack.by.clone();
        sent.acks.push(ack);
        self.seen.insert(ack_id.to_owned(), exp);
        Ok(self.append(EventKind::Ack, note_id, Some(by), exp))
    }

    /// The node's receipt for `id`, a note or an ack this Device sent. A refused ack puts its
    /// note back among the pending, so it wakes the agent again. `None` if `id` is unknown.
    pub fn receipt(&mut self, id: &str, stored: bool, reason: String, now_ms: u64) -> Option<u64> {
        let note_id = match self.acks_out.remove(id) {
            Some(note_id) => {
                let n = self.received.get_mut(&note_id)?;
                n.acked = n.acked && stored;
                note_id
            }
            None => {
                let s = self.sent.get_mut(id)?;
                s.stored = Some(stored);
                id.to_owned()
            }
        };
        let exp = self
            .received
            .get(&note_id)
            .map(|n| n.exp)
            .or_else(|| self.sent.get(&note_id).map(|s| s.exp))
            .unwrap_or(now_ms);
        (!stored).then(|| self.append(EventKind::Refused, &note_id, Some(reason), exp))
    }

    /// The first live event after `after`. An `Ack` or `Refused` is handed out once; a `Note`
    /// is handed to every waiter until this Device's Person acks it.
    pub fn next_event(&mut self, after: u64) -> Option<EventRec> {
        let received = &self.received;
        let event = self.events.iter_mut().find(|e| {
            e.cursor > after
                && match e.kind {
                    EventKind::Note => received.get(&e.note_id).is_some_and(|n| !n.acked),
                    EventKind::Ack
                    | EventKind::Refused
                    | EventKind::JoinRequested
                    | EventKind::Joined => !e.consumed,
                }
        })?;
        if event.kind != EventKind::Note {
            event.consumed = true;
        }
        Some(event.clone())
    }

    /// Forgets everything past its `exp`.
    pub fn prune(&mut self, now_ms: u64) -> bool {
        let before = (
            self.received.len(),
            self.sent.len(),
            self.seen.len(),
            self.events.len(),
        );
        self.received.retain(|_, n| n.exp > now_ms);
        self.sent.retain(|_, n| n.exp > now_ms);
        self.seen.retain(|_, exp| *exp > now_ms);
        self.events.retain(|e| e.exp > now_ms);
        before
            != (
                self.received.len(),
                self.sent.len(),
                self.seen.len(),
                self.events.len(),
            )
    }
}
