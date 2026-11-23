#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The challenge's frames (core/challenge/frame.rs) against the workspace protocol's own
//! encoding, recorded in `fixtures/server-identity-challenge/frames.json`.
//!
//! Controls: serde-rename the mirrored `ServerIdentityChallenge` variant → the agent's request is not
//! read; upper-case the domain in `answer_frame` → the answer's bytes differ; drop
//! `deny_unknown_fields` → "an extra field" reads as a challenge.

mod support;

use citadel_identity::ServerKey;
use support::challenge::{KAT_SERVER, domain, frames, kat_server};
use sw_node::core::challenge_frame::{answer_frame, challenge_nonce};
use sw_protocol::payload::{Hello, HelloKind, Payload};
use sw_protocol::signed::SwMessage;

#[test]
fn the_agents_request_is_a_challenge_with_its_nonce() {
    assert_eq!(challenge_nonce(&frames().request), Some(kat_server().nonce));
}

#[test]
fn the_answer_is_byte_for_byte_the_workspace_kernels() {
    let kat = kat_server();
    let key = ServerKey::from_seed(&kat.seed);
    let pk = key.public_key().as_bytes();
    for name in [KAT_SERVER, "synchronized.work"] {
        let sig = key.sign_challenge(&domain(name), kat.cid, &kat.nonce);
        let frame = answer_frame(name, pk, &sig).unwrap();
        assert_eq!(frame, frames().answer(name), "{name}");
    }
}

#[test]
fn no_near_miss_and_no_sw_message_is_a_challenge() {
    for (why, frame) in frames().near_misses {
        assert_eq!(challenge_nonce(&frame), None, "{why}");
    }
    let hello = SwMessage::Unsigned(Payload::Hello(Hello {
        protocol_version: 1,
        endpoint_kind: HelloKind::Controller,
        client_build: "test".into(),
    }));
    assert_eq!(challenge_nonce(&hello.to_bytes().unwrap()), None);
    assert_eq!(challenge_nonce(b""), None);
}
