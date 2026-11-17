#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The MCP's half of "Sign in with Citadel" against `fixtures/sign-in-kat.json` (C0): the
//! verifier/challenge pair, the link, the match words, the code format and the complete request.

mod support;

use data_encoding::HEXLOWER;
use serde_json::{Value as Json, json};
use support::sign_in::*;
use sw_mcp::keys::DeviceKeys;
use sw_mcp::signin::{self, CodeError, normalize_code};

#[test]
fn the_verifier_hashes_to_the_published_challenge_link_and_match_words() {
    let j = kat();
    let (pending, link) = signin::start(hex32(&j, "verifier_hex"), state(&j), "work.avarok.net");
    assert_eq!(pending.challenge, hex32(&j, "challenge_hex"));
    assert_eq!(link.url, j["link"].as_str().unwrap());
    let words: Vec<&str> = j["match_phrase"]["words"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    assert_eq!(link.match_words.to_vec(), words);
    let (other, other_link) = signin::start(wrong_verifier(&j), state(&j), "work.avarok.net");
    assert_ne!(
        other.challenge, pending.challenge,
        "a wrong verifier is another challenge"
    );
    assert_ne!(
        other_link.match_words, link.match_words,
        "and shows other words"
    );
}

#[test]
fn the_code_is_eight_symbols_of_the_alphabet_shown_with_a_dash() {
    let j = kat();
    let example = j["code"]["example"].as_str().unwrap();
    assert_eq!(j["code"]["alphabet"], signin::CODE_ALPHABET);
    for typed in [example, "ztbn-pg9a", "ZTBNPG9A", "  ZTBN-PG9A\n"] {
        assert_eq!(normalize_code(typed), Ok(example.to_owned()), "{typed:?}");
    }
    for (typed, why) in [
        ("ZTBN-PG9I", CodeError::Symbol('I')),
        ("ZTBL-PG9A", CodeError::Symbol('L')),
        ("ZTBO-PG9A", CodeError::Symbol('O')),
        ("ZTBU-PG9A", CodeError::Symbol('U')),
        ("ZTB-NPG9A", CodeError::Symbol('-')),
        ("ZTBN-PG9", CodeError::Length),
        ("ZTBN-PG9AA", CodeError::Length),
    ] {
        assert_eq!(normalize_code(typed), Err(why), "{typed:?}");
    }
}

#[test]
fn the_device_label_binds_this_devices_signing_key() {
    let wire: Json = serde_json::from_str(support::WIRE_KAT).unwrap();
    let xi = HEXLOWER
        .decode(wire["device"]["sign_xi_hex"].as_str().unwrap().as_bytes())
        .unwrap();
    let kem = HEXLOWER
        .decode(wire["device"]["kem_seed_hex"].as_str().unwrap().as_bytes())
        .unwrap();
    let file = ciborium::value::Value::Map(vec![
        (1u8.into(), ciborium::value::Value::Bytes(xi)),
        (2u8.into(), ciborium::value::Value::Bytes(kem)),
        (3u8.into(), ciborium::value::Value::Text("generated".into())),
    ]);
    let keys = DeviceKeys::from_bytes(&sw_protocol::cbor::encode(&file).unwrap()).unwrap();
    assert_eq!(keys.device_label("macbook-pro"), device(&kat()));
}

#[test]
fn the_complete_request_is_the_schemas_five_members() {
    let j = kat();
    let (pending, _) = signin::start(hex32(&j, "verifier_hex"), state(&j), "work.avarok.net");
    let request = signin::complete_request(&pending, "ztbnpg9a", &device(&j)).unwrap();
    assert_eq!(
        serde_json::to_value(&request).unwrap(),
        json!({ "v": 1, "client": "sw-mcp", "code": "ZTBN-PG9A",
                "verifier": "M5KIIDSMpWihPSDi-F8rzj4tGCdAedQ1MiJSu-ytIy8", "device": device(&j) })
    );
}
