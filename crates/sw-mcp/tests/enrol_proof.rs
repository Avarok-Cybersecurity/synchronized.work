#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! A Device re-enrols when its stored proof no longer announces its keys, as after the move to
//! envelope version 2: the node would otherwise keep sealing to a key the Device cannot use.

use sw_crypto::KemSecretKey;
use sw_mcp::outbound::proof_is_current;
use sw_testkit::pad::PadDevice;

#[test]
fn a_proof_is_current_only_for_the_keys_it_announces() {
    let d = PadDevice::new(1, 2, 77, 1_795_000_000_000);
    let (sign, seal) = (d.sign.public_key(), d.kem.encapsulation_key());
    assert!(
        proof_is_current(&d.proof, sign, seal),
        "control: its own keys"
    );
    let other = KemSecretKey::from_seed(&[9; 64]);
    assert!(!proof_is_current(&d.proof, sign, other.encapsulation_key()));
    assert!(!proof_is_current(b"not cbor", sign, seal));
}
