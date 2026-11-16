#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Runs natively (`cargo test`) and on wasm32 (`wasm-pack test --node`).

use sw_crypto::*;
use sw_testkit::LabelledEntropy;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn signature_round_trips_and_lengths_are_fips_204() {
    let key = SigningKey::from_seed(&[7u8; SIGN_SEED_LEN]);
    let sig = key.sign(b"hello");
    assert_eq!(key.public_key().len(), SIGN_PUBLIC_KEY_LEN);
    assert_eq!(sig.len(), SIGNATURE_LEN);
    verify_signature(key.public_key(), b"hello", &sig).unwrap();
}

#[test]
fn signing_is_deterministic() {
    let key = SigningKey::from_seed(&[7u8; SIGN_SEED_LEN]);
    assert_eq!(key.sign(b"m"), key.sign(b"m"));
}

#[test]
fn flipped_message_or_signature_byte_is_bad_signature() {
    let key = SigningKey::from_seed(&[1u8; SIGN_SEED_LEN]);
    let mut sig = key.sign(b"body");
    assert_eq!(
        verify_signature(key.public_key(), b"bodY", &sig),
        Err(CryptoError::BadSignature)
    );
    sig[100] ^= 1;
    assert!(verify_signature(key.public_key(), b"body", &sig).is_err());
}

#[test]
fn another_key_does_not_verify() {
    let a = SigningKey::from_seed(&[1u8; SIGN_SEED_LEN]);
    let b = SigningKey::from_seed(&[2u8; SIGN_SEED_LEN]);
    assert_eq!(
        verify_signature(b.public_key(), b"x", &a.sign(b"x")),
        Err(CryptoError::BadSignature)
    );
}

#[test]
fn truncated_key_is_malformed() {
    let key = SigningKey::from_seed(&[1u8; SIGN_SEED_LEN]);
    let sig = key.sign(b"x");
    assert!(matches!(
        verify_signature(&key.public_key()[..100], b"x", &sig),
        Err(CryptoError::Malformed(_))
    ));
}

#[test]
fn kem_shared_secret_agrees() {
    let mut e = LabelledEntropy::new("kem");
    let dk = KemSecretKey::generate(&mut e);
    assert_eq!(dk.encapsulation_key().len(), KEM_ENCAPSULATION_KEY_LEN);
    let (ct, ss) = kem_encapsulate(dk.encapsulation_key(), &mut e).unwrap();
    assert_eq!(*dk.decapsulate(&ct).unwrap(), *ss);
}

#[test]
fn kem_with_another_key_disagrees() {
    let mut e = LabelledEntropy::new("kem2");
    let a = KemSecretKey::generate(&mut e);
    let b = KemSecretKey::generate(&mut e);
    let (ct, ss) = kem_encapsulate(a.encapsulation_key(), &mut e).unwrap();
    assert_ne!(*b.decapsulate(&ct).unwrap(), *ss);
}

#[test]
fn aead_refuses_tampering_and_wrong_aad() {
    let key = [9u8; AEAD_KEY_LEN];
    let nonce = [3u8; AEAD_NONCE_LEN];
    let mut ct = aead_seal(&key, &nonce, b"secret", b"aad").unwrap();
    assert_eq!(aead_open(&key, &nonce, &ct, b"aad").unwrap(), b"secret");
    assert_eq!(
        aead_open(&key, &nonce, &ct, b"other"),
        Err(CryptoError::OpenFailed)
    );
    ct[0] ^= 1;
    assert_eq!(
        aead_open(&key, &nonce, &ct, b"aad"),
        Err(CryptoError::OpenFailed)
    );
}

#[test]
fn hkdf_separates_by_info() {
    let a = hkdf_sha3_256(b"ikm", b"sw/seal/1").unwrap();
    let b = hkdf_sha3_256(b"ikm", b"sw/seal/2").unwrap();
    assert_ne!(a, b);
    assert_eq!(a, hkdf_sha3_256(b"ikm", b"sw/seal/1").unwrap());
}

#[test]
fn kid_is_sha256_of_the_key() {
    assert_eq!(kid_of(b"abc"), sha256(b"abc"));
    assert_eq!(
        sha256(b"abc")[..4],
        [0xba, 0x78, 0x16, 0xbf],
        "SHA-256 known answer"
    );
    assert_eq!(
        sha3_256(b"")[..4],
        [0xa7, 0xff, 0xc6, 0xf8],
        "SHA3-256 known answer"
    );
}
