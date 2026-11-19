#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The sign-in flow end to end in-process: `complete_sign_in` against a stand-in issuer.
//!
//! Stand-in, justified: work.avarok.net's issuer (lane I4) is not deployed to anything a test
//! may call, and tests never reach real servers. The stand-in serves a `/.well-known/citadel`
//! signed with the KAT issuer seed by the identity crate's own `ServerKey`, and answers
//! `/sign-in/complete` with the KAT assertion only for the KAT code and verifier. Everything the
//! MCP does with those answers — pinning, parsing, verifying, storing — is production code.

mod support;

use std::sync::Mutex;

use citadel_identity::{ServerKey, WellKnownCitadel, WellKnownError, canonical_domain};
use data_encoding::{BASE64URL_NOPAD, HEXLOWER};
use serde_json::{Value as Json, json};
use sw_mcp::home::Home;
use sw_mcp::signin::{self, IssuerPort, IssuerReply, SignInError};

struct StandInIssuer {
    well_known: Vec<u8>,
    code: String,
    verifier: String,
    answer: Vec<u8>,
    completes: Mutex<u32>,
}

impl IssuerPort for StandInIssuer {
    async fn well_known(&self) -> Result<Vec<u8>, String> {
        Ok(self.well_known.clone())
    }
    async fn complete(&self, json: Vec<u8>) -> Result<IssuerReply, String> {
        *self.completes.lock().unwrap() += 1;
        let r: Json = serde_json::from_slice(&json).map_err(|e| e.to_string())?;
        let (status, body) = if r["code"] != self.code.as_str() {
            (400, b"unknown_code".to_vec())
        } else if r["verifier"] != self.verifier.as_str() {
            (400, b"verifier_mismatch".to_vec())
        } else {
            (200, self.answer.clone())
        };
        Ok(IssuerReply { status, body })
    }
}

fn kat() -> Json {
    serde_json::from_str(support::SIGN_IN_KAT).unwrap()
}

fn hex32(text: &Json) -> [u8; 32] {
    HEXLOWER
        .decode(text.as_str().unwrap().as_bytes())
        .unwrap()
        .try_into()
        .unwrap()
}

fn issuer(j: &Json, seed: [u8; 32]) -> StandInIssuer {
    let key = ServerKey::from_seed(&seed);
    let doc = WellKnownCitadel {
        v: 1,
        canonical_domain: canonical_domain("work.avarok.net").unwrap(),
        citadel_wss: None,
        server_key: key.public_key().clone(),
        former_domains: vec![],
        issued_at_s: 1_794_000_000,
    };
    let verifier = BASE64URL_NOPAD.encode(&hex32(&j["verifier_hex"]));
    let answer =
        json!({ "v": 1, "issuer": "work.avarok.net", "assertion": j["assertion"]["blob_b64url"] });
    StandInIssuer {
        well_known: key.sign_well_known(&doc).unwrap(),
        code: j["code"]["example"].as_str().unwrap().to_owned(),
        verifier,
        answer: serde_json::to_vec(&answer).unwrap(),
        completes: Mutex::new(0),
    }
}

fn fresh_home(name: &str) -> Home {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let home = Home::locate(Some(dir)).unwrap();
    home.ensure_layout().unwrap();
    home
}

fn kat_seed(j: &Json) -> [u8; 32] {
    hex32(&j["issuer"]["key"]["seed_hex"])
}

fn pending(j: &Json, verifier: &Json) -> signin::PendingSignIn {
    let state: [u8; 16] = BASE64URL_NOPAD
        .decode(j["state_b64url"].as_str().unwrap().as_bytes())
        .unwrap()
        .try_into()
        .unwrap();
    signin::start(hex32(verifier), state, "work.avarok.net").0
}

fn now_s(j: &Json) -> u64 {
    j["assertion"]["exp_s"].as_u64().unwrap() - 3600
}

#[tokio::test]
async fn a_good_code_stores_the_assertion_and_pins_the_issuer() {
    let j = kat();
    let home = fresh_home("flow-good");
    let port = issuer(&j, kat_seed(&j));
    let device = j["device"]["value"].as_str().unwrap();
    let p = pending(&j, &j["verifier_hex"]);
    let a = signin::complete_sign_in(
        &port,
        &home,
        &p,
        "ztbn-pg9a",
        device,
        "work.avarok.net",
        now_s(&j),
    )
    .await
    .unwrap();
    assert_eq!(a.device, device);
    let loaded = signin::load_assertion(&home, now_s(&j)).unwrap();
    assert_eq!(loaded.identity_pk, a.identity_pk);
    assert!(home.issuer_pin().exists());
}

#[tokio::test]
async fn the_issuers_refusal_is_reported_and_nothing_is_stored() {
    let j = kat();
    let home = fresh_home("flow-refused");
    let port = issuer(&j, kat_seed(&j));
    let wrong = &j["must_fail"][1]["verifier_hex"];
    assert_eq!(j["must_fail"][1]["case"], "wrong verifier");
    let p = pending(&j, wrong);
    let device = j["device"]["value"].as_str().unwrap();
    let out = signin::complete_sign_in(
        &port,
        &home,
        &p,
        "ZTBN-PG9A",
        device,
        "work.avarok.net",
        now_s(&j),
    )
    .await;
    assert!(
        matches!(out, Err(SignInError::Refused { status: 400, ref reason }) if reason == "verifier_mismatch")
    );
    assert!(!home.assertion().exists());
}

#[tokio::test]
async fn a_malformed_code_never_reaches_the_issuer() {
    let j = kat();
    let home = fresh_home("flow-code");
    let port = issuer(&j, kat_seed(&j));
    let p = pending(&j, &j["verifier_hex"]);
    let out = signin::complete_sign_in(
        &port,
        &home,
        &p,
        "ZTBN-PG9I",
        "x#y",
        "work.avarok.net",
        now_s(&j),
    )
    .await;
    assert!(matches!(out, Err(SignInError::Code(_))));
    assert_eq!(*port.completes.lock().unwrap(), 0);
}

#[tokio::test]
async fn a_changed_issuer_key_is_refused_after_the_first_pin() {
    let j = kat();
    let home = fresh_home("flow-pin");
    let device = j["device"]["value"].as_str().unwrap();
    let p = pending(&j, &j["verifier_hex"]);
    let first = issuer(&j, kat_seed(&j));
    signin::complete_sign_in(
        &first,
        &home,
        &p,
        "ZTBN-PG9A",
        device,
        "work.avarok.net",
        now_s(&j),
    )
    .await
    .unwrap();
    let same = issuer(&j, kat_seed(&j));
    assert!(
        signin::pin_issuer(&same, &home, "work.avarok.net")
            .await
            .is_ok()
    );
    let impostor = issuer(&j, [0x77; 32]);
    let out = signin::complete_sign_in(
        &impostor,
        &home,
        &p,
        "ZTBN-PG9A",
        device,
        "work.avarok.net",
        now_s(&j),
    )
    .await;
    assert!(matches!(
        out,
        Err(SignInError::WellKnown(WellKnownError::KeyChanged { .. }))
    ));
    assert_eq!(
        *impostor.completes.lock().unwrap(),
        0,
        "no code goes to an issuer that failed the pin"
    );
}
