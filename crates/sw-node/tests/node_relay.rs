#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Store-and-forward and restart (design §4.5, Phase 1 "wrangler dev restart"): a prompt for
//! an offline Device waits in its inbox, reaches it on connect and leaves on its ACK; a node
//! rebuilt from what was stored has the same people and tasks and still refuses a replay.

mod support;

use support::*;
use sw_model::EndpointKind::{Controller, Device};
use sw_node::core::{NodeCore, NodeRefusal};
use sw_protocol::body::Audience;
use sw_protocol::gate::Refusal;
use sw_protocol::payload::*;
use sw_protocol::sealed::Sealed;
use sw_testkit::LabelledEntropy;

fn prompt(w: &World) -> Payload {
    let inner = PromptInner {
        aud: w.alice.device.cid,
        task_id: "t".into(),
        text: "run the suite".into(),
        mode: PromptMode::Queue,
        agent_id: None,
        reply_to: None,
    };
    let ek = w.alice.device.kem.as_ref().unwrap().encapsulation_key();
    Payload::Prompt(Sealed::seal(ek, &inner, &mut LabelledEntropy::new("relay")).unwrap())
}

#[test]
fn an_offline_device_gets_its_prompt_on_connect_until_it_acknowledges() {
    let mut w = World::enrolled();
    let device = w.alice.device.cid;
    w.node.disconnected(device);
    let p = prompt(&w);
    let alice = std::mem::replace(&mut w.alice, World::new().alice);
    let m = w.envelope(&alice, &alice.controller, Audience::Endpoint(device), p);
    let signal_id = match &m {
        sw_protocol::signed::SwMessage::Signed { signed, .. } => signed.signal_id().unwrap(),
        _ => unreachable!(),
    };
    let bytes = m.to_bytes().unwrap();
    let effects = w.node.received(alice.controller.cid, &bytes, NOW).unwrap();
    w.alice = alice;
    assert!(
        w.apply(effects).is_empty(),
        "nothing is sent to an offline device"
    );
    assert!(
        w.stored
            .keys()
            .any(|k| k.starts_with(&format!("inbox/{device:020}/")))
    );

    let delivered = w.connect(device);
    assert_eq!(delivered, vec![(device, bytes.clone())]);
    let again = w.connect(device);
    assert_eq!(again, vec![(device, bytes)], "kept until acknowledged");

    let ack = Payload::Ack(Ack {
        signal_id,
        result: AckResult::Delivered,
        reason: None,
    });
    let controller = w.alice.controller.cid;
    let sent = w
        .send_as(Who::Alice, Device, Audience::Endpoint(controller), ack)
        .unwrap();
    assert_eq!(sent.len(), 1, "the ACK reaches the live controller");
    assert!(
        !w.stored.keys().any(|k| k.starts_with("inbox/")),
        "acknowledged: gone"
    );
}

#[test]
fn a_prompt_must_name_an_enrolled_device() {
    let mut w = World::enrolled();
    let p = prompt(&w);
    let to = w.alice.controller.cid;
    let refused = w.send_as(Who::Alice, Controller, Audience::Endpoint(to), p);
    assert_eq!(refused, Err(NodeRefusal::NoSuchRecipient));
}

#[test]
fn a_rebuilt_node_holds_what_was_stored_and_still_refuses_a_replay() {
    let mut w = World::enrolled();
    let task = Payload::TaskWrite(TaskWrite {
        op: TaskOp::Create,
        task_id: "p".into(),
        fields: TaskFields {
            title: Some("Project".into()),
            visibility: Some(sw_model::Visibility::Owner),
            ..TaskFields::EMPTY
        },
    });
    let alice = std::mem::replace(&mut w.alice, World::new().alice);
    let m = w.envelope(&alice, &alice.controller, Audience::Node, task);
    let bytes = m.to_bytes().unwrap();
    let effects = w.node.received(alice.controller.cid, &bytes, NOW).unwrap();
    w.apply(effects);

    let mut rebuilt = NodeCore::new(rules(&w.issuer));
    for (key, value) in &w.stored {
        rebuilt.load(key, value).unwrap();
    }
    assert_eq!(rebuilt.task("p"), w.node.task("p"));
    assert_eq!(
        rebuilt.directory().person(&alice.identity.id_fp()),
        w.node.directory().person(&alice.identity.id_fp())
    );
    rebuilt.connected(alice.controller.cid, NOW);
    let hello = sw_protocol::signed::SwMessage::Unsigned(Payload::Hello(Hello {
        protocol_version: 1,
        endpoint_kind: HelloKind::Controller,
        client_build: "test".into(),
    }));
    rebuilt
        .received(alice.controller.cid, &hello.to_bytes().unwrap(), NOW)
        .unwrap();
    assert_eq!(
        rebuilt.received(alice.controller.cid, &bytes, NOW),
        Err(NodeRefusal::Gate(Refusal::SeqNotIncreasing))
    );
}

#[test]
fn a_stored_entry_that_does_not_decode_stops_the_load() {
    let mut node = NodeCore::new(rules(&World::new().issuer));
    assert!(node.load("task/x", b"not cbor").is_err());
    assert!(node.load("unexpected/key", b"").is_err());
}
