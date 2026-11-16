//! Envelope builders shared by the gate tests.
#![allow(dead_code, clippy::unwrap_used)]

use sw_model::{Id128, Lane};
use sw_protocol::body::{Audience, Body};
use sw_protocol::gate::{Refusal, Verifier, verify};
use sw_protocol::payload::*;
use sw_protocol::ports::MessageSigner;
use sw_protocol::replay::ReplayGuard;
use sw_protocol::sealed::Sealed;
use sw_protocol::signed::Signed;
use sw_testkit::{Cast, FIXTURE_NOW_MS as NOW, LabelledEntropy, StandInIdentityPort as ID};

pub const NOW_MS: u64 = NOW;

pub fn status() -> Payload {
    Payload::StatusUpdate(StatusUpdate {
        agent_id: Id128([1; 16]),
        task_id: Id128([2; 16]),
        lane: Lane::Running,
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

pub fn device_status(cast: &Cast, seq: u64, nonce: u8) -> Signed {
    let b = body(
        cast,
        cast.alice_device_cid,
        Audience::Node,
        seq,
        nonce,
        status(),
    );
    Signed::sign(&b, &cast.alice_device).unwrap()
}

pub fn at_node(
    cast: &Cast,
    s: &Signed,
    replay: &mut ReplayGuard,
    from: u64,
) -> Result<(), Refusal> {
    let v = Verifier::Node { session_cid: from };
    verify(s, &cast.node_directory(), &ID, replay, v, NOW).map(|_| ())
}

pub fn enrol(cast: &Cast) -> Payload {
    Payload::Enrol(Enrol {
        op: EnrolOp::Revoke,
        endpoint_cid: cast.alice_device_cid,
        kind: sw_model::EndpointKind::Device,
        epoch: 2,
        signing_pk: None,
        sealing_pk: None,
        kid: Some(sw_crypto::kid_of(cast.alice_device.public_key())),
    })
}

pub fn prompt(cast: &Cast, aud_device: u64, by: &dyn MessageSigner, from: u64) -> Signed {
    let inner = PromptInner {
        task_id: Id128([2; 16]),
        agent_id: None,
        text: "go".into(),
        mode: PromptMode::Queue,
        reply_to: None,
    };
    let mut e = LabelledEntropy::new("gate-test");
    let sealed = Sealed::seal(
        cast.alice_device_kem.encapsulation_key(),
        aud_device,
        &inner,
        &mut e,
    )
    .unwrap();
    let mut b = body(
        cast,
        from,
        Audience::Device(aud_device),
        1,
        4,
        Payload::Prompt(SealedPayload { sealed }),
    );
    b.exp = NOW + 60 * 60 * 1000;
    Signed::sign(&b, by).unwrap()
}
