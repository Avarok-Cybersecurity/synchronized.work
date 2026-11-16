#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The two paths beside the inbox: ENROL by assertion, and P2P (sw-wire contract §3).

mod support;

use support::*;
use sw_protocol::body::Audience;
use sw_protocol::gate::{Refusal, Verifier, verify, verify_p2p};
use sw_protocol::replay::ReplayGuard;
use sw_protocol::signed::SwMessage;
use sw_testkit::{Cast, FIXTURE_NOW_MS as NOW, StandInIdentityPort as ID};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn enrol_by_assertion_is_self_signed_and_hands_the_assertion_to_the_node() {
    let cast = Cast::new();
    let new_cid = 7001;
    let dir = sw_testkit::Directory::default(); // the enrolling key is not known yet
    let b = body(
        &cast,
        new_cid,
        Audience::Node,
        1,
        1,
        enrol_by_assertion(&cast, new_cid),
    );
    let m = sign(&b, &cast.bob_controller);
    let v = verify(
        &m,
        &dir,
        &ID,
        &mut ReplayGuard::empty(),
        Verifier::Node {
            session_cid: new_cid,
        },
        NOW,
    )
    .unwrap();
    assert_eq!(v.assertion_to_verify, Some(vec![0x01, 0x02]));
    let other_key = sign(&b, &cast.alice_controller);
    let got = verify(
        &other_key,
        &dir,
        &ID,
        &mut ReplayGuard::empty(),
        Verifier::Node {
            session_cid: new_cid,
        },
        NOW,
    );
    assert_eq!(
        got.unwrap_err(),
        Refusal::UnknownKid,
        "signed by a key the payload does not enrol"
    );
    let elsewhere = sign(
        &body(
            &cast,
            7002,
            Audience::Node,
            1,
            1,
            enrol_by_assertion(&cast, new_cid),
        ),
        &cast.bob_controller,
    );
    let got = verify(
        &elsewhere,
        &dir,
        &ID,
        &mut ReplayGuard::empty(),
        Verifier::Node { session_cid: 7002 },
        NOW,
    );
    assert_eq!(got.unwrap_err(), Refusal::FromMismatch);
}

#[test]
fn the_p2p_path_runs_the_same_gate_and_refuses_ineligible_types() {
    let cast = Cast::new();
    let dev = cast.alice_device_cid;
    let p2p = |m: &SwMessage| {
        verify_p2p(
            m,
            &cast.device_allowlist(),
            &ID,
            &mut ReplayGuard::empty(),
            dev,
            NOW,
        )
        .map(|_| ())
    };
    p2p(&prompt(
        &cast,
        dev,
        &cast.alice_controller,
        cast.alice_controller_cid,
    ))
    .unwrap();
    let from_bob = prompt(&cast, dev, &cast.bob_controller, cast.bob_controller_cid);
    assert_eq!(
        p2p(&from_bob),
        Err(Refusal::UnknownKid),
        "the allowlist gates P2P exactly as the inbox"
    );
    assert_eq!(
        p2p(&device_status(&cast, 1, 1)),
        Err(Refusal::NotP2pEligible)
    );
}
