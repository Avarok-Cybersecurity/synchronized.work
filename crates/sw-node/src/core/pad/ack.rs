//! 19 `PAD_ACK` at the node: a target acknowledges a live note once; the ack is stored
//! beside the note and relayed to the note's sender Device, now or when it next connects.

use sw_protocol::gate::Verified;
use sw_protocol::payload::PadAck;

use super::PadRefusal;
use super::ledger::ack_key;
use crate::core::{Effect, NodeCore, NodeRefusal};

impl NodeCore {
    pub(in crate::core) fn pad_ack(
        &mut self,
        v: &Verified,
        a: &PadAck,
        bytes: &[u8],
        now_ms: u64,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        self.pad_proof(v, &a.proof.0)?;
        let by = v.body.iss.clone();
        let entry = self
            .pad
            .projects
            .get(&a.project)
            .and_then(|p| p.notes.get(&a.note_id))
            .filter(|e| e.exp > now_ms)
            .ok_or(PadRefusal::NoSuchNote)?;
        if !entry.targets.contains(&by) {
            return Err(PadRefusal::NotATarget.into());
        }
        if entry.acks.contains_key(&by) {
            return Err(PadRefusal::AlreadyAcked.into());
        }
        let sender = entry.from_cid;
        let to_sender = a
            .reply
            .as_ref()
            .is_none_or(|r| r.keys.iter().all(|k| k.cid == sender));
        if !to_sender {
            return Err(PadRefusal::BadRecipients.into());
        }
        let rate = self.rules.limits.pad_rate_per_min;
        if !self.pad.take_rate(&by, now_ms, rate) {
            return Err(PadRefusal::RateLimited.into());
        }
        let mut effects = vec![Effect::Put {
            key: ack_key(&a.project, &a.note_id, &by),
            value: bytes.to_vec(),
        }];
        if self.online.contains(&sender) {
            effects.push(Effect::Send {
                to: sender,
                bytes: bytes.to_vec(),
            });
        }
        if let Some(e) = self
            .pad
            .projects
            .get_mut(&a.project)
            .and_then(|p| p.notes.get_mut(&a.note_id))
        {
            e.acks.insert(by, bytes.to_vec());
        }
        effects.push(Self::pad_receipt(v.body.from, &a.project, v, None)?);
        Ok(effects)
    }
}
