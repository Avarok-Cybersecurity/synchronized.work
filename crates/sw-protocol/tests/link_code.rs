#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The link code (design §5.3): drawn uniformly from Crockford's base32, folded from what a
//! person types, hashed with its own domain before the node keeps it.

use sw_protocol::link_code::{ALPHABET, canonical, code_hash, generate};
use sw_protocol::payload::Link;
use sw_testkit::LabelledEntropy;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// Entropy that hands out the bytes it was given.
struct Fixed(Vec<u8>);

impl sw_crypto::Entropy for Fixed {
    fn fill(&mut self, out: &mut [u8]) {
        out.copy_from_slice(&self.0[..out.len()]);
    }
}

#[test]
fn a_code_takes_each_bytes_low_five_bits_as_one_symbol() {
    let code = generate(&mut Fixed(vec![0, 31, 32, 255, 10, 17, 18, 21]));
    assert_eq!(code, "0Z0ZAHJN");
    let mut e = LabelledEntropy::new("link");
    for _ in 0..64 {
        let c = generate(&mut e);
        assert_eq!(c.len(), Link::CODE_LEN);
        assert!(c.bytes().all(|b| ALPHABET.contains(&b)), "{c}");
        assert_eq!(
            canonical(&c).as_deref(),
            Some(c.as_str()),
            "a fresh code is canonical"
        );
    }
}

#[test]
fn what_a_person_types_is_folded_to_the_code() {
    assert_eq!(canonical("k7m2q9xw").as_deref(), Some("K7M2Q9XW"));
    assert_eq!(canonical("O1Il0oLi").as_deref(), Some("01110011"));
    for bad in ["K7M2Q9X", "K7M2Q9XWZ", "K7M2-9XW", "K7M2Q9XU", "K7M2Q9Xé"] {
        assert_eq!(canonical(bad), None, "{bad}");
    }
}

#[test]
fn the_stored_hash_is_domain_separated() {
    assert_ne!(code_hash("K7M2Q9XW"), sw_crypto::sha3_256(b"K7M2Q9XW"));
    assert_ne!(code_hash("K7M2Q9XW"), code_hash("K7M2Q9XX"));
}
