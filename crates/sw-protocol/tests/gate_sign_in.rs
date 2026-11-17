#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! `ENROL` by assertion (sign-in-with-citadel §6) against C0's sign-in vectors: the KAT
//! Device, unknown to the node, enrols itself with an assertion from the pinned issuer.

use sw_protocol::gate::{AssertionRefusal as A, Refusal, Verified, Verifier, verify};
use sw_protocol::ports::{MessageSigner, SignInClaims};
use sw_protocol::replay::ReplayGuard;
use sw_testkit::assertion::sign_assertion;
use sw_testkit::kat::Kat;
use sw_testkit::sign_in_kat::{SignInKat, enrol_by_assertion};
use sw_testkit::{Cast, Directory, FIXTURE_NOW_MS as NOW, StandInIdentityPort as ID};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

struct World {
    kat: Kat,
    si: SignInKat,
}

fn world() -> World {
    World {
        kat: Kat::parse(include_str!("../../../fixtures/sw-wire-kat.json")).unwrap(),
        si: SignInKat::parse(include_str!("../../../fixtures/sign-in-kat.json")).unwrap(),
    }
}

impl World {
    fn mint(&self, change: impl FnOnce(&mut SignInClaims)) -> Vec<u8> {
        let mut claims = self.si.claims.clone();
        change(&mut claims);
        sign_assertion(&self.si.issuer, &claims, &self.si.nonce)
    }

    fn enrol(
        &self,
        blob: &[u8],
        issuer: &[u8],
        verifier_cid: Option<u64>,
    ) -> Result<Verified, Refusal> {
        let verifier = match verifier_cid {
            None => Verifier::Node {
                session_cid: self.kat.device_cid,
                sign_in_issuer: issuer,
            },
            Some(cid) => Verifier::Endpoint { cid },
        };
        let msg = enrol_by_assertion(&self.kat, blob);
        verify(
            &msg,
            &Directory::default(),
            &ID,
            &mut ReplayGuard::empty(),
            verifier,
            NOW,
        )
    }

    fn at_node(&self, blob: &[u8]) -> Result<Verified, Refusal> {
        self.enrol(blob, self.si.issuer.public_key(), None)
    }
}

#[test]
fn the_published_assertion_enrols_its_device_and_both_audiences_are_accepted() {
    let w = world();
    let v = w.at_node(&w.si.blob).unwrap();
    assert_eq!(v.sign_in, Some(w.si.claims.clone()));
    assert_eq!(v.body.iss, w.kat.person);
    let for_site = w.mint(|c| c.aud = "synchronized.work".into());
    assert!(w.at_node(&for_site).is_ok());
}

#[test]
fn only_the_pinned_issuer_can_vouch() {
    let w = world();
    let cast = Cast::new();
    let other = cast.sign_in_issuer.public_key();
    assert_eq!(
        w.enrol(&w.si.blob, other, None).unwrap_err(),
        Refusal::Assertion(A::Invalid)
    );
    let mut forged = w.si.blob.clone();
    forged[3] ^= 1;
    assert_eq!(
        w.at_node(&forged).unwrap_err(),
        Refusal::Assertion(A::Invalid)
    );
}

#[test]
fn an_assertion_for_another_app_is_refused() {
    let w = world();
    for aud in ["work.avarok.net", "sw-mcp2", "Sw-mcp"] {
        let blob = w.mint(|c| c.aud = aud.into());
        assert_eq!(
            w.at_node(&blob).unwrap_err(),
            Refusal::Assertion(A::WrongAudience),
            "{aud}"
        );
    }
}

#[test]
fn an_expired_assertion_is_refused() {
    let w = world();
    let now_s = NOW / 1000;
    assert!(w.at_node(&w.mint(|c| c.exp_s = now_s + 1)).is_ok());
    let blob = w.mint(|c| c.exp_s = now_s);
    assert_eq!(
        w.at_node(&blob).unwrap_err(),
        Refusal::Assertion(A::Expired)
    );
}

#[test]
fn the_assertion_must_name_the_enrolling_key() {
    let w = world();
    let cast = Cast::new();
    let other_kid =
        data_encoding::BASE64URL_NOPAD.encode(&sw_crypto::kid_of(cast.alice_device.public_key()));
    let (slug, own_kid) = w.si.claims.device.split_once('#').unwrap();
    let cases = [
        format!("{slug}#{other_kid}"),
        own_kid.to_owned(),
        format!("Macbook#{own_kid}"),
        format!("#{own_kid}"),
        format!("{}#{own_kid}", "a".repeat(65)),
    ];
    for device in cases {
        let blob = w.mint(|c| c.device = device.clone());
        assert_eq!(
            w.at_node(&blob).unwrap_err(),
            Refusal::Assertion(A::WrongDevice),
            "{device}"
        );
    }
}

#[test]
fn the_assertion_must_be_for_the_person_named_by_iss() {
    let w = world();
    let cast = Cast::new();
    let blob = w.mint(|c| c.identity = cast.bob.public_key().to_vec());
    assert_eq!(w.at_node(&blob).unwrap_err(), Refusal::IssuerMismatch);
}

#[test]
fn an_endpoint_never_accepts_an_enrol_by_assertion() {
    let w = world();
    let got = w.enrol(
        &w.si.blob,
        w.si.issuer.public_key(),
        Some(w.kat.controller_cid),
    );
    assert_eq!(got.unwrap_err(), Refusal::UnknownKid);
}
