//! A three-member project for the scratchpad tests: Alice owns `p` (aware, auto-admit), Bob
//! and Carol joined it; each has one Device enrolled by assertion from the testkit issuer.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use sw_model::Cid;
use sw_model::tier::{JoinPolicy, TierName};
use sw_node::core::{Effect, NodeCore, NodeRefusal};
use sw_protocol::pad::{Recipient, seal_text};
use sw_protocol::payload::*;
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Bytes;
use sw_testkit::LabelledEntropy;
use sw_testkit::pad::{PadDevice, issuer};

pub const NOW: u64 = 1_795_000_000_000;
pub const HOUR: u64 = 3_600_000;

pub struct Pad {
    pub node: NodeCore,
    pub a: PadDevice,
    pub b: PadDevice,
    pub c: PadDevice,
    /// Enrolled, never joined `p`.
    pub d: PadDevice,
    pub stored: BTreeMap<String, Vec<u8>>,
    nonce: u8,
}

fn hello() -> Vec<u8> {
    SwMessage::Unsigned(Payload::Hello(Hello {
        protocol_version: 1,
        endpoint_kind: HelloKind::Device,
        client_build: "test".into(),
    }))
    .to_bytes()
    .unwrap()
}

fn task(op: TaskOp, fields: TaskFields) -> Payload {
    Payload::TaskWrite(TaskWrite {
        op,
        task_id: "p".into(),
        fields,
    })
}

impl Pad {
    pub fn new(rules: sw_node::core::Rules) -> Self {
        let mut w = Self {
            node: NodeCore::new(rules),
            a: PadDevice::new(1, 11, 101, NOW),
            b: PadDevice::new(2, 12, 102, NOW),
            c: PadDevice::new(3, 13, 103, NOW),
            d: PadDevice::new(4, 14, 104, NOW),
            stored: BTreeMap::new(),
            nonce: 0,
        };
        for cid in [101, 102, 103, 104] {
            w.online(cid);
        }
        for who in 1..=4 {
            let proof = w.dev(who).proof.clone();
            w.raw(sw_node_cid(&proof), &proof).unwrap();
        }
        let create = TaskFields {
            title: Some("project".into()),
            visibility: Some(sw_model::Visibility::Members),
            ..TaskFields::EMPTY
        };
        let open = TaskFields {
            tier: Some(TierName::Aware),
            join_policy: Some(JoinPolicy::Auto { max_coworkers: 8 }),
            ..TaskFields::EMPTY
        };
        w.send(1, task(TaskOp::Create, create), 60_000).unwrap();
        w.send(1, task(TaskOp::Update, open), 60_000).unwrap();
        for who in [2, 3] {
            let token = [who; 32];
            let invite = Payload::Invite(Invite {
                project: "p".into(),
                token_hash: sw_protocol::invite::token_hash(&token),
                exp: NOW + HOUR,
            });
            w.send(1, invite, 60_000).unwrap();
            let join = Payload::JoinRequest(JoinRequest {
                task_id: "p".into(),
                note: "joining".into(),
                readiness: sw_model::readiness::Readiness::from_permille(1000).unwrap(),
                agent_id: None,
                invite: Some(token),
            });
            w.send(who, join, 60_000).unwrap();
        }
        w
    }

    pub fn standard() -> Self {
        let mut rules = crate::support::rules(&issuer());
        rules.limits.pad_rate_per_min = 10;
        rules.limits.pad_pending_max = 50;
        let mut w = Self::new(rules);
        let policy = Payload::PadPolicy(PadPolicy {
            project: "p".into(),
            retention_ms: 24 * HOUR,
        });
        w.send(1, policy, 60_000).unwrap();
        w
    }

    pub fn dev(&self, who: u8) -> &PadDevice {
        match who {
            1 => &self.a,
            2 => &self.b,
            3 => &self.c,
            _ => &self.d,
        }
    }

    pub fn online(&mut self, cid: Cid) -> Vec<(Cid, Vec<u8>)> {
        let mut effects = self.node.connected(cid, NOW);
        effects.extend(self.node.received(cid, &hello(), NOW).unwrap());
        self.apply(effects)
    }

    pub fn apply(&mut self, effects: Vec<Effect>) -> Vec<(Cid, Vec<u8>)> {
        let mut sent = Vec::new();
        for e in effects {
            match e {
                Effect::Put { key, value } => {
                    self.stored.insert(key, value);
                }
                Effect::Delete { key } => {
                    self.stored.remove(&key);
                }
                Effect::Send { to, bytes } => sent.push((to, bytes)),
            }
        }
        sent
    }

    pub fn raw(&mut self, cid: Cid, bytes: &[u8]) -> Result<Vec<(Cid, Vec<u8>)>, NodeRefusal> {
        let effects = self.node.received(cid, bytes, NOW)?;
        Ok(self.apply(effects))
    }

    pub fn signed(&mut self, who: u8, payload: Payload, life: u64) -> Vec<u8> {
        self.nonce += 1;
        let n = self.nonce;
        self.dev(who).signed(payload, NOW, life, n)
    }

    pub fn send(
        &mut self,
        who: u8,
        payload: Payload,
        life: u64,
    ) -> Result<Vec<(Cid, Vec<u8>)>, NodeRefusal> {
        let bytes = self.signed(who, payload, life);
        let cid = self.dev(who).cid;
        self.raw(cid, &bytes)
    }

    /// A note from `who` sealed to `to`'s Devices.
    pub fn note(&mut self, who: u8, to: &[u8], scope: PadScope) -> Vec<u8> {
        let recipients: Vec<Recipient<'_>> = to
            .iter()
            .map(|t| Recipient {
                cid: self.dev(*t).cid,
                seal_pk: self.dev(*t).kem.encapsulation_key(),
            })
            .collect();
        let text = PadText {
            body: "free to merge".into(),
            refs: vec![],
        };
        let body = seal_text(&text, &recipients, &mut LabelledEntropy::new("n")).unwrap();
        let targets = match scope {
            PadScope::All => vec![],
            PadScope::Members => to.iter().map(|t| self.dev(*t).id_fp()).collect(),
        };
        let proof = Bytes(self.dev(who).proof.clone());
        let payload = Payload::PadNote(PadNote {
            project: "p".into(),
            scope,
            to: targets,
            body,
            proof,
        });
        self.signed(who, payload, HOUR)
    }

    pub fn ack(&mut self, who: u8, note_id: [u8; 32]) -> Vec<u8> {
        let proof = Bytes(self.dev(who).proof.clone());
        let payload = Payload::PadAck(PadAck {
            project: "p".into(),
            note_id,
            reply: None,
            proof,
        });
        self.signed(who, payload, HOUR)
    }
}

pub fn sw_node_cid(proof: &[u8]) -> Cid {
    let SwMessage::Signed { signed, .. } = SwMessage::from_bytes(proof).unwrap() else {
        unreachable!()
    };
    sw_protocol::body::Body::from_bytes(&signed.body.0)
        .unwrap()
        .from
}

pub fn id_of(bytes: &[u8]) -> [u8; 32] {
    let SwMessage::Signed { signed, .. } = SwMessage::from_bytes(bytes).unwrap() else {
        unreachable!()
    };
    signed.signal_id().unwrap()
}

/// The `PAD_RECEIPT`s among what was sent: (to, stored, reason).
pub fn receipts(sent: &[(Cid, Vec<u8>)]) -> Vec<(Cid, bool, Option<String>)> {
    sent.iter()
        .filter_map(|(to, b)| match SwMessage::from_bytes(b).ok()? {
            SwMessage::Unsigned(Payload::PadReceipt(r)) => Some((*to, r.stored, r.reason)),
            _ => None,
        })
        .collect()
}
