//! A small world for the node's tests: two Persons enrolled by assertion from the pinned
//! issuer, and builders for the envelopes they send. Every key is deterministic.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

pub mod identity_kat;
pub mod rules;
pub mod vars;

pub use rules::rules;

use std::collections::BTreeMap;

use data_encoding::BASE64URL_NOPAD;
use sw_crypto::{KemSecretKey, SigningKey, kid_of};
use sw_model::{Cid, EndpointKind};
use sw_node::core::{Effect, NodeCore};
use sw_protocol::body::{Audience, Body};
use sw_protocol::payload::*;
use sw_protocol::ports::{MessageSigner, SignInClaims};
use sw_protocol::signed::SwMessage;
use sw_testkit::assertion::sign_assertion;
use sw_testkit::{FIXTURE_NOW_MS, StandInIdentity};

pub const NOW: u64 = FIXTURE_NOW_MS;

pub struct Endpoint {
    pub cid: Cid,
    pub kind: EndpointKind,
    pub key: SigningKey,
    pub kem: Option<KemSecretKey>,
}

pub struct Person {
    pub identity: StandInIdentity,
    pub controller: Endpoint,
    pub device: Endpoint,
}

pub struct World {
    pub issuer: SigningKey,
    pub alice: Person,
    pub bob: Person,
    pub node: NodeCore,
    pub seq: BTreeMap<Cid, u64>,
    /// What the node stored, as the Durable Object would hold it.
    pub stored: BTreeMap<String, Vec<u8>>,
}

fn person(seed: u8, base_cid: Cid) -> Person {
    Person {
        identity: StandInIdentity::from_seed(&[seed; 32]),
        controller: Endpoint {
            cid: base_cid,
            kind: EndpointKind::Controller,
            key: SigningKey::from_seed(&[seed + 0x10; 32]),
            kem: None,
        },
        device: Endpoint {
            cid: base_cid + 1,
            kind: EndpointKind::Device,
            key: SigningKey::from_seed(&[seed + 0x20; 32]),
            kem: Some(KemSecretKey::from_seed(&[seed + 0x30; 64])),
        },
    }
}

impl World {
    pub fn new() -> Self {
        let issuer = SigningKey::from_seed(&[0x51; 32]);
        let node = NodeCore::new(rules(&issuer));
        Self {
            issuer,
            alice: person(0x01, 1000),
            bob: person(0x02, 2000),
            node,
            seq: BTreeMap::new(),
            stored: BTreeMap::new(),
        }
    }

    /// Records what the node stored; returns what it sent.
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

    pub fn connect(&mut self, cid: Cid) -> Vec<(Cid, Vec<u8>)> {
        let effects = connect(&mut self.node, cid);
        self.apply(effects)
    }

    pub fn send(
        &mut self,
        who: &Person,
        ep: &Endpoint,
        aud: Audience,
        payload: Payload,
    ) -> Result<Vec<(Cid, Vec<u8>)>, sw_node::core::NodeRefusal> {
        let bytes = self.envelope(who, ep, aud, payload).to_bytes().unwrap();
        let effects = self.node.received(ep.cid, &bytes, NOW)?;
        Ok(self.apply(effects))
    }

    pub fn envelope(
        &mut self,
        who: &Person,
        ep: &Endpoint,
        aud: Audience,
        payload: Payload,
    ) -> SwMessage {
        let seq = self.seq.entry(ep.cid).or_insert(0);
        *seq += 1;
        let body = Body {
            iss: who.identity.id_fp(),
            from: ep.cid,
            aud,
            iat: NOW,
            exp: NOW + 60_000,
            nonce: [(*seq % 251) as u8; 16],
            seq: *seq,
            payload,
        };
        SwMessage::signed(&body, &ep.key).unwrap()
    }

    pub fn enrol_payload(&self, who: &Person, ep: &Endpoint) -> Payload {
        let device = format!(
            "laptop#{}",
            BASE64URL_NOPAD.encode(&kid_of(ep.key.public_key()))
        );
        let claims = SignInClaims {
            identity: who.identity.public_key().to_vec(),
            device,
            aud: "sw-mcp".into(),
            exp_s: NOW / 1000 + 600,
        };
        let assertion = sign_assertion(&self.issuer, &claims, &[7; 32]);
        Payload::Enrol(Enrol {
            op: EnrolOp::Enrol,
            epoch: 1,
            endpoint_cid: Some(ep.cid),
            kind: Some(ep.kind),
            pubkeys: Some(PubKeys {
                sign: sw_protocol::wire::Bytes(ep.key.public_key().to_vec()),
                seal: ep
                    .kem
                    .as_ref()
                    .map(|k| sw_protocol::wire::Bytes(k.encapsulation_key().to_vec())),
            }),
            kid: None,
            assertion: Some(sw_protocol::wire::Bytes(assertion)),
        })
    }

    /// Both Persons, both Endpoints each, enrolled by assertion and connected.
    pub fn enrolled() -> Self {
        let mut w = Self::new();
        for who in [Who::Alice, Who::Bob] {
            for kind in [EndpointKind::Controller, EndpointKind::Device] {
                let cid = w.ep(who, kind).cid;
                w.connect(cid);
                let payload = w.enrol_payload(w.person(who), w.ep(who, kind));
                w.send_as(who, kind, Audience::Node, payload).unwrap();
            }
        }
        w
    }

    pub fn person(&self, who: Who) -> &Person {
        match who {
            Who::Alice => &self.alice,
            Who::Bob => &self.bob,
        }
    }

    pub fn ep(&self, who: Who, kind: EndpointKind) -> &Endpoint {
        let p = self.person(who);
        match kind {
            EndpointKind::Controller => &p.controller,
            EndpointKind::Device => &p.device,
        }
    }

    pub fn send_as(
        &mut self,
        who: Who,
        kind: EndpointKind,
        aud: Audience,
        payload: Payload,
    ) -> Result<Vec<(Cid, Vec<u8>)>, sw_node::core::NodeRefusal> {
        // The Persons are moved out for the call so `self.node` can be borrowed mutably.
        let alice = std::mem::replace(&mut self.alice, person(0x01, 1000));
        let bob = std::mem::replace(&mut self.bob, person(0x02, 2000));
        let p = match who {
            Who::Alice => &alice,
            Who::Bob => &bob,
        };
        let ep = match kind {
            EndpointKind::Controller => &p.controller,
            EndpointKind::Device => &p.device,
        };
        let out = self.send(p, ep, aud, payload);
        self.alice = alice;
        self.bob = bob;
        out
    }
}

/// Opens session `cid` and sends its `HELLO`; returns what the node did.
pub fn connect(node: &mut NodeCore, cid: Cid) -> Vec<Effect> {
    let mut effects = node.connected(cid, NOW);
    let hello = SwMessage::Unsigned(Payload::Hello(Hello {
        protocol_version: 1,
        endpoint_kind: HelloKind::Device,
        client_build: "test".into(),
    }));
    effects.extend(node.received(cid, &hello.to_bytes().unwrap(), NOW).unwrap());
    effects
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who {
    Alice,
    Bob,
}
