#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The viewer fold over a [`ReplayPort`]: the subscription it sends, what it folds, and that a
//! catch-up replay is never applied twice.

use ciborium::value::Value;
use sw_client::ReplayPort;
use sw_client::live::{Folded, LiveProject, ProjectState};
use sw_model::{IdFp, Lane};
use sw_protocol::payload::{Event, EventBody, Payload, Presence, Subscribe};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Raw;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const ALICE: &str = "3foriftckvpoy2rh";

fn text_map(pairs: Vec<(&str, Value)>) -> Value {
    Value::Map(
        pairs
            .into_iter()
            .map(|(k, v)| (Value::Text(k.into()), v))
            .collect(),
    )
}

fn t(s: &str) -> Value {
    Value::Text(s.into())
}

fn event(project: &str, cursor: u64, typ: u8, payload: Value) -> Vec<u8> {
    SwMessage::Unsigned(Payload::Event(Event {
        project_id: project.into(),
        cursor,
        kind: "demo".into(),
        body: EventBody {
            typ,
            from: 53249,
            iss: IdFp::parse(ALICE).unwrap(),
            payload: Raw(payload),
        },
    }))
    .to_bytes()
    .unwrap()
}

fn task(cursor: u64, id: &str, title: &str) -> Vec<u8> {
    let fields = text_map(vec![("title", t(title)), ("owner", t(ALICE))]);
    let write = text_map(vec![
        ("op", t("create")),
        ("task_id", t(id)),
        ("fields", fields),
    ]);
    event("p1", cursor, 10, write)
}

fn status(cursor: u64, agent: &str, task: &str) -> Vec<u8> {
    let s = text_map(vec![
        ("agent_id", t(agent)),
        ("task_id", t(task)),
        ("lane", t("running")),
        ("blockers", Value::Array(vec![])),
        ("harness", t("claude-code")),
    ]);
    event("p1", cursor, 2, s)
}

#[test]
fn subscribe_sends_a_viewer_hello_then_subscribe_from_the_cursor() {
    let mut live = LiveProject::new("p1", "site@abc");
    let mut port = ReplayPort::new(9, vec![task(4, "t1", "Ship it")]);
    live.pump(&mut port).unwrap();
    live.subscribe(&mut port).unwrap();
    let sent: Vec<SwMessage> = port
        .sent()
        .iter()
        .map(|b| SwMessage::from_bytes(b).unwrap())
        .collect();
    assert!(
        matches!(&sent[0], SwMessage::Unsigned(Payload::Hello(h)) if h.client_build == "site@abc")
    );
    let SwMessage::Unsigned(Payload::Subscribe(Subscribe {
        since_cursor,
        project_id,
        ..
    })) = &sent[1]
    else {
        panic!("second message is not SUBSCRIBE: {:?}", sent[1]);
    };
    assert_eq!((project_id.as_str(), *since_cursor), ("p1", 4));
}

#[test]
fn tasks_agents_progress_and_presence_fold_into_the_state() {
    let presence = SwMessage::Unsigned(Payload::Presence(Presence {
        device_cid: 53249,
        online: true,
        agents: vec!["agent-7".into()],
        username: format!("{ALICE}.macbook-pro@synchronized.work"),
    }))
    .to_bytes()
    .unwrap();
    let progress = text_map(vec![
        ("agent_id", t("agent-7")),
        (
            "events",
            Value::Array(vec![text_map(vec![
                ("at", Value::Integer(1u64.into())),
                ("kind", t("line")),
                ("body", t("tests green")),
            ])]),
        ),
    ]);
    let frames = vec![
        task(1, "task-42", "Ship it"),
        status(2, "agent-7", "task-42"),
        event("p1", 3, 3, progress),
        presence,
    ];
    let mut live = LiveProject::new("p1", "site@abc");
    assert_eq!(live.pump(&mut ReplayPort::new(9, frames)).unwrap(), 4);
    let s: &ProjectState = live.state();
    assert_eq!(s.tasks["task-42"].title.as_deref(), Some("Ship it"));
    let agent = &s.agents["agent-7"];
    assert_eq!(
        (agent.lane, agent.last_line.as_deref(), agent.online),
        (Lane::Running, Some("tests green"), Some(true))
    );
    let alice = &s.persons[&IdFp::parse(ALICE).unwrap()];
    assert_eq!(
        alice.username.as_deref(),
        Some("3foriftckvpoy2rh.macbook-pro@synchronized.work")
    );
}

#[test]
fn a_replayed_event_is_stale_and_changes_nothing() {
    let mut state = ProjectState::new("p1");
    let first = SwMessage::from_bytes(&task(5, "t1", "First")).unwrap();
    assert_eq!(state.fold(&first).unwrap(), Folded::Applied);
    let replay = SwMessage::from_bytes(&task(5, "t1", "Rewritten")).unwrap();
    assert_eq!(state.fold(&replay).unwrap(), Folded::Stale);
    assert_eq!(state.tasks["t1"].title.as_deref(), Some("First"));
}

#[test]
fn another_projects_event_is_ignored() {
    let mut state = ProjectState::new("p2");
    let other = SwMessage::from_bytes(&task(1, "t1", "Not yours")).unwrap();
    assert_eq!(state.fold(&other).unwrap(), Folded::Ignored);
    assert!(state.tasks.is_empty());
}

#[test]
fn a_malformed_frame_stops_the_pump_with_its_error() {
    let mut live = LiveProject::new("p1", "site@abc");
    let err = live
        .pump(&mut ReplayPort::new(9, vec![vec![0xff]]))
        .unwrap_err();
    assert!(matches!(err, sw_client::live::LiveError::Wire(_)));
}
