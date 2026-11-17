//! Store-and-forward (design §3.1, §4.5): the Citadel server delivers only to live sessions,
//! so a signal for an offline Endpoint waits here. A `PROMPT`/`CONTROL` stays until the
//! Device acknowledges it (at-least-once; the Device de-duplicates by `signal_id`); an `ACK`
//! for a Controller is delivered once. Expired entries are dropped. Bounds are refusals.

use std::collections::BTreeMap;

use sw_model::Cid;

use super::NodeRefusal;
use crate::limits::Limits;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queued {
    pub signal_id: [u8; 32],
    pub exp: u64,
    /// Kept until the recipient acknowledges it (signals to a Device), or sent once (ACKs).
    pub until_ack: bool,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct Inbox {
    by_cid: BTreeMap<Cid, Vec<Queued>>,
}

pub fn key(to: Cid, signal_id: &[u8; 32]) -> String {
    let hex: String = signal_id.iter().map(|b| format!("{b:02x}")).collect();
    format!("inbox/{to:020}/{hex}")
}

/// `exp (u64 BE) ‖ until_ack (u8) ‖ bytes`.
pub fn encode(q: &Queued) -> Vec<u8> {
    let mut out = q.exp.to_be_bytes().to_vec();
    out.push(u8::from(q.until_ack));
    out.extend_from_slice(&q.bytes);
    out
}

pub fn decode(signal_id: [u8; 32], stored: &[u8]) -> Option<Queued> {
    let (exp, rest) = stored.split_first_chunk::<8>()?;
    let (flag, bytes) = rest.split_first()?;
    Some(Queued {
        signal_id,
        exp: u64::from_be_bytes(*exp),
        until_ack: match flag {
            0 => false,
            1 => true,
            _ => return None,
        },
        bytes: bytes.to_vec(),
    })
}

impl Inbox {
    pub fn load(&mut self, to: Cid, q: Queued) {
        self.by_cid.entry(to).or_default().push(q);
    }

    pub fn push(&mut self, to: Cid, q: Queued, limits: &Limits) -> Result<(), NodeRefusal> {
        let queue = self.by_cid.entry(to).or_default();
        if queue.iter().any(|e| e.signal_id == q.signal_id) {
            return Ok(());
        }
        let bytes: usize = queue.iter().map(|e| e.bytes.len()).sum();
        if queue.len() >= limits.inbox_max_signals || bytes + q.bytes.len() > limits.inbox_max_bytes
        {
            return Err(NodeRefusal::InboxFull);
        }
        queue.push(q);
        Ok(())
    }

    /// What to deliver to `to` now, and the keys of entries that are gone afterwards (expired,
    /// or sent once).
    pub fn drain_for(&mut self, to: Cid, now_ms: u64) -> (Vec<Vec<u8>>, Vec<[u8; 32]>) {
        let Some(queue) = self.by_cid.get_mut(&to) else {
            return (Vec::new(), Vec::new());
        };
        let mut send = Vec::new();
        let mut gone = Vec::new();
        queue.retain(|q| {
            if q.exp <= now_ms {
                gone.push(q.signal_id);
                return false;
            }
            send.push(q.bytes.clone());
            if q.until_ack {
                true
            } else {
                gone.push(q.signal_id);
                false
            }
        });
        (send, gone)
    }

    /// The Device acknowledged `signal_id`: it is no longer queued for it.
    pub fn acknowledged(&mut self, to: Cid, signal_id: &[u8; 32]) -> bool {
        let Some(queue) = self.by_cid.get_mut(&to) else {
            return false;
        };
        let before = queue.len();
        queue.retain(|q| q.signal_id != *signal_id);
        queue.len() != before
    }
}
