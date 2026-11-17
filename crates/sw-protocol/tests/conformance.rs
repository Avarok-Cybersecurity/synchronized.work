#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! C0's frozen fixtures, judged by the shared check natively and on wasm32
//! (`cargo test --target wasm32-unknown-unknown` runs this file in node).

use sw_testkit::conformance::{check, check_sign_in_enrol};
use sw_testkit::kat::Kat;
use sw_testkit::sign_in_kat::SignInKat;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

macro_rules! fixtures {
    ($($name:literal),* $(,)?) => {
        const FIXTURES: &[(&str, &str)] = &[
            $(($name, include_str!(concat!("../../../fixtures/sw-wire/", $name)))),*
        ];
    };
}

fixtures! {
    "01-hello.json", "02-status-update.json", "04-prompt-sealed.json", "06-ack.json",
    "07-presence.json", "11-enrol.json", "11-revoke.json", "15-subscribe.json",
    "17-progress-tail.json", "invalid-nonshortest-int.json", "invalid-unsorted-map.json",
    "p2p-envelope.json", "person-record.json",
}

const KAT: &str = include_str!("../../../fixtures/sw-wire-kat.json");
const SIGN_IN_KAT: &str = include_str!("../../../fixtures/sign-in-kat.json");

fn kats() -> (Kat, SignInKat) {
    (
        Kat::parse(KAT).unwrap(),
        SignInKat::parse(SIGN_IN_KAT).unwrap(),
    )
}

#[test]
fn every_contract_fixture_passes_the_shared_check() {
    let (kat, sign_in) = kats();
    for (name, json) in FIXTURES {
        check(name, json, &kat, &sign_in).unwrap();
    }
}

#[test]
fn the_kat_device_enrols_itself_by_the_published_assertion() {
    let (kat, sign_in) = kats();
    check_sign_in_enrol(&kat, &sign_in).unwrap();
}

#[test]
fn a_flipped_cbor_byte_in_any_valid_fixture_is_refused() {
    let (kat, sign_in) = kats();
    for (name, json) in FIXTURES.iter().filter(|(n, _)| !n.starts_with("invalid-")) {
        let start = json.find("\"cbor_hex\": \"").unwrap() + 13;
        let hex_len = json[start..].find('"').unwrap();
        for at in [start + 9, start + hex_len / 2, start + hex_len - 1] {
            let mut flipped = json.to_string().into_bytes();
            flipped[at] = if flipped[at] == b'0' { b'1' } else { b'0' };
            let flipped = String::from_utf8(flipped).unwrap();
            assert!(
                check(name, &flipped, &kat, &sign_in).is_err(),
                "{name} accepted a flipped nibble at {at}"
            );
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn this_list_names_every_mirrored_fixture() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/sw-wire");
    let mut on_disk: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    on_disk.sort();
    let listed: Vec<&str> = FIXTURES.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        on_disk, listed,
        "a fixture C0 added is missing from this list"
    );
}

/// The contract publishes no `signal_id`, so this one was computed independently (Python
/// `hashlib.sha3_256` over `04-prompt-sealed`'s bytes after the 4-byte `{1: 4, 3:` head).
#[test]
fn signal_id_is_sha3_256_of_the_signed_bytes() {
    let (name, json) = FIXTURES[2];
    assert_eq!(name, "04-prompt-sealed.json");
    let start = json.find("\"cbor_hex\": \"").unwrap() + 13;
    let hex = &json[start..start + json[start..].find('"').unwrap()];
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    let msg = sw_protocol::signed::SwMessage::from_bytes(&bytes).unwrap();
    let sw_protocol::signed::SwMessage::Signed { signed, .. } = msg else {
        panic!("04 is signed")
    };
    let id: String = signed
        .signal_id()
        .unwrap()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        id,
        "3e857b92f991b42ee288244755dbe2993ef9472cfda1a4e1bd3bcbdab77545ea"
    );
}
