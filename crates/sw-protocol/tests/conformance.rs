#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! C0's frozen fixtures, judged by the shared check natively and on wasm32
//! (`cargo test --target wasm32-unknown-unknown` runs this file in node).

use sw_testkit::conformance::check;
use sw_testkit::kat::Kat;

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

#[test]
fn every_contract_fixture_passes_the_shared_check() {
    let kat = Kat::parse(KAT).unwrap();
    for (name, json) in FIXTURES {
        check(name, json, &kat).unwrap();
    }
}

#[test]
fn a_flipped_cbor_byte_in_any_valid_fixture_is_refused() {
    let kat = Kat::parse(KAT).unwrap();
    for (name, json) in FIXTURES.iter().filter(|(n, _)| !n.starts_with("invalid-")) {
        let start = json.find("\"cbor_hex\": \"").unwrap() + 13;
        let hex_len = json[start..].find('"').unwrap();
        for at in [start + 9, start + hex_len / 2, start + hex_len - 1] {
            let mut flipped = json.to_string().into_bytes();
            flipped[at] = if flipped[at] == b'0' { b'1' } else { b'0' };
            let flipped = String::from_utf8(flipped).unwrap();
            assert!(
                check(name, &flipped, &kat).is_err(),
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
