#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The P2P path beside the inbox (sw-wire contract §3).

mod support;

use support::*;
use sw_protocol::gate::{Refusal, verify_p2p};
use sw_protocol::replay::ReplayGuard;
use sw_protocol::signed::SwMessage;
use sw_testkit::{Cast, FIXTURE_NOW_MS as NOW, StandInIdentityPort as ID};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

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
