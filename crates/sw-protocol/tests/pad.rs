#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The scratchpad envelope (types 18, 19): text sealed to recipients only, and the reader's
//! gate, which verifies a relayed note against the sender's own proof. Each refusal has a
//! control: the untouched message passes.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use sw_crypto::SigningKey;
use sw_protocol::gate::{AssertionRefusal, BoundDevice, Refusal, verify_pad};
use sw_protocol::pad::{PadOpenError, Recipient, open_text, seal_text};
use sw_protocol::payload::pad::pad_limits;
use sw_protocol::payload::*;
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Bytes;
use sw_testkit::pad::{PadDevice, issuer};
use sw_testkit::{LabelledEntropy, StandInIdentityPort as ID};

const NOW: u64 = 1_795_000_000_000;
const HOUR: u64 = 3_600_000;

fn text(body: &str) -> PadText {
    PadText {
        body: body.into(),
        refs: vec!["https://github.com/o/r/pull/9".into()],
    }
}

fn note(from: &PadDevice, to: &[&PadDevice], body: &str, nonce: u8) -> Vec<u8> {
    note_for(from, to, body, nonce, HOUR)
}

fn note_for(from: &PadDevice, to: &[&PadDevice], body: &str, nonce: u8, life: u64) -> Vec<u8> {
    let recipients: Vec<Recipient<'_>> = to
        .iter()
        .map(|d| Recipient {
            cid: d.cid,
            seal_pk: d.kem.encapsulation_key(),
        })
        .collect();
    let sealed = seal_text(&text(body), &recipients, &mut LabelledEntropy::new("pad")).unwrap();
    let payload = Payload::PadNote(PadNote {
        project: "p1".into(),
        scope: PadScope::Members,
        to: to.iter().map(|d| d.id_fp()).collect(),
        body: sealed,
        proof: Bytes(from.proof.clone()),
    });
    from.signed(payload, NOW, life, nonce)
}

fn pinned() -> Vec<u8> {
    issuer().public_key().to_vec()
}

fn check(bytes: &[u8]) -> Result<BoundDevice, Refusal> {
    let message = SwMessage::from_bytes(bytes).unwrap();
    verify_pad(&message, &pinned(), &ID, NOW + 1000).map(|(v, d)| {
        assert_eq!(v.body.iss, d.id_fp);
        d
    })
}

fn body_of(bytes: &[u8]) -> PadNote {
    let SwMessage::Signed { signed, .. } = SwMessage::from_bytes(bytes).unwrap() else {
        panic!("signed")
    };
    match sw_protocol::body::Body::from_bytes(&signed.body.0)
        .unwrap()
        .payload
    {
        Payload::PadNote(n) => n,
        _ => panic!("a note"),
    }
}

#[test]
fn a_note_opens_for_its_recipient_only() {
    let (a, b, c) = (
        PadDevice::new(1, 11, 101, NOW),
        PadDevice::new(2, 12, 102, NOW),
        PadDevice::new(3, 13, 103, NOW),
    );
    let n = body_of(&note(&a, &[&b], "free to merge my PR", 1));
    assert_eq!(
        open_text(&n.body, b.cid, &b.kem).unwrap().body,
        "free to merge my PR"
    );
    assert_eq!(
        open_text(&n.body, c.cid, &c.kem),
        Err(PadOpenError::NotForMe)
    );
    assert!(matches!(
        open_text(&n.body, b.cid, &c.kem),
        Err(PadOpenError::Seal(_))
    ));
}

#[test]
fn the_reader_gate_accepts_a_proven_note_and_refuses_forgeries() {
    let (a, b) = (
        PadDevice::new(1, 11, 101, NOW),
        PadDevice::new(2, 12, 102, NOW),
    );
    let good = note(&a, &[&b], "hello", 1);
    assert_eq!(
        check(&good).unwrap().cid,
        a.cid,
        "control: an untouched note passes"
    );

    let mut flipped = good.clone();
    let at = flipped.len() - 10;
    flipped[at] ^= 1;
    assert!(check(&flipped).is_err(), "a flipped bit in the signature");

    let mut stranger = PadDevice::new(1, 14, 104, NOW);
    stranger.proof = a.proof.clone();
    let wrong_key = note(&stranger, &[&b], "hello", 2);
    assert_eq!(check(&wrong_key).err(), Some(Refusal::UnknownKid));

    let other = PadDevice::new(9, 19, 109, NOW);
    let mut posing = PadDevice::new(1, 19, 109, NOW);
    posing.proof = other.proof.clone();
    assert_eq!(
        check(&note(&posing, &[&b], "hi", 3)).err(),
        Some(Refusal::IssuerMismatch)
    );
}

#[test]
fn a_proof_from_an_unpinned_issuer_is_refused() {
    let (a, b) = (
        PadDevice::new(1, 11, 101, NOW),
        PadDevice::new(2, 12, 102, NOW),
    );
    let message = SwMessage::from_bytes(&note(&a, &[&b], "hi", 1)).unwrap();
    let wrong = SigningKey::from_seed(&[0x77; 32]).public_key().to_vec();
    let refused = verify_pad(&message, &wrong, &ID, NOW + 1000).err();
    assert_eq!(refused, Some(Refusal::Assertion(AssertionRefusal::Invalid)));
    assert!(
        verify_pad(&message, &pinned(), &ID, NOW + 1000).is_ok(),
        "control"
    );
}

#[test]
fn a_proof_is_judged_when_the_device_enrolled_not_now() {
    let (a, b) = (
        PadDevice::new(1, 11, 101, NOW),
        PadDevice::new(2, 12, 102, NOW),
    );
    let message = SwMessage::from_bytes(&note_for(&a, &[&b], "hi", 1, 3 * HOUR)).unwrap();
    let after_assertion = NOW + 2 * HOUR;
    assert!(verify_pad(&message, &pinned(), &ID, after_assertion).is_ok());
    let after_note = NOW + 3 * HOUR;
    assert_eq!(
        verify_pad(&message, &pinned(), &ID, after_note).err(),
        Some(Refusal::Expired)
    );
}

#[test]
fn a_note_scope_must_match_its_targets() {
    let (a, b) = (
        PadDevice::new(1, 11, 101, NOW),
        PadDevice::new(2, 12, 102, NOW),
    );
    let decode = |bytes: &[u8]| {
        let SwMessage::Signed { signed, .. } = SwMessage::from_bytes(bytes).unwrap() else {
            panic!("signed")
        };
        sw_protocol::body::Body::from_bytes(&signed.body.0).map(|_| ())
    };
    let mut n = body_of(&note(&a, &[&b], "hi", 1));
    assert!(
        decode(&a.signed(Payload::PadNote(n.clone()), NOW, HOUR, 5)).is_ok(),
        "control"
    );
    n.scope = PadScope::All;
    assert!(decode(&a.signed(Payload::PadNote(n), NOW, HOUR, 5)).is_err());
}

#[test]
fn text_over_the_limit_is_not_sealed() {
    let b = PadDevice::new(2, 12, 102, NOW);
    let to = [Recipient {
        cid: b.cid,
        seal_pk: b.kem.encapsulation_key(),
    }];
    let long = "x".repeat(pad_limits::TEXT_BYTES + 1);
    assert!(seal_text(&text(&long), &to, &mut LabelledEntropy::new("x")).is_err());
    let ok = "x".repeat(pad_limits::TEXT_BYTES);
    assert!(
        seal_text(&text(&ok), &to, &mut LabelledEntropy::new("x")).is_ok(),
        "control"
    );
}
