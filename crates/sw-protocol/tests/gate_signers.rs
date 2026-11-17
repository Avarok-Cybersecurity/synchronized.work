#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Who may sign what, for whom (sw-wire contract §2, design §6.1).

mod support;

use support::*;
use sw_protocol::body::Audience;
use sw_protocol::gate::{Refusal, Verifier, accept_unsigned, verify};
use sw_protocol::payload::*;
use sw_protocol::replay::ReplayGuard;
use sw_protocol::signed::SwMessage;
use sw_testkit::{Cast, FIXTURE_NOW_MS as NOW, StandInIdentityPort as ID};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn signers_must_hold_the_right_role_cid_and_issuer() {
    let cast = Cast::new();
    let ctl = cast.alice_controller_cid;
    let fresh = || ReplayGuard::empty();
    let by_controller = sign(
        &body(&cast, ctl, Audience::Node, 1, 1, status()),
        &cast.alice_controller,
    );
    assert_eq!(
        at_node(&cast, &by_controller, &mut fresh(), ctl),
        Err(Refusal::WrongSigner)
    );
    let by_identity = sign(
        &body(&cast, ctl, Audience::Node, 1, 1, status()),
        &cast.alice,
    );
    assert_eq!(
        at_node(&cast, &by_identity, &mut fresh(), ctl),
        Err(Refusal::WrongSigner)
    );
    let wrong_from = sign(
        &body(&cast, 9999, Audience::Node, 1, 1, status()),
        &cast.alice_device,
    );
    assert_eq!(
        at_node(&cast, &wrong_from, &mut fresh(), 9999),
        Err(Refusal::FromMismatch)
    );
    let m = device_status(&cast, 1, 1);
    assert_eq!(
        at_node(&cast, &m, &mut fresh(), 4242),
        Err(Refusal::FromMismatch),
        "session CID"
    );
    let mut as_bob = body(&cast, cast.alice_device_cid, Audience::Node, 1, 1, status());
    as_bob.iss = cast.bob.id_fp();
    let m = sign(&as_bob, &cast.alice_device);
    assert_eq!(
        at_node(&cast, &m, &mut fresh(), cast.alice_device_cid),
        Err(Refusal::IssuerMismatch)
    );
}

#[test]
fn the_outer_typ_must_be_the_body_typ() {
    let cast = Cast::new();
    let SwMessage::Signed { signed, .. } = device_status(&cast, 1, 1) else {
        unreachable!()
    };
    let lying = SwMessage::Signed { typ: 3, signed };
    let got = at_node(
        &cast,
        &lying,
        &mut ReplayGuard::empty(),
        cast.alice_device_cid,
    );
    assert_eq!(got, Err(Refusal::TypMismatch));
}

#[test]
fn revoke_needs_the_identity_key() {
    let cast = Cast::new();
    let ctl = cast.alice_controller_cid;
    let ok = sign(
        &body(&cast, ctl, Audience::Node, 1, 1, revoke(&cast)),
        &cast.alice,
    );
    at_node(&cast, &ok, &mut ReplayGuard::empty(), ctl).unwrap();
    let forged = tamper(&ok, |signed| signed.sig.0[7] ^= 1);
    let got = at_node(&cast, &forged, &mut ReplayGuard::empty(), ctl);
    assert_eq!(
        got,
        Err(Refusal::BadSignature),
        "an identity signature is checked too"
    );
    let by_endpoint = sign(
        &body(&cast, ctl, Audience::Node, 1, 1, revoke(&cast)),
        &cast.alice_controller,
    );
    assert_eq!(
        at_node(&cast, &by_endpoint, &mut ReplayGuard::empty(), ctl),
        Err(Refusal::WrongSigner)
    );
}

#[test]
fn a_device_runs_only_signals_from_its_allowlist_addressed_to_it() {
    let cast = Cast::new();
    let dev = cast.alice_device_cid;
    let at_device = |m: &SwMessage, cid| {
        verify(
            m,
            &cast.device_allowlist(),
            &ID,
            &mut ReplayGuard::empty(),
            Verifier::Endpoint { cid },
            NOW,
        )
        .map(|_| ())
    };
    at_device(
        &prompt(
            &cast,
            dev,
            &cast.alice_controller,
            cast.alice_controller_cid,
        ),
        dev,
    )
    .unwrap();
    let from_bob = prompt(&cast, dev, &cast.bob_controller, cast.bob_controller_cid);
    assert_eq!(
        at_device(&from_bob, dev),
        Err(Refusal::UnknownKid),
        "unpaired controller"
    );
    let elsewhere = prompt(
        &cast,
        7777,
        &cast.alice_controller,
        cast.alice_controller_cid,
    );
    assert_eq!(at_device(&elsewhere, dev), Err(Refusal::WrongAudience));
    let relayed = prompt(
        &cast,
        dev,
        &cast.alice_controller,
        cast.alice_controller_cid,
    );
    at_node(
        &cast,
        &relayed,
        &mut ReplayGuard::empty(),
        cast.alice_controller_cid,
    )
    .unwrap();
}

#[test]
fn signing_must_match_the_type() {
    let cast = Cast::new();
    let hello = Payload::Hello(Hello {
        protocol_version: 1,
        endpoint_kind: HelloKind::Device,
        client_build: "x".into(),
    });
    let m = sign(
        &body(
            &cast,
            cast.alice_device_cid,
            Audience::Node,
            1,
            1,
            hello.clone(),
        ),
        &cast.alice_device,
    );
    assert_eq!(
        at_node(&cast, &m, &mut ReplayGuard::empty(), cast.alice_device_cid),
        Err(Refusal::SigningMismatch)
    );
    assert!(accept_unsigned(SwMessage::Unsigned(hello)).is_ok());
    assert_eq!(
        accept_unsigned(SwMessage::Unsigned(status())),
        Err(Refusal::SigningMismatch)
    );
    let raw = device_status(&cast, 1, 1);
    assert_eq!(
        SwMessage::from_bytes(&raw.to_bytes().unwrap()).unwrap(),
        raw
    );
}
