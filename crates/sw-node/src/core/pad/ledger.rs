//! The developer scratchpad's ledger at the node: per project, the policy and an
//! append-only set of notes with their acks. An entry stores the note's signed bytes and the
//! targets resolved when it was posted; everything else (sender, Devices, expiry) is derived
//! from those bytes on load, never copied (SSOT).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use sw_model::{Cid, IdFp};
use sw_protocol::body::Body;
use sw_protocol::payload::{PadNote, Payload};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::{Bytes, MapIn, MapOut};
use sw_protocol::{WireError, cbor};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub from_person: IdFp,
    pub from_cid: Cid,
    /// The Devices the body is sealed to.
    pub devices: Vec<Cid>,
    pub targets: BTreeSet<IdFp>,
    pub created: u64,
    pub exp: u64,
    pub bytes: Vec<u8>,
    /// Each target's ack, as the signed bytes it sent.
    pub acks: BTreeMap<IdFp, Vec<u8>>,
}

impl Entry {
    pub fn pending_for(&self, person: &IdFp) -> bool {
        self.targets.contains(person) && !self.acks.contains_key(person)
    }
}

#[derive(Debug, Default)]
pub struct PadProject {
    pub retention_ms: Option<u64>,
    pub notes: BTreeMap<[u8; 32], Entry>,
}

#[derive(Debug, Default)]
pub struct Ledger {
    pub projects: BTreeMap<String, PadProject>,
    /// Each Device's ENROL-by-assertion bytes, by CID.
    pub proofs: BTreeMap<Cid, Vec<u8>>,
    /// When each Person's recent notes and acks were accepted (in memory: a restart forgets
    /// at most one minute of rate).
    pub recent: BTreeMap<IdFp, VecDeque<u64>>,
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn policy_key(project: &str) -> String {
    format!("padpolicy/{}", hex(project.as_bytes()))
}

pub fn note_key(project: &str, id: &[u8; 32]) -> String {
    format!("pad/{}/{}", hex(project.as_bytes()), hex(id))
}

/// Sorts after its note's key, so a sorted load meets the note first.
pub fn ack_key(project: &str, id: &[u8; 32], by: &IdFp) -> String {
    format!("{}/ack/{}", note_key(project, id), by.as_str())
}

pub fn proof_key(cid: Cid) -> String {
    format!("proof/{cid:020}")
}

/// The note inside signed bytes, with the body that carries it.
pub fn decode_note(bytes: &[u8]) -> Result<(Body, PadNote), WireError> {
    let SwMessage::Signed { typ: 18, signed } = SwMessage::from_bytes(bytes)? else {
        return Err(WireError::Shape("a stored note is a signed 18"));
    };
    let body = Body::from_bytes(&signed.body.0)?;
    match body.payload.clone() {
        Payload::PadNote(n) => Ok((body, n)),
        _ => Err(WireError::Shape("a stored note is a PAD_NOTE")),
    }
}

impl Entry {
    pub fn new(bytes: Vec<u8>, targets: BTreeSet<IdFp>, created: u64) -> Result<Self, WireError> {
        let (body, note) = decode_note(&bytes)?;
        Ok(Self {
            from_person: body.iss,
            from_cid: body.from,
            devices: note.body.keys.iter().map(|k| k.cid).collect(),
            targets,
            created,
            exp: body.exp,
            bytes,
            acks: BTreeMap::new(),
        })
    }

    /// `{bytes, targets, created}`.
    pub fn encode(&self) -> Result<Vec<u8>, WireError> {
        let mut out = MapOut::default();
        out.field("bytes", &Bytes(self.bytes.clone()));
        out.field("targets", &self.targets.iter().cloned().collect::<Vec<_>>());
        out.field("created", &self.created);
        cbor::encode(&out.build())
    }

    pub fn decode(stored: &[u8]) -> Result<Self, WireError> {
        let mut map = MapIn::new(cbor::decode(stored)?)?;
        let bytes: Bytes = map.field("bytes")?;
        let targets: Vec<IdFp> = map.field("targets")?;
        let created: u64 = map.field("created")?;
        map.finish()?;
        Self::new(bytes.0, targets.into_iter().collect(), created)
    }
}

pub fn policy_value(retention_ms: u64) -> Vec<u8> {
    retention_ms.to_be_bytes().to_vec()
}

pub fn policy_from(stored: &[u8]) -> Option<u64> {
    stored.try_into().ok().map(u64::from_be_bytes)
}

impl Ledger {
    /// Whether `person` may send one more scratchpad message at `now_ms`; records it if so.
    pub fn take_rate(&mut self, person: &IdFp, now_ms: u64, per_min: usize) -> bool {
        let q = self.recent.entry(person.clone()).or_default();
        while q
            .front()
            .is_some_and(|t| now_ms.saturating_sub(*t) >= 60_000)
        {
            q.pop_front();
        }
        if q.len() >= per_min {
            return false;
        }
        q.push_back(now_ms);
        true
    }
}
