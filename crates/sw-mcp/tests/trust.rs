#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The allowlist file: signed by this Device's key, refused when anyone else edits it.

mod support;

use support::*;
use sw_crypto::SigningKey;
use sw_mcp::adapters::CitadelIdentity;
use sw_mcp::trust::{TrustError, TrustList, fingerprint_words};
use sw_protocol::ports::IdentityPort as _;

#[test]
fn the_allowlist_round_trips_under_the_device_key_and_refuses_any_other() {
    let kat = kat();
    let list = trusted(&kat, HOUR_MS);
    let bytes = list.to_bytes(&kat.device).unwrap();
    assert_eq!(
        TrustList::from_bytes(&bytes, kat.device.public_key()).unwrap(),
        list
    );
    let other = SigningKey::from_seed(&[3; 32]);
    assert_eq!(
        TrustList::from_bytes(&bytes, other.public_key()),
        Err(TrustError::NotOurs)
    );
    let mut edited = bytes.clone();
    let at = edited.len() / 2;
    edited[at] ^= 1;
    assert!(TrustList::from_bytes(&edited, kat.device.public_key()).is_err());
}

#[test]
fn revoking_by_prefix_needs_eight_hex_and_removes_only_that_kid() {
    let kat = kat();
    let mut list = trusted(&kat, HOUR_MS);
    let kid = sw_crypto::kid_of(kat.controller.public_key());
    let hex: String = kid.iter().map(|b| format!("{b:02x}")).collect();
    assert!(
        list.revoke_prefix(&hex[..7]).is_empty(),
        "seven characters are too few"
    );
    assert_eq!(list.revoke_prefix(&hex[..8]), vec![kid]);
    assert!(list.entries().is_empty());
}

#[test]
fn the_fingerprint_is_six_words_of_the_kid() {
    let kat = kat();
    let kid = sw_crypto::kid_of(kat.controller.public_key());
    let words = fingerprint_words(&kid);
    assert_eq!(words.split(' ').count(), 6);
    let other = fingerprint_words(&sw_crypto::kid_of(kat.device.public_key()));
    assert_ne!(words, other);
}

#[test]
fn the_identity_adapter_agrees_with_the_published_id_fp() {
    let kat = kat();
    assert_eq!(CitadelIdentity.id_fp(&kat.identity_pk).unwrap(), kat.person);
}
