#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The Device gate against the C0 fixture envelope `04-prompt-sealed` and envelopes signed by
//! the same KAT Controller. Every refusal test runs the accepted case first in the same test,
//! so the envelope is known good and only the property under test differs.

mod support;

use support::*;
use sw_mcp::gate::{Outcome, Reason, SignalKind};
use sw_mcp::trust::TrustList;
use sw_protocol::gate::Refusal;

#[test]
fn the_c0_prompt_fixture_is_admitted_for_a_trusted_controller() {
    let kat = kat();
    let trust = trusted(&kat, 48 * HOUR_MS);
    let out = admit(&mut gate(limits()), &kat, &trust, &fixture_prompt(), NOW, 0);
    let Outcome::Admitted(a) = out else {
        panic!("refused: {out:?}")
    };
    let SignalKind::Prompt(p) = a.kind else {
        panic!("not a prompt")
    };
    assert_eq!(p.text, "Run the tests and report the failing names.");
    assert_eq!(p.task_id, "task-42");
    assert_eq!(a.controller_cid, kat.controller_cid);
    assert!(
        !a.needs_approval,
        "trusted two days ago: outside the 24 h window"
    );
}

#[test]
fn an_unknown_controller_is_refused() {
    let kat = kat();
    let known = trusted(&kat, 48 * HOUR_MS);
    assert_eq!(
        reason(&admit(
            &mut gate(limits()),
            &kat,
            &known,
            &fixture_prompt(),
            NOW,
            0
        )),
        "admitted"
    );
    let empty = TrustList::default();
    let out = admit(&mut gate(limits()), &kat, &empty, &fixture_prompt(), NOW, 0);
    assert_eq!(reason(&out), "unknown_controller");
    let Outcome::Refused(r) = out else {
        unreachable!()
    };
    assert_eq!(
        r.ack, None,
        "no ACK is signed towards a CID unauthenticated bytes named"
    );
}

#[test]
fn a_replayed_envelope_is_refused() {
    let kat = kat();
    let trust = trusted(&kat, 48 * HOUR_MS);
    let mut g = gate(limits());
    assert_eq!(
        reason(&admit(&mut g, &kat, &trust, &fixture_prompt(), NOW, 0)),
        "admitted"
    );
    let again = admit(&mut g, &kat, &trust, &fixture_prompt(), NOW + 1_000, 1);
    assert_eq!(
        again,
        Outcome::Refused(sw_mcp::gate::Refused {
            reason: Reason::Verify(Refusal::Replay),
            kid: Some(sw_crypto::kid_of(kat.controller.public_key())),
            ack: None,
            pause: false,
        })
    );
}

#[test]
fn the_replay_state_survives_a_restart() {
    let kat = kat();
    let trust = trusted(&kat, 48 * HOUR_MS);
    let mut first = gate(limits());
    assert_eq!(
        reason(&admit(&mut first, &kat, &trust, &fixture_prompt(), NOW, 0)),
        "admitted"
    );
    let mut restarted = sw_mcp::gate::DeviceGate::new(limits(), first.replay_log().to_vec());
    assert_eq!(
        reason(&admit(
            &mut restarted,
            &kat,
            &trust,
            &fixture_prompt(),
            NOW,
            0
        )),
        "verify_replay"
    );
}

#[test]
fn a_flipped_byte_anywhere_in_the_envelope_is_refused() {
    let kat = kat();
    let trust = trusted(&kat, 48 * HOUR_MS);
    let good = fixture_prompt();
    assert_eq!(
        reason(&admit(&mut gate(limits()), &kat, &trust, &good, NOW, 0)),
        "admitted"
    );
    // Header, kid, the signed body (inside the sealed ciphertext too) and the signature.
    for at in [3, 20, 200, 1500, good.len() - 1000, good.len() - 1] {
        let mut bad = good.clone();
        bad[at] ^= 0x01;
        let out = admit(&mut gate(limits()), &kat, &trust, &bad, NOW, 0);
        assert!(
            matches!(out, Outcome::Refused(_)),
            "byte {at} flipped was admitted"
        );
    }
}

#[test]
fn a_prompt_sealed_for_another_device_is_refused() {
    let kat = kat();
    let trust = trusted(&kat, 48 * HOUR_MS);
    let other = prompt(&kat, 1, 1, kat.device_cid + 1);
    let out = admit(&mut gate(limits()), &kat, &trust, &other, NOW, 0);
    assert_eq!(reason(&out), "wrong_device");
    let Outcome::Refused(r) = out else {
        unreachable!()
    };
    assert!(
        r.ack.is_some(),
        "a verified refusal is acknowledged to its Controller"
    );
}

#[test]
fn a_prompt_for_a_task_with_no_agent_here_is_refused() {
    let kat = kat();
    let trust = trusted(&kat, 48 * HOUR_MS);
    let m = fixture_prompt();
    assert_eq!(
        reason(&admit(&mut gate(limits()), &kat, &trust, &m, NOW, 0)),
        "admitted"
    );
    let nobody = |_: Option<&str>, _: Option<&str>| false;
    let out = admit_with(&mut gate(limits()), &kat, &trust, &m, NOW, 0, &nobody);
    assert_eq!(reason(&out), "no_attached_agent");
}
