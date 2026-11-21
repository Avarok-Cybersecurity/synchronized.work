#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! After the request: only the owner answers (from any of their Endpoints), a refused person
//! gets nothing, only members write under a project, and the writer of a refused write is
//! told why. Every refusal has its control beside it.

mod invite_support;
mod support;

use invite_support::*;
use support::*;
use sw_model::Cid;
use sw_model::EndpointKind::{Controller, Device};
use sw_model::tier::Refusal;
use sw_node::core::NodeRefusal;
use sw_protocol::body::Audience;
use sw_protocol::payload::*;
use sw_protocol::signed::SwMessage;
#[test]
fn only_the_owner_answers_from_any_endpoint_and_a_refused_person_gets_nothing() {
    let mut w = invited();
    let (id, ok) = bob_joins(&mut w, Some(1), NOW);
    ok.unwrap();
    let by_bob = w.send_as(Who::Bob, Device, Audience::Node, answer(id, true));
    assert_eq!(by_bob, Err(NodeRefusal::Tier(Refusal::NotAuthorized)));
    assert_eq!(members(&w), 1);
    let mut refused = invited();
    let (rid, ok) = bob_joins(&mut refused, Some(1), NOW);
    ok.unwrap();
    refused
        .send_as(Who::Alice, Device, Audience::Node, answer(rid, false))
        .unwrap();
    assert_eq!(members(&refused), 1, "a refused person is no member");
    let child = refused.send_as(Who::Bob, Device, Audience::Node, create("c", Some("p")));
    assert_eq!(child, Err(NodeRefusal::NotAMember));
    // Control: the owner's Device accepts, and Bob is a member.
    w.send_as(Who::Alice, Device, Audience::Node, answer(id, true))
        .unwrap();
    assert_eq!(members(&w), 2);
}

#[test]
fn only_the_owner_and_admitted_members_create_child_tasks() {
    let mut w = invited();
    let early = w.send_as(Who::Bob, Controller, Audience::Node, create("c", Some("p")));
    assert_eq!(early, Err(NodeRefusal::NotAMember));
    let (id, ok) = bob_joins(&mut w, Some(1), NOW);
    ok.unwrap();
    let pending = w.send_as(Who::Bob, Controller, Audience::Node, create("c", Some("p")));
    assert_eq!(
        pending,
        Err(NodeRefusal::NotAMember),
        "a request is not membership"
    );
    w.send_as(Who::Alice, Device, Audience::Node, answer(id, true))
        .unwrap();
    // Controls: the admitted member and the owner both may.
    w.send_as(Who::Bob, Controller, Audience::Node, create("c", Some("p")))
        .unwrap();
    w.send_as(Who::Alice, Device, Audience::Node, create("d", Some("c")))
        .unwrap();
}

fn pending_for(w: &mut World, cid: Cid) -> Vec<PendingJoin> {
    let ask = SwMessage::Unsigned(Payload::JoinPending(JoinPending {
        project: None,
        requests: None,
    }));
    let effects = w.node.received(cid, &ask.to_bytes().unwrap(), NOW).unwrap();
    let sent = w.apply(effects);
    let [(to, bytes)] = sent.as_slice() else {
        panic!("one answer: {sent:?}")
    };
    assert_eq!(*to, cid);
    match SwMessage::from_bytes(bytes).unwrap() {
        SwMessage::Unsigned(Payload::JoinPending(j)) => j.requests.unwrap(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_owner_reads_pending_requests_and_nobody_else_does() {
    let mut w = invited();
    let (id, ok) = bob_joins(&mut w, Some(1), NOW);
    ok.unwrap();
    let (a, b) = (w.alice.device.cid, w.bob.device.cid);
    let alice = pending_for(&mut w, a);
    assert_eq!(alice.len(), 1);
    assert_eq!(alice[0].request_id, id);
    assert_eq!(alice[0].person, w.bob.identity.id_fp());
    assert!(pending_for(&mut w, b).is_empty());
}

#[test]
fn a_refused_write_is_receipted_with_its_reason_and_other_types_are_not() {
    let mut w = invited();
    let bob = std::mem::replace(&mut w.bob, World::new().bob);
    let bytes = w
        .envelope(&bob, &bob.device, Audience::Node, join(Some(9)))
        .to_bytes()
        .unwrap();
    w.bob = bob;
    let refusal = w.node.received(w.bob.device.cid, &bytes, NOW).unwrap_err();
    let sent = w.apply(w.node.refusal_receipt(w.bob.device.cid, &bytes, &refusal));
    let SwMessage::Unsigned(Payload::PadReceipt(r)) = SwMessage::from_bytes(&sent[0].1).unwrap()
    else {
        panic!("a receipt")
    };
    assert_eq!((r.typ, r.stored, r.project.as_str()), (8, false, "p"));
    assert_eq!(r.reason.as_deref(), Some("InviteInvalid"));
    let status = Payload::StatusUpdate(StatusUpdate {
        agent_id: "a".into(),
        task_id: "p".into(),
        lane: sw_model::Lane::Running,
        blockers: vec![],
        harness: "h".into(),
    });
    let bob = std::mem::replace(&mut w.bob, World::new().bob);
    let bytes = w
        .envelope(&bob, &bob.device, Audience::Node, status)
        .to_bytes()
        .unwrap();
    w.bob = bob;
    let refusal = w.node.received(w.bob.device.cid, &bytes, NOW).unwrap_err();
    assert!(
        w.node
            .refusal_receipt(w.bob.device.cid, &bytes, &refusal)
            .is_empty()
    );
}
