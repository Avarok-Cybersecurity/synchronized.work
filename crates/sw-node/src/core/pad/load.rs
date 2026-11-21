//! Rebuilding the scratchpad from stored keys. Keys arrive sorted, so a note precedes its
//! acks; an ack without its note is a refusal to start, like any other undecodable entry.

use sw_model::IdFp;

use super::ledger::{Entry, policy_from};
use crate::core::NodeCore;

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

fn project_of(hex: &str) -> Result<String, String> {
    let bytes = unhex(hex).ok_or("bad project hex")?;
    String::from_utf8(bytes).map_err(|_| "project is not UTF-8".to_owned())
}

impl NodeCore {
    /// Loads one scratchpad key; `Ok(false)` if `key` is not one.
    pub(crate) fn pad_load(&mut self, key: &str, value: &[u8]) -> Result<bool, String> {
        if let Some(hex) = key.strip_prefix("padpolicy/") {
            let retention = policy_from(value).ok_or("bad retention")?;
            let project = project_of(hex)?;
            self.pad.projects.entry(project).or_default().retention_ms = Some(retention);
        } else if let Some(cid) = key.strip_prefix("proof/") {
            let cid: u64 = cid.parse().map_err(|_| "bad proof cid")?;
            self.pad.proofs.insert(cid, value.to_vec());
        } else if let Some(rest) = key.strip_prefix("pad/") {
            let mut parts = rest.split('/');
            let project = project_of(parts.next().ok_or("no project")?)?;
            let id: [u8; 32] = unhex(parts.next().ok_or("no note id")?)
                .and_then(|v| v.try_into().ok())
                .ok_or("bad note id")?;
            let notes = &mut self.pad.projects.entry(project).or_default().notes;
            match (parts.next(), parts.next(), parts.next()) {
                (None, None, None) => {
                    let entry = Entry::decode(value).map_err(|e| format!("{e:?}"))?;
                    notes.insert(id, entry);
                }
                (Some("ack"), Some(by), None) => {
                    let by = IdFp::parse(by).map_err(|_| "bad ack id_fp")?;
                    let entry = notes.get_mut(&id).ok_or("an ack before its note")?;
                    entry.acks.insert(by, value.to_vec());
                }
                _ => return Err("bad scratchpad key".into()),
            }
        } else {
            return Ok(false);
        }
        Ok(true)
    }
}
