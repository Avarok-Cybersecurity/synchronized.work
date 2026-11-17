#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! ENROL by assertion at the node, HELLO first, and the signature / replay controls (§11.4
//! "signature": a flipped envelope byte → `bad_signature`; a replayed envelope → `replay`).

mod support;

use support::*;
use sw_model::EndpointKind::{Controller, Device};
use sw_node::core::NodeRefusal;
use sw_protocol::body::Audience;
use sw_protocol::gate::Refusal;

#[test]
fn a_person_arrives_by_assertion_and_is_stored() {
    let w = World::enrolled();
    let alice = w
        .node
        .directory()
        .person(&w.alice.identity.id_fp())
        .unwrap();
    assert_eq!(alice.endpoints.len(), 2);
    assert!(
        w.stored
            .contains_key(&format!("person/{}", w.alice.identity.id_fp().as_str()))
    );
}

#[test]
fn nothing_but_hello_opens_a_session() {
    let mut w = World::new();
    w.node.connected(w.alice.device.cid, NOW);
    let payload = w.enrol_payload(&w.alice, &w.alice.device);
    let refused = w.send_as(Who::Alice, Device, Audience::Node, payload);
    assert_eq!(refused, Err(NodeRefusal::NoHello));
}

#[test]
fn a_flipped_byte_is_a_bad_signature_and_a_replay_is_refused() {
    let mut w = World::enrolled();
    let task = sw_protocol::payload::Payload::TaskWrite(sw_protocol::payload::TaskWrite {
        op: sw_protocol::payload::TaskOp::Create,
        task_id: "t1".into(),
        fields: sw_protocol::payload::TaskFields {
            title: Some("Ship the node".into()),
            visibility: Some(sw_model::Visibility::Members),
            ..sw_protocol::payload::TaskFields::EMPTY
        },
    });
    let alice = std::mem::replace(&mut w.alice, World::new().alice);
    let message = w.envelope(&alice, &alice.controller, Audience::Node, task);
    let cid = alice.controller.cid;
    let mut bytes = message.to_bytes().unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    assert_eq!(
        w.node.received(cid, &bytes, NOW),
        Err(NodeRefusal::Gate(Refusal::BadSignature))
    );
    let good = message.to_bytes().unwrap();
    w.node.received(cid, &good, NOW).unwrap();
    assert_eq!(
        w.node.received(cid, &good, NOW),
        Err(NodeRefusal::Gate(Refusal::Replay))
    );
}

#[test]
fn an_endpoint_cannot_be_enrolled_twice_under_two_people() {
    let mut w = World::enrolled();
    // Bob's assertion over Alice's device key and CID is refused: the device names the key.
    let mut payload = w.enrol_payload(&w.bob, &w.bob.controller);
    if let sw_protocol::payload::Payload::Enrol(e) = &mut payload {
        e.endpoint_cid = Some(w.alice.device.cid);
    }
    let refused = w.send_as(Who::Bob, Controller, Audience::Node, payload);
    assert!(refused.is_err(), "{refused:?}");
}
