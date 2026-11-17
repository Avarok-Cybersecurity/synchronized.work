//! Envelope builders shared by the gate tests.
#![allow(dead_code, clippy::unwrap_used)]

use sw_protocol::body::{Audience, Body};
use sw_protocol::gate::{Refusal, Verifier, verify};
use sw_protocol::payload::*;
use sw_protocol::ports::MessageSigner;
use sw_protocol::replay::ReplayGuard;
use sw_protocol::sealed::Sealed;
use sw_protocol::signed::SwMessage;
use sw_testkit::{Cast, FIXTURE_NOW_MS as NOW, LabelledEntropy, StandInIdentityPort as ID};

pub fn status() -> Payload {
    Payload::StatusUpdate(StatusUpdate {
        agent_id: "agent-1".into(),
        task_id: "task-1".into(),
        lane: sw_model::Lane::Running,
        blockers: vec![],
        harness: "h".into(),
    })
}

pub fn body(cast: &Cast, from: u64, aud: Audience, seq: u64, nonce: u8, payload: Payload) -> Body {
    Body {
        iss: cast.alice.id_fp(),
        from,
        aud,
        iat: NOW,
        exp: NOW + 60_000,
        nonce: [nonce; 16],
        seq,
        payload,
    }
}

pub fn sign(body: &Body, key: &dyn MessageSigner) -> SwMessage {
    SwMessage::signed(body, key).unwrap()
}

pub fn device_status(cast: &Cast, seq: u64, nonce: u8) -> SwMessage {
    sign(
        &body(
            cast,
            cast.alice_device_cid,
            Audience::Node,
            seq,
            nonce,
            status(),
        ),
        &cast.alice_device,
    )
}

pub fn at_node(
    cast: &Cast,
    m: &SwMessage,
    replay: &mut ReplayGuard,
    from: u64,
) -> Result<(), Refusal> {
    let v = Verifier::Node {
        session_cid: from,
        sign_in_issuer: cast.sign_in_issuer.public_key(),
    };
    verify(m, &cast.node_directory(), &ID, replay, v, NOW).map(|_| ())
}

/// Applies `f` to the `Signed` envelope inside a signed message.
pub fn tamper(m: &SwMessage, f: impl FnOnce(&mut sw_protocol::signed::Signed)) -> SwMessage {
    let mut m = m.clone();
    if let SwMessage::Signed { signed, .. } = &mut m {
        f(signed);
    }
    m
}

pub fn revoke(cast: &Cast) -> Payload {
    Payload::Enrol(Enrol {
        op: EnrolOp::Revoke,
        epoch: 2,
        endpoint_cid: None,
        kind: None,
        pubkeys: None,
        kid: Some(sw_crypto::kid_of(cast.alice_device.public_key())),
        assertion: None,
    })
}

pub fn prompt(cast: &Cast, aud_device: u64, by: &dyn MessageSigner, from: u64) -> SwMessage {
    let inner = PromptInner {
        aud: aud_device,
        task_id: "task-1".into(),
        text: "go".into(),
        mode: PromptMode::Queue,
        agent_id: None,
        reply_to: None,
    };
    let mut e = LabelledEntropy::new("gate-test");
    let sealed = Sealed::seal(cast.alice_device_kem.encapsulation_key(), &inner, &mut e).unwrap();
    let mut b = body(
        cast,
        from,
        Audience::Endpoint(aud_device),
        1,
        4,
        Payload::Prompt(sealed),
    );
    b.exp = NOW + 60 * 60 * 1000;
    sign(&b, by)
}
