#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The committed fixtures, judged by the shared check natively and on wasm32
//! (`cargo test --target wasm32-unknown-unknown` runs this file in node).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

macro_rules! fixtures {
    ($($name:literal => $diag:literal),* $(,)?) => {
        const FIXTURES: &[(&str, &[u8], &str)] = &[
            $(($name, include_bytes!(concat!("../../../fixtures/", $name)), include_str!(concat!("../../../fixtures/", $diag)))),*
        ];
    };
}

fixtures! {
    "01-hello.cbor" => "01-hello.diag",
    "02-status-update.cbor" => "02-status-update.diag",
    "03-progress-event.cbor" => "03-progress-event.diag",
    "04-prompt.cbor" => "04-prompt.diag",
    "05-control.cbor" => "05-control.diag",
    "06-ack.cbor" => "06-ack.diag",
    "07-presence.cbor" => "07-presence.diag",
    "08-join-request.cbor" => "08-join-request.diag",
    "09-join-answer.cbor" => "09-join-answer.diag",
    "10-task-write.cbor" => "10-task-write.diag",
    "11-enrol.cbor" => "11-enrol.diag",
    "12-suggestion.cbor" => "12-suggestion.diag",
    "13-event.cbor" => "13-event.diag",
    "14-notify.cbor" => "14-notify.diag",
    "15-subscribe.cbor" => "15-subscribe.diag",
    "16-link-code.cbor" => "16-link-code.diag",
    "16-link-redeem.cbor" => "16-link-redeem.diag",
    "17-progress-tail.p2p.cbor" => "17-progress-tail.p2p.diag",
}

#[test]
fn every_committed_fixture_passes_the_shared_check() {
    for (name, bytes, diag) in FIXTURES {
        sw_testkit::conformance::check(name, bytes, diag).unwrap();
    }
}

#[test]
fn committed_fixtures_are_what_the_generator_produces() {
    let generated = sw_testkit::fixtures::all();
    assert_eq!(
        generated.len(),
        FIXTURES.len(),
        "this list must name every fixture"
    );
    for ((name, bytes, _), (gen_name, gen_bytes)) in FIXTURES.iter().zip(&generated) {
        assert_eq!(name, gen_name);
        assert_eq!(
            *bytes,
            gen_bytes.as_slice(),
            "{name} drifted from the generator"
        );
    }
}

#[test]
fn a_flipped_byte_in_any_fixture_is_refused() {
    for (name, bytes, diag) in FIXTURES {
        for at in [bytes.len() / 3, bytes.len() / 2, bytes.len() - 1] {
            let mut flipped = bytes.to_vec();
            flipped[at] ^= 0x01;
            assert!(
                sw_testkit::conformance::check(name, &flipped, diag).is_err(),
                "{name} accepted a flipped byte at {at}"
            );
        }
    }
}

#[test]
fn every_message_type_has_a_fixture() {
    let mut typs: Vec<u8> = Vec::new();
    for (name, _, _) in FIXTURES {
        typs.push(name[..2].parse().unwrap());
    }
    typs.dedup();
    assert_eq!(typs, (1..=17).collect::<Vec<u8>>());
}
