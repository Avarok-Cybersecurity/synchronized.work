#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The assertion checks of sign-in (contract §1 step 5) against the C0 KAT assertion. Each
//! refusal sits beside the accepted case it differs from.

mod support;

use citadel_identity::{AssertionError, MlDsa65Pk, ServerKey, SignInAssertion};
use data_encoding::BASE64URL_NOPAD;
use serde_json::{Value as Json, json};
use support::sign_in::*;
use sw_mcp::signin::{self, AcceptError};

#[test]
fn the_published_assertion_is_accepted_and_a_wrong_verifier_is_refused() {
    let j = kat();
    let (blob, pk, exp_s) = published(&j);
    let (pending, _) = signin::start(hex32(&j, "verifier_hex"), state(&j), "work.avarok.net");
    let ok = signin::accept_blob(&pending, blob.clone(), &pk, &device(&j), exp_s - 1).unwrap();
    assert_eq!(ok.device, device(&j));
    let (wrong, _) = signin::start(wrong_verifier(&j), state(&j), "work.avarok.net");
    let refused = signin::accept_blob(&wrong, blob, &pk, &device(&j), exp_s - 1);
    assert_eq!(
        refused,
        Err(AcceptError::Assertion(AssertionError::WrongNonce))
    );
}

#[test]
fn an_expired_assertion_another_device_or_another_issuer_is_refused() {
    let j = kat();
    let (blob, pk, exp_s) = published(&j);
    let (pending, _) = signin::start(hex32(&j, "verifier_hex"), state(&j), "work.avarok.net");
    assert!(signin::accept_blob(&pending, blob.clone(), &pk, &device(&j), exp_s - 1).is_ok());
    let expired = signin::accept_blob(&pending, blob.clone(), &pk, &device(&j), exp_s);
    assert!(matches!(
        expired,
        Err(AcceptError::Assertion(AssertionError::Expired { .. }))
    ));
    let other_device = signin::accept_blob(&pending, blob.clone(), &pk, "imac#AAAA", exp_s - 1);
    assert!(matches!(other_device, Err(AcceptError::WrongDevice { .. })));
    let stranger = ServerKey::from_seed(&[9; 32]);
    let forged = signin::accept_blob(
        &pending,
        blob,
        stranger.public_key(),
        &device(&j),
        exp_s - 1,
    );
    assert!(matches!(
        forged,
        Err(AcceptError::Assertion(AssertionError::Signature(_)))
    ));
}

#[test]
fn an_assertion_for_another_audience_is_refused() {
    let j = kat();
    let (published_blob, pk, exp_s) = published(&j);
    let (pending, _) = signin::start(hex32(&j, "verifier_hex"), state(&j), "work.avarok.net");
    let identity =
        MlDsa65Pk::from_base64url(j["assertion"]["identity_pk_b64url"].as_str().unwrap()).unwrap();
    let mint = |aud: &str| {
        let a = SignInAssertion {
            identity: identity.clone(),
            device: device(&j),
            aud: aud.into(),
            exp_s,
            nonce: pending.challenge,
        };
        issuer_key(&j).sign_assertion(&a).unwrap()
    };
    assert_eq!(
        mint("sw-mcp"),
        published_blob,
        "re-minting reproduces the KAT bytes"
    );
    let other = signin::accept_blob(
        &pending,
        mint("synchronized.work"),
        &pk,
        &device(&j),
        exp_s - 1,
    );
    assert!(matches!(
        other,
        Err(AcceptError::Assertion(AssertionError::WrongAudience { .. }))
    ));
}

#[test]
fn the_response_must_be_exactly_the_schemas_from_this_issuer() {
    let j = kat();
    let (blob, pk, exp_s) = published(&j);
    let (pending, _) = signin::start(hex32(&j, "verifier_hex"), state(&j), "work.avarok.net");
    let body = |v: Json| serde_json::to_vec(&v).unwrap();
    let assertion = BASE64URL_NOPAD.encode(&blob);
    let good = body(json!({ "v": 1, "issuer": "work.avarok.net", "assertion": assertion }));
    assert!(
        signin::accept(
            &pending,
            &good,
            "work.avarok.net",
            &pk,
            &device(&j),
            exp_s - 1
        )
        .is_ok()
    );
    let other = body(json!({ "v": 1, "issuer": "evil.example", "assertion": assertion }));
    assert!(matches!(
        signin::accept(
            &pending,
            &other,
            "work.avarok.net",
            &pk,
            &device(&j),
            exp_s - 1
        ),
        Err(AcceptError::WrongIssuer { .. })
    ));
    let extra =
        body(json!({ "v": 1, "issuer": "work.avarok.net", "assertion": assertion, "token": "x" }));
    assert!(matches!(
        signin::accept(
            &pending,
            &extra,
            "work.avarok.net",
            &pk,
            &device(&j),
            exp_s - 1
        ),
        Err(AcceptError::Response(_))
    ));
}
