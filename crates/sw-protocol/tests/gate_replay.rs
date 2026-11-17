#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Replay, nonce, seq, time and tamper rules (sw-wire contract §2).

mod support;

use support::*;
use sw_protocol::body::Audience;
use sw_protocol::gate::Refusal;
use sw_protocol::replay::ReplayGuard;
use sw_testkit::{Cast, FIXTURE_NOW_MS as NOW};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn a_valid_envelope_verifies_once_and_its_replay_is_refused() {
    let cast = Cast::new();
    let mut replay = ReplayGuard::empty();
    let m = device_status(&cast, 1, 7);
    at_node(&cast, &m, &mut replay, cast.alice_device_cid).unwrap();
    assert_eq!(
        at_node(&cast, &m, &mut replay, cast.alice_device_cid),
        Err(Refusal::Replay)
    );
}

#[test]
fn a_reused_nonce_with_a_new_seq_is_a_replay() {
    let cast = Cast::new();
    let mut replay = ReplayGuard::empty();
    at_node(
        &cast,
        &device_status(&cast, 1, 7),
        &mut replay,
        cast.alice_device_cid,
    )
    .unwrap();
    let again = device_status(&cast, 2, 7);
    assert_eq!(
        at_node(&cast, &again, &mut replay, cast.alice_device_cid),
        Err(Refusal::Replay)
    );
}

#[test]
fn seq_must_strictly_increase_per_kid() {
    let cast = Cast::new();
    let mut replay = ReplayGuard::empty();
    let dev = cast.alice_device_cid;
    at_node(&cast, &device_status(&cast, 5, 1), &mut replay, dev).unwrap();
    for seq in [5, 4] {
        let m = device_status(&cast, seq, seq as u8 + 10);
        assert_eq!(
            at_node(&cast, &m, &mut replay, dev),
            Err(Refusal::SeqNotIncreasing)
        );
    }
    at_node(&cast, &device_status(&cast, 6, 2), &mut replay, dev).unwrap();
}

#[test]
fn a_forged_envelope_cannot_poison_the_replay_state() {
    let cast = Cast::new();
    let mut replay = ReplayGuard::empty();
    let forged = tamper(&device_status(&cast, 9, 3), |s| s.sig.0[10] ^= 1);
    let dev = cast.alice_device_cid;
    assert_eq!(
        at_node(&cast, &forged, &mut replay, dev),
        Err(Refusal::BadSignature)
    );
    assert_eq!(replay.tracked_nonces(), 0);
    at_node(&cast, &device_status(&cast, 9, 3), &mut replay, dev).unwrap();
}

#[test]
fn flipped_body_or_signature_byte_is_bad_signature() {
    let cast = Cast::new();
    let dev = cast.alice_device_cid;
    for m in [
        tamper(&device_status(&cast, 1, 1), |s| s.body.0[5] ^= 1),
        tamper(&device_status(&cast, 1, 1), |s| s.sig.0[0] ^= 1),
    ] {
        assert_eq!(
            at_node(&cast, &m, &mut ReplayGuard::empty(), dev),
            Err(Refusal::BadSignature)
        );
    }
}

#[test]
fn time_rules() {
    let cast = Cast::new();
    let day = 24 * 60 * 60 * 1000;
    let cases = [
        (NOW + 61_000, NOW + 120_000, Refusal::FromFuture),
        (NOW - 120_000, NOW, Refusal::Expired),
        (NOW, NOW + day + 1, Refusal::LifetimeTooLong),
    ];
    for (iat, exp, refusal) in cases {
        let mut b = body(&cast, cast.alice_device_cid, Audience::Node, 1, 1, status());
        (b.iat, b.exp) = (iat, exp);
        let m = sign(&b, &cast.alice_device);
        let got = at_node(&cast, &m, &mut ReplayGuard::empty(), cast.alice_device_cid);
        assert_eq!(got, Err(refusal));
    }
}

#[test]
fn short_lived_types_are_held_to_five_minutes() {
    let cast = Cast::new();
    let ctl = cast.alice_controller_cid;
    let mut b = body(&cast, ctl, Audience::Node, 1, 1, revoke(&cast));
    b.exp = NOW + 5 * 60 * 1000 + 1;
    let m = sign(&b, &cast.alice);
    assert_eq!(
        at_node(&cast, &m, &mut ReplayGuard::empty(), ctl),
        Err(Refusal::LifetimeTooLong)
    );
}
