#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The Task tree and the tier transitions through the node, and the significant events they
//! push (design §2.5, §4.7): open up → request → admit → leave, each told to the right people.

mod support;

use support::rules::notified;
use support::*;
use sw_model::EndpointKind::{Controller, Device};
use sw_model::tier::{JoinPolicy, Tier, TierName};
use sw_model::{Lane, Visibility};
use sw_node::core::NodeRefusal;
use sw_protocol::body::Audience;
use sw_protocol::payload::*;

fn create(id: &str, parent: Option<&str>) -> Payload {
    Payload::TaskWrite(TaskWrite {
        op: TaskOp::Create,
        task_id: id.into(),
        fields: TaskFields {
            title: Some(format!("task {id}")),
            visibility: Some(Visibility::Members),
            parent: parent.map(str::to_owned),
            ..TaskFields::EMPTY
        },
    })
}

fn open_up(id: &str) -> Payload {
    Payload::TaskWrite(TaskWrite {
        op: TaskOp::Update,
        task_id: id.into(),
        fields: TaskFields {
            tier: Some(TierName::Aware),
            join_policy: Some(JoinPolicy::Manual),
            ..TaskFields::EMPTY
        },
    })
}

fn status(task: &str, lane: Lane) -> Payload {
    Payload::StatusUpdate(StatusUpdate {
        agent_id: "bob-agent".into(),
        task_id: task.into(),
        lane,
        blockers: vec![],
        harness: "claude-code".into(),
    })
}

/// Alice owns project `p` with task `t` opened up; Bob has asked to join.
type Sent = Vec<(u64, Vec<u8>)>;

fn requested() -> (World, [u8; 32], Sent) {
    let mut w = World::enrolled();
    w.send_as(Who::Alice, Controller, Audience::Node, create("p", None))
        .unwrap();
    w.send_as(
        Who::Alice,
        Controller,
        Audience::Node,
        create("t", Some("p")),
    )
    .unwrap();
    w.send_as(Who::Alice, Controller, Audience::Node, open_up("t"))
        .unwrap();
    let request = Payload::JoinRequest(JoinRequest {
        task_id: "t".into(),
        note: "I can take the tests".into(),
        readiness: sw_model::readiness::Readiness::from_permille(1000).unwrap(),
        agent_id: None,
        invite: None,
    });
    let bob = std::mem::replace(&mut w.bob, World::new().bob);
    let m = w.envelope(&bob, &bob.controller, Audience::Node, request);
    let signal_id = match &m {
        sw_protocol::signed::SwMessage::Signed { signed, .. } => signed.signal_id().unwrap(),
        _ => unreachable!(),
    };
    let effects = w
        .node
        .received(bob.controller.cid, &m.to_bytes().unwrap(), NOW)
        .unwrap();
    w.bob = bob;
    let sent = w.apply(effects);
    (w, signal_id, sent)
}

#[test]
fn a_join_request_is_told_to_the_owner_only() {
    let (w, _, sent) = requested();
    let to: Vec<_> = notified(&sent);
    assert_eq!(
        to,
        vec![
            (w.alice.controller.cid, "join_requested".into()),
            (w.alice.device.cid, "join_requested".into())
        ]
    );
}

#[test]
fn admission_synchronizes_the_task_and_a_finished_coworker_leaves_it() {
    let (mut w, request_id, _) = requested();
    let answer = Payload::JoinAnswer(JoinAnswer {
        request_id,
        accept: true,
        reason: None,
    });
    let sent = w
        .send_as(Who::Alice, Controller, Audience::Node, answer)
        .unwrap();
    let kinds = notified(&sent);
    for cid in [
        w.alice.controller.cid,
        w.bob.controller.cid,
        w.bob.device.cid,
    ] {
        assert!(
            kinds.contains(&(cid, "person_joined_task".into())),
            "{kinds:?}"
        );
    }
    assert!(matches!(
        w.node.task("t").unwrap().coop.tier,
        Tier::Synchronized { .. }
    ));

    let sent = w
        .send_as(Who::Bob, Device, Audience::Node, status("t", Lane::Running))
        .unwrap();
    assert!(notified(&sent).contains(&(w.alice.controller.cid, "task_claimed".into())));
    let sent = w
        .send_as(Who::Bob, Device, Audience::Node, status("t", Lane::Done))
        .unwrap();
    let kinds = notified(&sent);
    assert!(
        kinds.contains(&(w.alice.device.cid, "person_left_task".into())),
        "{kinds:?}"
    );
    assert!(
        kinds.contains(&(w.bob.device.cid, "person_left_task".into())),
        "{kinds:?}"
    );
    assert!(matches!(
        w.node.task("t").unwrap().coop.tier,
        Tier::Aware { .. }
    ));
}

#[test]
fn only_the_owner_answers_and_a_stranger_cannot_work() {
    let (mut w, request_id, _) = requested();
    let answer = Payload::JoinAnswer(JoinAnswer {
        request_id,
        accept: true,
        reason: None,
    });
    let refused = w.send_as(Who::Bob, Controller, Audience::Node, answer);
    assert!(matches!(refused, Err(NodeRefusal::Tier(_))), "{refused:?}");
    let refused = w.send_as(Who::Bob, Device, Audience::Node, status("t", Lane::Running));
    assert_eq!(refused, Err(NodeRefusal::NotAWorker));
}

#[test]
fn a_session_is_not_a_signature_and_the_tree_keeps_its_shape() {
    let mut w = World::enrolled();
    let orphan = w.send_as(
        Who::Alice,
        Controller,
        Audience::Node,
        create("c", Some("nope")),
    );
    assert_eq!(orphan, Err(NodeRefusal::NoSuchTask));
    w.send_as(Who::Alice, Controller, Audience::Node, create("p", None))
        .unwrap();
    w.send_as(
        Who::Alice,
        Controller,
        Audience::Node,
        create("c", Some("p")),
    )
    .unwrap();
    let delete = |id: &str| {
        Payload::TaskWrite(TaskWrite {
            op: TaskOp::Delete,
            task_id: id.into(),
            fields: TaskFields::EMPTY,
        })
    };
    assert_eq!(
        w.send_as(Who::Alice, Controller, Audience::Node, delete("p")),
        Err(NodeRefusal::HasChildren)
    );
    assert!(
        w.send_as(Who::Bob, Controller, Audience::Node, delete("c"))
            .is_err()
    );
    w.send_as(Who::Alice, Controller, Audience::Node, delete("c"))
        .unwrap();
    assert!(!w.stored.contains_key("task/c"));
}
