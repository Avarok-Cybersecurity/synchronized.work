#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Viewers (design §4.6, §6.5): `SUBSCRIBE` catches up from a cursor, live `EVENT`s reach
//! subscribed sessions only, each rendered through the one visibility rule, and a session's
//! subscriptions end with it.

mod feed_support;
mod support;

use feed_support::*;
use support::*;
use sw_client::live::{Folded, ProjectState};
use sw_model::readiness::{Blocker, BlockerKind};
use sw_model::{EndpointKind, Lane, Visibility};
use sw_node::core::NodeRefusal;
use sw_protocol::body::Audience;
use sw_protocol::payload::*;
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Wire;

fn ids(list: &[(u64, &str)]) -> Vec<(u64, String)> {
    list.iter().map(|(c, i)| (*c, (*i).to_owned())).collect()
}

#[test]
fn catch_up_from_a_cursor_replays_exactly_the_later_entries_once() {
    let mut w = World::enrolled();
    for (id, parent) in [
        ("p", None),
        ("a", Some("p")),
        ("b", Some("p")),
        ("c", Some("p")),
    ] {
        alice(&mut w, create(id, parent, Visibility::Members));
    }
    let me = w.alice.controller.cid;
    let all = ids(&[(1, "p"), (2, "a"), (3, "b"), (4, "c")]);
    assert_eq!(
        task_ids(&events(&subscribe(&mut w, me, "p", 0).unwrap(), me)),
        all
    );
    let later = events(&subscribe(&mut w, me, "p", 2).unwrap(), me);
    assert_eq!(task_ids(&later), ids(&[(3, "b"), (4, "c")]));

    // A Task changed again moves to the head; nothing is sent twice.
    let retitle = Payload::TaskWrite(TaskWrite {
        op: TaskOp::Update,
        task_id: "a".into(),
        fields: TaskFields {
            title: Some("renamed".into()),
            ..TaskFields::EMPTY
        },
    });
    alice(&mut w, retitle);
    let later = events(&subscribe(&mut w, me, "p", 2).unwrap(), me);
    assert_eq!(task_ids(&later), ids(&[(3, "b"), (4, "c"), (5, "a")]));
    assert!(subscribe(&mut w, me, "p", 5).unwrap().is_empty());
}

#[test]
fn a_live_change_reaches_a_subscribed_viewer_and_not_an_unsubscribed_one() {
    let mut w = World::enrolled();
    alice(&mut w, create("p", None, Visibility::Members));
    let (viewer, other) = (w.alice.controller.cid, w.alice.device.cid);
    subscribe(&mut w, viewer, "p", 0).unwrap();
    let sent = alice(&mut w, create("d", Some("p"), Visibility::Members));
    assert_eq!(task_ids(&events(&sent, viewer)), ids(&[(2, "d")]));
    assert!(
        events(&sent, other).is_empty(),
        "the Device never subscribed"
    );

    unsubscribe(&mut w, viewer, "p");
    let sent = alice(&mut w, create("e", Some("p"), Visibility::Members));
    assert!(events(&sent, viewer).is_empty(), "unsubscribed");
}

fn status(task: &str, blockers: Vec<Blocker>) -> Payload {
    Payload::StatusUpdate(StatusUpdate {
        agent_id: "alice-agent".into(),
        task_id: task.into(),
        lane: Lane::Running,
        blockers,
        harness: "claude-code".into(),
    })
}

#[test]
fn a_viewer_never_receives_what_the_tasks_visibility_hides() {
    let mut w = with_member();
    let (owner, member) = (w.alice.controller.cid, w.bob.controller.cid);
    subscribe(&mut w, owner, "p", 0).unwrap();
    subscribe(&mut w, member, "p", 0).unwrap();
    let sent = alice(&mut w, create("s", Some("p"), Visibility::Owner));
    let (to_owner, to_member) = (events(&sent, owner), events(&sent, member));
    assert_eq!(
        written(&to_owner[0]).fields.title.as_deref(),
        Some("title of s")
    );
    let hidden = written(&to_member[0]).fields;
    assert_eq!(
        (hidden.title, hidden.description),
        (None, None),
        "vis:owner hides the title and body"
    );
    assert_eq!(
        hidden.owner.as_ref(),
        Some(&w.alice.identity.id_fp()),
        "existence is shown"
    );

    let blocker = Blocker {
        kind: BlockerKind::Human,
        text: "needs a decision".into(),
        task_id: None,
    };
    let sent = w
        .send_as(
            Who::Alice,
            EndpointKind::Device,
            Audience::Node,
            status("s", vec![blocker]),
        )
        .unwrap();
    let blockers = |to| {
        let e = &events(&sent, to)[0];
        StatusUpdate::from_value(e.body.payload.0.clone())
            .unwrap()
            .blockers
            .len()
    };
    assert_eq!(
        (blockers(owner), blockers(member)),
        (1, 0),
        "blockers are bodies"
    );
}

#[test]
fn a_sessions_end_drops_its_subscriptions() {
    let mut w = World::enrolled();
    alice(&mut w, create("p", None, Visibility::Members));
    let me = w.alice.controller.cid;
    subscribe(&mut w, me, "p", 0).unwrap();
    w.node.disconnected(me);
    w.connect(me);
    let sent = alice(&mut w, create("d", Some("p"), Visibility::Members));
    assert!(
        events(&sent, me).is_empty(),
        "the new session never subscribed"
    );
}

#[test]
fn a_subscribe_from_anyone_but_a_member_of_a_project_is_refused() {
    let mut w = project();
    let bob = w.bob.controller.cid;
    assert_eq!(subscribe(&mut w, bob, "p", 0), Err(NodeRefusal::NotAMember));
    w.connect(9000);
    assert_eq!(
        subscribe(&mut w, 9000, "p", 0),
        Err(NodeRefusal::UnknownPerson)
    );
    let me = w.alice.controller.cid;
    assert_eq!(subscribe(&mut w, me, "t", 0), Err(NodeRefusal::NotAProject));
    w.node.connected(9001, NOW);
    let bytes = subscribe_bytes(SubscribeOp::Subscribe, "p", 0);
    assert_eq!(
        w.node.received(9001, &bytes, NOW),
        Err(NodeRefusal::NoHello)
    );
    assert!(subscribe(&mut w, me, "p", 0).is_ok(), "the owner may");
}

#[test]
fn subscriptions_are_capped_per_session_and_per_project() {
    let mut w = with_member();
    for root in ["q", "r"] {
        alice(&mut w, create(root, None, Visibility::Members));
    }
    let me = w.alice.controller.cid;
    subscribe(&mut w, me, "p", 0).unwrap();
    subscribe(&mut w, me, "q", 0).unwrap();
    let cap = NodeRefusal::CapReached("subscriptions per session");
    assert_eq!(subscribe(&mut w, me, "r", 0), Err(cap));
    assert!(
        subscribe(&mut w, me, "p", 0).is_ok(),
        "subscribing again is not another"
    );
    for cid in [w.alice.device.cid, w.bob.controller.cid] {
        subscribe(&mut w, cid, "p", 0).unwrap();
    }
    let cap = NodeRefusal::CapReached("subscribers per project");
    let last = w.bob.device.cid;
    assert_eq!(subscribe(&mut w, last, "p", 0), Err(cap));
}

#[test]
fn the_marks_survive_a_restart() {
    let mut w = with_member();
    let me = w.alice.controller.cid;
    let before = subscribe(&mut w, me, "p", 0).unwrap();
    w.node = restarted(&w);
    w.connect(me);
    let after = subscribe(&mut w, me, "p", 0).unwrap();
    assert!(!before.is_empty());
    assert_eq!(before, after, "the same entries, cursors and authors");
}

#[test]
fn what_the_node_sends_is_what_a_viewers_fold_consumes() {
    let mut w = with_member();
    let me = w.bob.controller.cid;
    let mut sent = subscribe(&mut w, me, "p", 0).unwrap();
    sent.extend(
        w.send_as(
            Who::Alice,
            EndpointKind::Device,
            Audience::Node,
            status("t", vec![]),
        )
        .unwrap(),
    );
    let mut state = ProjectState::new("p");
    for (_, bytes) in sent.iter().filter(|(to, _)| *to == me) {
        let folded = state.fold(&SwMessage::from_bytes(bytes).unwrap()).unwrap();
        assert_eq!(folded, Folded::Applied);
    }
    assert_eq!(state.tasks.keys().collect::<Vec<_>>(), ["p", "t"]);
    assert_eq!(state.agents["alice-agent"].task_id, "t");
    assert_eq!(state.tasks["t"].title.as_deref(), Some("title of t"));
}
