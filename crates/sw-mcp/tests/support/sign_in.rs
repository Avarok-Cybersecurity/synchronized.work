//! The C0 sign-in KAT values the sign-in tests share.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use citadel_identity::{MlDsa65Pk, ServerKey};
use data_encoding::{BASE64URL_NOPAD, HEXLOWER};
use serde_json::Value as Json;

pub fn kat() -> Json {
    serde_json::from_str(super::SIGN_IN_KAT).unwrap()
}

pub fn hex32(j: &Json, key: &str) -> [u8; 32] {
    HEXLOWER
        .decode(j[key].as_str().unwrap().as_bytes())
        .unwrap()
        .try_into()
        .unwrap()
}

pub fn state(j: &Json) -> [u8; 16] {
    BASE64URL_NOPAD
        .decode(j["state_b64url"].as_str().unwrap().as_bytes())
        .unwrap()
        .try_into()
        .unwrap()
}

pub fn issuer_key(j: &Json) -> ServerKey {
    let seed = hex32(&j["issuer"]["key"], "seed_hex");
    ServerKey::from_seed(&seed)
}

pub fn device(j: &Json) -> String {
    j["device"]["value"].as_str().unwrap().to_owned()
}

pub fn wrong_verifier(j: &Json) -> [u8; 32] {
    let case = j["must_fail"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["case"] == "wrong verifier")
        .unwrap();
    hex32(case, "verifier_hex")
}

pub fn published(j: &Json) -> (Vec<u8>, MlDsa65Pk, u64) {
    let a = &j["assertion"];
    let blob = BASE64URL_NOPAD
        .decode(a["blob_b64url"].as_str().unwrap().as_bytes())
        .unwrap();
    let pk = MlDsa65Pk::from_base64url(j["issuer"]["key"]["pk_b64url"].as_str().unwrap()).unwrap();
    (blob, pk, a["exp_s"].as_u64().unwrap())
}
