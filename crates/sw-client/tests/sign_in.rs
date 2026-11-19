#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The link against the C0 known answers (`fixtures/sign-in-kat.json`): the same verifier and
//! state must give the published link byte for byte.

use data_encoding::{BASE64URL_NOPAD, HEXLOWER};
use sw_client::sign_in::{SignInClient, SignInError, sign_in_link};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const KAT: &str = include_str!("../../../fixtures/sign-in-kat.json");

fn kat() -> (serde_json::Value, [u8; 32], Vec<u8>) {
    let j: serde_json::Value = serde_json::from_str(KAT).unwrap();
    let verifier: [u8; 32] = HEXLOWER
        .decode(j["verifier_hex"].as_str().unwrap().as_bytes())
        .unwrap()
        .try_into()
        .unwrap();
    let state = BASE64URL_NOPAD
        .decode(j["state_b64url"].as_str().unwrap().as_bytes())
        .unwrap();
    (j, verifier, state)
}

const MCP: SignInClient<'static> = SignInClient {
    issuer: "work.avarok.net",
    client: "sw-mcp",
    return_url: "https://work.avarok.net/sign-in/code",
};

#[test]
fn the_link_is_the_published_known_answer() {
    let (j, verifier, state) = kat();
    let link = sign_in_link(&MCP, &verifier, &state).unwrap();
    assert_eq!(link, j["link"].as_str().unwrap());
}

#[test]
fn another_verifier_gives_another_challenge() {
    let (j, mut verifier, state) = kat();
    verifier[0] ^= 1;
    let link = sign_in_link(&MCP, &verifier, &state).unwrap();
    assert_ne!(link, j["link"].as_str().unwrap());
    assert!(!link.contains(j["challenge_b64url"].as_str().unwrap()));
}

#[test]
fn the_link_carries_no_verifier_and_exactly_four_parameters() {
    let (_, verifier, state) = kat();
    let site = SignInClient {
        issuer: "work.avarok.net",
        client: "synchronized.work",
        return_url: "https://synchronized.work/auth/return",
    };
    let link = sign_in_link(&site, &verifier, &state).unwrap();
    assert!(!link.contains(&BASE64URL_NOPAD.encode(&verifier)));
    let query = link.split_once('?').unwrap().1;
    let keys: Vec<&str> = query
        .split('&')
        .map(|kv| kv.split('=').next().unwrap())
        .collect();
    assert_eq!(keys, ["client", "challenge", "state", "return"]);
    assert!(link.ends_with("return=https%3A%2F%2Fsynchronized.work%2Fauth%2Freturn"));
}

#[test]
fn a_state_outside_16_to_32_bytes_is_refused() {
    let (_, verifier, _) = kat();
    for len in [0, 15, 33] {
        assert_eq!(
            sign_in_link(&MCP, &verifier, &vec![7; len]),
            Err(SignInError::StateLength)
        );
    }
}
