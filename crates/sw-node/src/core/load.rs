//! Rebuilding the core from what the adapter stored: every key the core writes, and nothing
//! else. An entry that does not decode stops the start (fail fast): a node that silently
//! skipped a Person record would refuse that Person's Endpoints as strangers.

use sw_crypto::Kid;

use super::dispatch::{person_key, seq_key, task_key};
use super::{NodeCore, codec, inbox};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    pub key: String,
    pub why: String,
}

fn hex32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

impl NodeCore {
    pub fn load(&mut self, key: &str, value: &[u8]) -> Result<(), LoadError> {
        let fail = |why: String| LoadError {
            key: key.to_owned(),
            why,
        };
        let wire = |e: sw_protocol::WireError| fail(format!("{e:?}"));
        if let Some(_id) = key.strip_prefix("person/") {
            let record = sw_protocol::record::record_from_bytes(value).map_err(wire)?;
            if person_key(&record.id_fp) != key {
                return Err(fail("record stored under another key".into()));
            }
            self.directory.put(record);
        } else if key.starts_with("task/") {
            let task = codec::task_from_bytes(value).map_err(wire)?;
            if task_key(&task.id) != key {
                return Err(fail("task stored under another key".into()));
            }
            self.tree.insert_loaded(task);
        } else if let Some(kid) = key.strip_prefix("seq/") {
            let kid: Kid = hex32(kid).ok_or_else(|| fail("bad kid".into()))?;
            let seq: [u8; 8] = value.try_into().map_err(|_| fail("bad seq".into()))?;
            self.replay.restore_seq(&kid, u64::from_be_bytes(seq));
            if seq_key(&kid) != key {
                return Err(fail("seq stored under another key".into()));
            }
        } else if let Some(rest) = key.strip_prefix("inbox/") {
            let (cid, id) = rest
                .split_once('/')
                .ok_or_else(|| fail("bad inbox key".into()))?;
            let cid: u64 = cid.parse().map_err(|_| fail("bad cid".into()))?;
            let id = hex32(id).ok_or_else(|| fail("bad signal id".into()))?;
            let q = inbox::decode(id, value).ok_or_else(|| fail("bad inbox entry".into()))?;
            self.inbox.load(cid, q);
        } else if !self.pad_load(key, value).map_err(fail)? {
            return Err(fail("not a key the node writes".into()));
        }
        Ok(())
    }
}
