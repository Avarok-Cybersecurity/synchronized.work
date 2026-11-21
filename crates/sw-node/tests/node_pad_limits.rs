#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The developer scratchpad's limits at the node: rate, pending cap, project policy and
//! membership. Each refusal has its control.

mod pad_support;
mod support;

use pad_support::*;
use sw_node::core::{NodeRefusal, PadRefusal};
use sw_protocol::payload::*;
use sw_protocol::signed::SwMessage;
use sw_testkit::pad::issuer;

fn to(sent: &[(u64, Vec<u8>)], bytes: &[u8]) -> Vec<u64> {
    sent.iter()
        .filter(|(_, b)| b == bytes)
        .map(|(c, _)| *c)
        .collect()
}

#[test]
fn the_eleventh_message_in_a_minute_is_refused() {
    let mut w = Pad::standard();
    for i in 0..10 {
        let note = w.note(1, &[2], PadScope::Members);
        let sent = w.raw(101, &note).unwrap();
        assert_eq!(
            receipts(&sent),
            vec![(101, true, None)],
            "control: note {i}"
        );
    }
    let note = w.note(1, &[2], PadScope::Members);
    let sent = w.raw(101, &note).unwrap();
    assert_eq!(
        receipts(&sent),
        vec![(101, false, Some("RateLimited".into()))]
    );
    assert!(to(&sent, &note).is_empty(), "nobody is sent a refused note");
}

#[test]
fn a_target_with_too_many_pending_notes_is_refused_more() {
    let mut rules = support::rules(&issuer());
    rules.limits.pad_rate_per_min = 1000;
    rules.limits.pad_pending_max = 3;
    let mut w = Pad::new(rules);
    let policy = Payload::PadPolicy(PadPolicy {
        project: "p".into(),
        retention_ms: HOUR,
    });
    w.send(1, policy, 60_000).unwrap();
    let mut first = None;
    for _ in 0..3 {
        let note = w.note(1, &[2], PadScope::Members);
        first.get_or_insert(id_of(&note));
        assert!(
            receipts(&w.raw(101, &note).unwrap())[0].1,
            "control: under the cap"
        );
    }
    let fourth = w.note(1, &[2], PadScope::Members);
    let sent = w.raw(101, &fourth).unwrap();
    assert_eq!(
        receipts(&sent),
        vec![(101, false, Some("PendingFull".into()))]
    );
    let to_c = w.note(1, &[3], PadScope::Members);
    assert!(receipts(&w.raw(101, &to_c).unwrap())[0].1, "C has room");

    let ack = w.ack(2, first.unwrap());
    w.raw(102, &ack).unwrap();
    let fifth = w.note(1, &[2], PadScope::Members);
    assert!(
        receipts(&w.raw(101, &fifth).unwrap())[0].1,
        "control: room again after an ack"
    );
}

#[test]
fn notes_wait_for_the_project_policy_and_respect_its_retention() {
    let mut w = Pad::new(support::rules(&issuer()));
    let note = w.note(1, &[2], PadScope::Members);
    assert_eq!(
        receipts(&w.raw(101, &note).unwrap())[0].2.as_deref(),
        Some("NoPolicy")
    );

    let by_member = Payload::PadPolicy(PadPolicy {
        project: "p".into(),
        retention_ms: HOUR,
    });
    assert_eq!(
        w.send(2, by_member, 60_000),
        Err(NodeRefusal::Pad(PadRefusal::NotTheOwner))
    );
    let short = Payload::PadPolicy(PadPolicy {
        project: "p".into(),
        retention_ms: HOUR / 2,
    });
    w.send(1, short, 60_000).unwrap();
    let note = w.note(1, &[2], PadScope::Members);
    assert_eq!(
        receipts(&w.raw(101, &note).unwrap())[0].2.as_deref(),
        Some("TooLong")
    );
    let long = Payload::PadPolicy(PadPolicy {
        project: "p".into(),
        retention_ms: HOUR,
    });
    w.send(1, long, 60_000).unwrap();
    let note = w.note(1, &[2], PadScope::Members);
    assert!(
        receipts(&w.raw(101, &note).unwrap())[0].1,
        "control: within retention"
    );
}

#[test]
fn strangers_and_misaddressed_keys_are_refused() {
    let mut w = Pad::standard();
    let note = w.note(4, &[2], PadScope::Members);
    let sent = w.raw(104, &note).unwrap();
    assert_eq!(
        receipts(&sent),
        vec![(104, false, Some("NotAMember".into()))]
    );
    let to_d = w.note(1, &[4], PadScope::Members);
    let sent = w.raw(101, &to_d).unwrap();
    assert_eq!(
        receipts(&sent),
        vec![(101, false, Some("NotAMember".into()))]
    );

    let mut w = Pad::standard();
    let sealed_to_c = w.note(1, &[3], PadScope::Members);
    let SwMessage::Signed { signed, .. } = SwMessage::from_bytes(&sealed_to_c).unwrap() else {
        unreachable!()
    };
    let body = sw_protocol::body::Body::from_bytes(&signed.body.0).unwrap();
    let Payload::PadNote(mut n) = body.payload else {
        unreachable!()
    };
    n.to = vec![w.b.id_fp()];
    let misaddressed = w.signed(1, Payload::PadNote(n), HOUR);
    let sent = w.raw(101, &misaddressed).unwrap();
    assert_eq!(
        receipts(&sent),
        vec![(101, false, Some("BadRecipients".into()))]
    );
}

#[test]
fn an_enrolled_device_can_re_enrol_to_refresh_its_proof() {
    let mut w = Pad::standard();
    let key = "proof/00000000000000000101";
    assert_eq!(
        w.stored.get(key),
        Some(&w.a.proof),
        "control: the first proof"
    );
    let again = w.a.enrol_at(NOW + 1000, 200);
    w.raw(101, &again).unwrap();
    assert_eq!(
        w.stored.get(key),
        Some(&again),
        "a known Device re-enrols by assertion"
    );
}
