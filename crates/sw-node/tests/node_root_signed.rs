#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Root-signed `ENROL` and `REVOKE` at the node (sw-wire §3 row 11): once a Person exists,
//! adding or removing one of its Endpoints is signed by the Person's identity key, which the
//! node checks with `citadel-identity`. The KAT Person (C0 identity A) arrives by its
//! published assertion; the root messages are signed with A's key, with identity B's key
//! under A's `kid`, or with one byte of A's signature flipped.

mod support;

use support::identity_kat::{RootKey, root_key};
use support::{NOW, connect, rules};
use sw_crypto::kid_of;
use sw_model::EndpointKind;
use sw_node::core::{NodeCore, NodeRefusal};
use sw_protocol::body::{Audience, Body};
use sw_protocol::gate::Refusal;
use sw_protocol::payload::{Enrol, EnrolOp, Payload, PubKeys};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Bytes;
use sw_testkit::kat::Kat;
use sw_testkit::sign_in_kat::{SignInKat, enrol_by_assertion};

#[derive(Clone, Copy)]
enum By {
    A,
    B,
}

struct World {
    node: NodeCore,
    kat: Kat,
    a: RootKey,
    b: RootKey,
    seq: u64,
}

impl World {
    /// The node with the KAT Person enrolled by its published assertion: one Device.
    fn new() -> Self {
        let kat = Kat::parse(include_str!("../../../fixtures/sw-wire-kat.json")).unwrap();
        let si = SignInKat::parse(include_str!("../../../fixtures/sign-in-kat.json")).unwrap();
        let a = root_key("A");
        assert_eq!(a.id_fp, kat.person, "sw-wire's Person is identity A");
        let mut node = NodeCore::new(rules(&si.issuer));
        connect(&mut node, kat.device_cid);
        let enrol = enrol_by_assertion(&kat, &si.blob).to_bytes().unwrap();
        node.received(kat.device_cid, &enrol, NOW).unwrap();
        Self {
            node,
            kat,
            a,
            b: root_key("B"),
            seq: 0,
        }
    }

    /// A's re-`ENROL` adding the KAT Controller.
    fn add_controller(&self) -> Payload {
        Payload::Enrol(Enrol {
            op: EnrolOp::Enrol,
            epoch: 1,
            endpoint_cid: Some(self.kat.controller_cid),
            kind: Some(EndpointKind::Controller),
            pubkeys: Some(PubKeys {
                sign: Bytes(self.kat.controller.public_key().to_vec()),
                seal: None,
            }),
            kid: None,
            assertion: None,
        })
    }

    /// A's `REVOKE` of the KAT Controller.
    fn revoke_controller(&self) -> Payload {
        Payload::Enrol(Enrol {
            op: EnrolOp::Revoke,
            epoch: 2,
            endpoint_cid: None,
            kind: None,
            pubkeys: None,
            kid: Some(kid_of(self.kat.controller.public_key())),
            assertion: None,
        })
    }

    /// `payload` from A, sent on the Device's session, signed with `by`'s identity key but
    /// naming A's `kid`, so the node looks up A's key whichever key signed.
    fn root_signed(&mut self, payload: Payload, by: By) -> SwMessage {
        self.seq += 1;
        let key = match by {
            By::A => &self.a.key,
            By::B => &self.b.key,
        };
        let body = Body {
            iss: self.a.id_fp.clone(),
            from: self.kat.device_cid,
            aud: Audience::Node,
            iat: NOW,
            exp: NOW + 60_000,
            nonce: [self.seq as u8; 16],
            seq: self.seq,
            payload,
        };
        let mut message = SwMessage::signed(&body, key).unwrap();
        if let SwMessage::Signed { signed, .. } = &mut message {
            signed.kid = kid_of(self.a.key.public_key());
        }
        message
    }

    fn send(&mut self, message: &SwMessage) -> Result<(), NodeRefusal> {
        let bytes = message.to_bytes().unwrap();
        self.node
            .received(self.kat.device_cid, &bytes, NOW)
            .map(drop)
    }

    fn endpoints(&self) -> Vec<u64> {
        self.node.directory().endpoints_of(&self.a.id_fp)
    }
}

#[test]
fn a_root_signed_enrol_and_revoke_from_the_persons_own_key_are_accepted() {
    let mut w = World::new();
    let (a_device, a_controller) = (w.kat.device_cid, w.kat.controller_cid);
    let enrol = w.root_signed(w.add_controller(), By::A);
    w.send(&enrol).unwrap();
    assert_eq!(w.endpoints(), vec![a_device, a_controller]);
    let revoke = w.root_signed(w.revoke_controller(), By::A);
    w.send(&revoke).unwrap();
    assert_eq!(w.endpoints(), vec![a_device]);
    assert_eq!(w.node.directory().person(&w.a.id_fp).unwrap().epoch, 2);
}

#[test]
fn a_root_enrol_signed_by_another_identity_is_refused_and_changes_nothing() {
    let mut w = World::new();
    let forged = w.root_signed(w.add_controller(), By::B);
    assert_eq!(
        w.send(&forged),
        Err(NodeRefusal::Gate(Refusal::BadSignature))
    );
    assert_eq!(w.endpoints(), vec![w.kat.device_cid]);
    // The same enrol signed by A is accepted: the refusal was the key, not the message.
    let genuine = w.root_signed(w.add_controller(), By::A);
    w.send(&genuine).unwrap();
    assert_eq!(w.endpoints().len(), 2);
}

#[test]
fn a_root_revoke_with_a_flipped_signature_byte_is_refused_and_changes_nothing() {
    let mut w = World::new();
    let enrol = w.root_signed(w.add_controller(), By::A);
    w.send(&enrol).unwrap();
    let genuine = w.root_signed(w.revoke_controller(), By::A);
    let mut flipped = genuine.clone();
    if let SwMessage::Signed { signed, .. } = &mut flipped {
        signed.sig.0[0] ^= 1;
    }
    assert_eq!(
        w.send(&flipped),
        Err(NodeRefusal::Gate(Refusal::BadSignature))
    );
    assert_eq!(w.endpoints().len(), 2);
    // The refused copy burned nothing: the unflipped revoke is still accepted.
    w.send(&genuine).unwrap();
    assert_eq!(w.endpoints(), vec![w.kat.device_cid]);
}
