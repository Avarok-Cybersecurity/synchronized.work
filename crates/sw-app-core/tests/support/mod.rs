#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Fixed inputs for the App tests. Every value is chosen for the test, not a default the
//! production code could fall back to (PCND): the theme is a high-contrast test palette, the
//! limits are the manifest's of design §7.1, the text measure a fixed advance per character.

use ciborium::value::Value;
use sw_app_core::*;
use sw_graph::Motion;
use sw_model::IdFp;
use sw_protocol::payload::{Event, EventBody, Notify, PadReceipt, Payload};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Raw;

pub const ALICE: &str = "3foriftckvpoy2rh";
pub const NODE: &str = "node-7";

mod counted;
pub mod fit_checks;
mod fixed;
#[allow(unused_imports)]
pub use counted::Counted;
#[allow(unused_imports)]
pub use fixed::{FULL, config, theme};

/// Half the pixel size per character: a stand-in for the host's `MeasureText`.
#[derive(Debug, Clone, Copy)]
pub struct FixedAdvance;

impl TextMeasure for FixedAdvance {
    fn width(&self, text: &str, px: f32) -> f32 {
        text.chars().count() as f32 * px / 2.0
    }
}

pub fn granted(citadel_session: bool, net_host: bool) -> Granted {
    Granted {
        citadel_session,
        net_host,
        notifications: true,
        storage_user: true,
        storage_local: false,
    }
}

pub fn ctx(granted: Granted, canvas: CanvasAbi, motion: Motion) -> AppCtx {
    AppCtx {
        acting_username: "alice".into(),
        install_id: "inst-0123456789".into(),
        node_id: NODE.into(),
        app_id: "work.synchronized.app".into(),
        scope: Scope::Node {
            node_id: NODE.into(),
        },
        granted,
        canvas,
        size: Size {
            width: 800.0,
            height: 600.0,
        },
        theme: theme(motion),
        locale: "en".into(),
    }
}

pub fn app(granted: Granted, canvas: CanvasAbi) -> App<FixedAdvance> {
    App::new(ctx(granted, canvas, Motion::Lite), config(), FixedAdvance)
}

pub fn t(s: &str) -> Value {
    Value::Text(s.into())
}

pub fn text_map(pairs: Vec<(&str, Value)>) -> Value {
    Value::Map(pairs.into_iter().map(|(k, v)| (t(k), v)).collect())
}

pub fn event(project: &str, cursor: u64, typ: u8, payload: Value) -> Vec<u8> {
    SwMessage::Unsigned(Payload::Event(Event {
        project_id: project.into(),
        cursor,
        kind: "test".into(),
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

pub fn task(project: &str, cursor: u64, id: &str, title: &str) -> Vec<u8> {
    let fields = text_map(vec![("title", t(title)), ("owner", t(ALICE))]);
    let write = text_map(vec![
        ("op", t("create")),
        ("task_id", t(id)),
        ("fields", fields),
    ]);
    event(project, cursor, 10, write)
}

/// An agent of Alice's on `task_id`, then one progress line from it.
pub fn agent_with_line(project: &str, cursor: u64, task_id: &str, line: &str) -> [Vec<u8>; 2] {
    let status = text_map(vec![
        ("agent_id", t("agent-1")),
        ("task_id", t(task_id)),
        ("lane", t("running")),
        ("blockers", Value::Array(vec![])),
        ("harness", t("claude-code")),
    ]);
    let item = text_map(vec![
        ("at", Value::Integer(1u64.into())),
        ("kind", t("note")),
        ("body", t(line)),
    ]);
    let progress = text_map(vec![
        ("agent_id", t("agent-1")),
        ("events", Value::Array(vec![item])),
    ]);
    [
        event(project, cursor, 2, status),
        event(project, cursor + 1, 3, progress),
    ]
}

pub fn notify(id: u8, text: &str) -> Vec<u8> {
    SwMessage::Unsigned(Payload::Notify(Notify {
        event_id: [id; 32],
        kind: "person_joined_task".into(),
        subject: "Ship it".into(),
        text: text.into(),
        at: 1_700_000_000_000,
    }))
    .to_bytes()
    .unwrap()
}

pub fn receipt(id: [u8; 32], stored: bool, project: &str, reason: Option<&str>) -> Vec<u8> {
    SwMessage::Unsigned(Payload::PadReceipt(PadReceipt {
        project: project.into(),
        id,
        typ: 16,
        stored,
        reason: reason.map(str::to_owned),
    }))
    .to_bytes()
    .unwrap()
}

/// The messages the effects send, decoded.
pub fn sent(effects: &[Effect]) -> Vec<Payload> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::SessionSend { body } => match SwMessage::from_bytes(body).unwrap() {
                SwMessage::Unsigned(p) => Some(p),
                other => panic!("the App sent a signed message: {other:?}"),
            },
            _ => None,
        })
        .collect()
}

pub fn draws(effects: &[Effect]) -> usize {
    effects
        .iter()
        .filter(|e| matches!(e, Effect::Draw(_)))
        .count()
}

/// Started on a session, connected, with this saved link (`None`: never linked).
pub fn connected(app: &mut App<FixedAdvance>, record: Option<Vec<u8>>) -> Vec<Effect> {
    let mut out = app.start();
    out.extend(app.handle(Input::LinkLoaded { record }));
    out.extend(app.handle(Input::SessionOpened {
        cid: 0x1234,
        status: HostSessionStatus::Connected,
    }));
    out
}

pub fn linked_to(project: &str) -> Vec<u8> {
    sw_app_core::link::LinkRecord {
        project: Some(project.into()),
    }
    .to_bytes()
    .unwrap()
}
