#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The gate's limits and modes: the per-kid rate, the pending bound, approval mode for a new
//! Controller, pausing after repeated refusals, and which controls reach an agent.

mod support;

use support::*;
use sw_mcp::gate::Outcome;
use sw_protocol::payload::ControlOp;

#[test]
fn the_rate_limit_refuses_past_the_per_kid_budget_and_recovers_after_a_minute() {
    let kat = kat();
    let trust = trusted(&kat, 48 * HOUR_MS);
    let mut l = limits();
    l.per_kid_per_min = 3;
    l.refusals_before_pause = 100;
    let mut g = gate(l);
    for seq in 1..=3 {
        let m = prompt(&kat, seq, seq as u8, kat.device_cid);
        assert_eq!(
            reason(&admit(&mut g, &kat, &trust, &m, NOW, 0)),
            "admitted",
            "signal {seq}"
        );
    }
    let fourth = prompt(&kat, 4, 4, kat.device_cid);
    assert_eq!(
        reason(&admit(&mut g, &kat, &trust, &fourth, NOW, 0)),
        "rate_limited"
    );
    let fifth = prompt(&kat, 5, 5, kat.device_cid);
    assert_eq!(
        reason(&admit(&mut g, &kat, &trust, &fifth, NOW + 60_000, 0)),
        "admitted"
    );
}

#[test]
fn the_pending_bound_refuses_when_the_inbox_is_full() {
    let kat = kat();
    let trust = trusted(&kat, 48 * HOUR_MS);
    let m = prompt(&kat, 1, 1, kat.device_cid);
    assert_eq!(
        reason(&admit(&mut gate(limits()), &kat, &trust, &m, NOW, 49)),
        "admitted"
    );
    assert_eq!(
        reason(&admit(&mut gate(limits()), &kat, &trust, &m, NOW, 50)),
        "inbox_full"
    );
}

#[test]
fn a_new_controller_is_held_for_approval_for_the_configured_window() {
    let kat = kat();
    let fresh = trusted(&kat, 23 * HOUR_MS);
    let Outcome::Admitted(a) = admit(&mut gate(limits()), &kat, &fresh, &fixture_prompt(), NOW, 0)
    else {
        panic!("refused")
    };
    assert!(a.needs_approval, "trusted 23 h ago: inside the 24 h window");
    let old = trusted(&kat, 25 * HOUR_MS);
    let Outcome::Admitted(a) = admit(&mut gate(limits()), &kat, &old, &fixture_prompt(), NOW, 0)
    else {
        panic!("refused")
    };
    assert!(!a.needs_approval);
}

#[test]
fn repeated_refusals_pause_the_controller_and_a_paused_one_is_refused() {
    let kat = kat();
    let mut trust = trusted(&kat, 48 * HOUR_MS);
    let mut g = gate(limits());
    let mut pause = false;
    for seq in 1..=3 {
        let m = prompt(&kat, seq, seq as u8, kat.device_cid + 1);
        let Outcome::Refused(r) = admit(&mut g, &kat, &trust, &m, NOW, 0) else {
            panic!()
        };
        pause = r.pause;
    }
    assert!(pause, "the third refusal inside a minute pauses the kid");
    trust.pause(&sw_crypto::kid_of(kat.controller.public_key()));
    let good = prompt(&kat, 10, 10, kat.device_cid);
    assert_eq!(
        reason(&admit(&mut g, &kat, &trust, &good, NOW, 0)),
        "controller_paused"
    );
}

#[test]
fn pause_resume_stop_reach_an_agent_and_join_answers_are_refused_until_built() {
    let kat = kat();
    let trust = trusted(&kat, 48 * HOUR_MS);
    for (seq, op) in [
        (1, ControlOp::Pause),
        (2, ControlOp::Resume),
        (3, ControlOp::Stop),
    ] {
        let m = control(&kat, seq, seq as u8, op);
        assert_eq!(
            reason(&admit(&mut gate(limits()), &kat, &trust, &m, NOW, 0)),
            "admitted",
            "{op:?}"
        );
    }
    let join = control(&kat, 4, 4, ControlOp::ApproveJoin);
    assert_eq!(
        reason(&admit(&mut gate(limits()), &kat, &trust, &join, NOW, 0)),
        "unsupported_op"
    );
}
