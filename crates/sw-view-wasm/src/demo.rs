//! The demo Project shown while nobody is signed in. Its people, agent and status are the
//! contract fixtures (`fixtures/sw-wire/02-status-update`, `07-presence`; identities A and B of
//! `identity-kat.json`); the tasks around them are written here. The frames are real
//! `SwMessage` bytes folded by the same [`LiveProject`] a signed-in viewer runs, over a
//! [`ReplayPort`] instead of the Citadel session: the view says "demo" on every screen.

use ciborium::value::Value;
use data_encoding::HEXLOWER;
use sw_client::ReplayPort;
use sw_client::live::{LiveError, LiveProject, ProjectState};
use sw_model::{Cid, IdFp};
use sw_protocol::body::Body;
use sw_protocol::payload::{Event, EventBody, Payload, Presence};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::{Raw, Wire};

pub const DEMO_PROJECT_ID: &str = "demo";
pub const DEMO_PROJECT_NAME: &str = "Demo project";

const STATUS_FIXTURE: &str = include_str!("../../../fixtures/sw-wire/02-status-update.json");
const PRESENCE_FIXTURE: &str = include_str!("../../../fixtures/sw-wire/07-presence.json");
/// Identity B of `identity-kat.json`.
const BOB: &str = "nx4lsn6nz54fudy7";
const BOB_DEVICE: Cid = 61441;

fn fixture(json: &str) -> Result<SwMessage, String> {
    let j: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let hex = j["cbor_hex"].as_str().ok_or("fixture without cbor_hex")?;
    let bytes = HEXLOWER.decode(hex.as_bytes()).map_err(|e| e.to_string())?;
    SwMessage::from_bytes(&bytes).map_err(|e| format!("{e:?}"))
}

fn t(s: &str) -> Value {
    Value::Text(s.into())
}

fn map(pairs: Vec<(&str, Value)>) -> Value {
    Value::Map(pairs.into_iter().map(|(k, v)| (t(k), v)).collect())
}

struct Frames {
    out: Vec<Vec<u8>>,
    cursor: u64,
}

impl Frames {
    fn event(&mut self, typ: u8, from: Cid, iss: &IdFp, payload: Value) -> Result<(), String> {
        self.cursor += 1;
        let event = Payload::Event(Event {
            project_id: DEMO_PROJECT_ID.into(),
            cursor: self.cursor,
            kind: "demo".into(),
            body: EventBody {
                typ,
                from,
                iss: iss.clone(),
                payload: Raw(payload),
            },
        });
        self.push(event)
    }

    fn push(&mut self, payload: Payload) -> Result<(), String> {
        self.out.push(
            SwMessage::Unsigned(payload)
                .to_bytes()
                .map_err(|e| format!("{e:?}"))?,
        );
        Ok(())
    }

    fn task(&mut self, by: &IdFp, id: &str, fields: Vec<(&str, Value)>) -> Result<(), String> {
        let write = map(vec![
            ("op", t("create")),
            ("task_id", t(id)),
            ("fields", map(fields)),
        ]);
        self.event(10, 0, by, write)
    }

    fn status(
        &mut self,
        from: Cid,
        by: &IdFp,
        agent: &str,
        task: &str,
        lane: &str,
        blockers: Vec<Value>,
    ) -> Result<(), String> {
        let s = map(vec![
            ("agent_id", t(agent)),
            ("task_id", t(task)),
            ("lane", t(lane)),
            ("blockers", Value::Array(blockers)),
            ("harness", t("claude-code")),
        ]);
        self.event(2, from, by, s)
    }

    fn line(
        &mut self,
        from: Cid,
        by: &IdFp,
        agent: &str,
        at: u64,
        body: &str,
    ) -> Result<(), String> {
        let item = map(vec![
            ("at", Value::Integer(at.into())),
            ("kind", t("line")),
            ("body", t(body)),
        ]);
        self.event(
            3,
            from,
            by,
            map(vec![
                ("agent_id", t(agent)),
                ("events", Value::Array(vec![item])),
            ]),
        )
    }
}

/// The demo's frames, in the order a node would send them.
pub fn frames() -> Result<Vec<Vec<u8>>, String> {
    let SwMessage::Signed { signed, .. } = fixture(STATUS_FIXTURE)? else {
        return Err("02-status-update is not signed".into());
    };
    let status = Body::from_bytes(&signed.body.0).map_err(|e| format!("{e:?}"))?;
    let Payload::StatusUpdate(update) = &status.payload else {
        return Err("02-status-update carries no STATUS_UPDATE".into());
    };
    let alice = status.iss.clone();
    let bob = IdFp::parse(BOB).map_err(|e| e.to_string())?;
    let mut f = Frames {
        out: Vec::new(),
        cursor: 0,
    };
    f.task(
        &alice,
        &update.task_id,
        vec![
            ("title", t("Ship the synchronized.work site")),
            ("owner", t(alice.as_str())),
            ("tier", t("synchronized")),
            ("status", t("open")),
        ],
    )?;
    f.task(
        &alice,
        "task-44",
        vec![
            ("title", t("Write the MCP install guide")),
            ("owner", t(alice.as_str())),
            ("parent", t(&update.task_id)),
            ("tier", t("off")),
            ("status", t("open")),
        ],
    )?;
    f.task(
        &bob,
        "task-43",
        vec![
            ("title", t("Sign in with Citadel")),
            ("owner", t(bob.as_str())),
            ("tier", t("aware")),
            ("status", t("open")),
        ],
    )?;
    f.event(2, status.from, &alice, update.to_value())?;
    f.line(
        status.from,
        &alice,
        &update.agent_id,
        1,
        "Landing renders at 390 px; 14 checks green",
    )?;
    let dependency = map(vec![
        ("kind", t("dependency")),
        ("text", t("Needs the sign-in return page")),
        ("task_id", t("task-43")),
    ]);
    f.status(
        BOB_DEVICE,
        &bob,
        "agent-8",
        &update.task_id,
        "waiting",
        vec![dependency],
    )?;
    let human = map(vec![
        ("kind", t("human")),
        ("text", t("Owner to approve the issuer allowlist")),
    ]);
    f.status(
        BOB_DEVICE,
        &bob,
        "agent-9",
        "task-43",
        "blocked",
        vec![human],
    )?;
    f.line(
        BOB_DEVICE,
        &bob,
        "agent-9",
        2,
        "Waiting for a decision on the return URL",
    )?;
    f.out.push(
        fixture(PRESENCE_FIXTURE)?
            .to_bytes()
            .map_err(|e| format!("{e:?}"))?,
    );
    f.push(Payload::Presence(Presence {
        device_cid: BOB_DEVICE,
        online: true,
        agents: vec!["agent-8".into(), "agent-9".into()],
        username: format!("{BOB}.workstation@synchronized.work"),
    }))?;
    Ok(f.out)
}

/// The demo folded by the production viewer logic.
pub fn state() -> Result<ProjectState, String> {
    let mut live = LiveProject::new(DEMO_PROJECT_ID, "demo");
    let mut port = ReplayPort::new(0, frames()?);
    live.pump(&mut port)
        .map_err(|e: LiveError| format!("{e:?}"))?;
    Ok(live.state().clone())
}
