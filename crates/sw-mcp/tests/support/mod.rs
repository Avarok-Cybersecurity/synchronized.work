//! Shared builders for the sw-mcp tests: the C0 KAT cast, its fixture envelopes, and signals
//! signed by the KAT Controller and sealed to the KAT Device.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

pub mod device;
pub mod node;
pub mod pad_sender;
pub mod pad_world;
pub mod sign_in;

use serde_json::Value as Json;
use sw_mcp::adapters::CitadelIdentity;
use sw_mcp::gate::{DeviceGate, GateContext, GateLimits, Outcome};
use sw_mcp::trust::TrustList;
use sw_protocol::body::{Audience, Body};
use sw_protocol::payload::{ControlInner, ControlOp, Payload, PromptInner, PromptMode};
use sw_protocol::sealed::Sealed;
use sw_protocol::signed::SwMessage;
use sw_testkit::LabelledEntropy;
use sw_testkit::kat::Kat;

pub const WIRE_KAT: &str = include_str!("../../../../fixtures/sw-wire-kat.json");
pub const SIGN_IN_KAT: &str = include_str!("../../../../fixtures/sign-in-kat.json");
pub const PROMPT_FIXTURE: &str = include_str!("../../../../fixtures/sw-wire/04-prompt-sealed.json");

/// The prompt fixture's `iat`; its `exp` is a day later.
pub const FIXTURE_IAT: u64 = 1_794_700_000_000;
/// One minute into the fixture's lifetime.
pub const NOW: u64 = FIXTURE_IAT + 60_000;
pub const HOUR_MS: u64 = 3_600_000;

pub fn kat() -> Kat {
    Kat::parse(WIRE_KAT).expect("the C0 sw-wire KAT parses")
}

pub fn fixture_prompt() -> Vec<u8> {
    let j: Json = serde_json::from_str(PROMPT_FIXTURE).unwrap();
    data_encoding::HEXLOWER
        .decode(j["cbor_hex"].as_str().unwrap().as_bytes())
        .unwrap()
}

/// The KAT Controller, trusted `trusted_ago_ms` before `NOW`.
pub fn trusted(kat: &Kat, trusted_ago_ms: u64) -> TrustList {
    let mut list = TrustList::default();
    list.trust(
        kat.controller.public_key(),
        kat.controller_cid,
        kat.person.clone(),
        "laptop browser".into(),
        NOW - trusted_ago_ms,
    )
    .unwrap();
    list
}

/// Limits roomy enough that only the limit under test can bite.
pub fn limits() -> GateLimits {
    GateLimits {
        per_kid_per_min: 10,
        pending_max: 50,
        refusals_before_pause: 3,
        approval_window_ms: 24 * HOUR_MS,
    }
}

pub fn gate(limits: GateLimits) -> DeviceGate {
    DeviceGate::new(limits, Vec::new())
}

fn signed(kat: &Kat, seq: u64, nonce: u8, iat: u64, payload: Payload) -> Vec<u8> {
    let body = Body {
        iss: kat.person.clone(),
        from: kat.controller_cid,
        aud: Audience::Endpoint(kat.device_cid),
        iat,
        exp: iat + HOUR_MS,
        nonce: [nonce; 16],
        seq,
        payload,
    };
    SwMessage::signed(&body, &kat.controller)
        .unwrap()
        .to_bytes()
        .unwrap()
}

/// A `PROMPT` for `task-42`, sealed to the Device whose CID is `inner_aud`.
pub fn prompt(kat: &Kat, seq: u64, nonce: u8, inner_aud: u64) -> Vec<u8> {
    let inner = PromptInner {
        aud: inner_aud,
        task_id: "task-42".into(),
        text: format!("prompt number {seq}"),
        mode: PromptMode::Queue,
        agent_id: None,
        reply_to: None,
    };
    let mut entropy = LabelledEntropy::new(&format!("prompt-{seq}-{nonce}"));
    let sealed = Sealed::seal(kat.device_kem.encapsulation_key(), &inner, &mut entropy).unwrap();
    signed(kat, seq, nonce, NOW, Payload::Prompt(sealed))
}

pub fn control(kat: &Kat, seq: u64, nonce: u8, op: ControlOp) -> Vec<u8> {
    let id = matches!(op, ControlOp::ApproveJoin | ControlOp::RefuseJoin).then_some([7u8; 32]);
    let reason = (op == ControlOp::RefuseJoin).then(|| "no".to_owned());
    let inner = ControlInner {
        aud: kat.device_cid,
        op,
        tier: None,
        id,
        reason,
    };
    let mut entropy = LabelledEntropy::new(&format!("control-{seq}"));
    let sealed = Sealed::seal(kat.device_kem.encapsulation_key(), &inner, &mut entropy).unwrap();
    signed(kat, seq, nonce, NOW, Payload::Control(sealed))
}

/// Runs the gate as the KAT Device with an agent attached to `task-42`.
pub fn admit(
    gate: &mut DeviceGate,
    kat: &Kat,
    trust: &TrustList,
    bytes: &[u8],
    now: u64,
    pending: usize,
) -> Outcome {
    let attached = |task: Option<&str>, _agent: Option<&str>| task.is_none_or(|t| t == "task-42");
    admit_with(gate, kat, trust, bytes, now, pending, &attached)
}

/// As [`admit`], with the attached-agent answer given.
pub fn admit_with(
    gate: &mut DeviceGate,
    kat: &Kat,
    trust: &TrustList,
    bytes: &[u8],
    now: u64,
    pending: usize,
    attached: &dyn Fn(Option<&str>, Option<&str>) -> bool,
) -> Outcome {
    let ctx = GateContext {
        trust,
        kem: &kat.device_kem,
        identity: &CitadelIdentity,
        own_cid: kat.device_cid,
        now_ms: now,
        pending,
        attached,
    };
    gate.admit(bytes, &ctx)
}

pub fn reason(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Admitted(_) => "admitted".into(),
        Outcome::Refused(r) => r.reason.code(),
    }
}
