//! Post-quantum primitives for synchronized.work, bytes in and bytes out.
//!
//! Nothing here reads a clock, the OS or a global RNG: every random input arrives through
//! [`Entropy`], which each adapter implements over its platform's CSPRNG (web crypto, the OS,
//! the Worker). ML-DSA-65 signing is deterministic, so signing needs no entropy at all.

mod aead;
mod hash;
mod kem;
mod sign;

pub use aead::{AEAD_KEY_LEN, AEAD_NONCE_LEN, aead_open, aead_seal};
pub use hash::{hkdf_sha3_256, kid_of, sha3_256, sha256};
pub use kem::{
    KEM_CIPHERTEXT_LEN, KEM_ENCAPSULATION_KEY_LEN, KEM_SEED_LEN, KemSecretKey, SharedSecret,
    kem_encapsulate,
};
pub use sign::{SIGN_PUBLIC_KEY_LEN, SIGN_SEED_LEN, SIGNATURE_LEN, SigningKey, verify_signature};

/// The only source of randomness the pure crates accept (SBIO). An adapter must fill `out`
/// entirely with output from a cryptographically secure generator, or not return.
pub trait Entropy {
    fn fill(&mut self, out: &mut [u8]);
}

/// 32 bytes identifying a public key: `SHA-256(public key bytes)` (design §4.2 `kid`).
pub type Kid = [u8; 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoError {
    /// A key, signature or ciphertext had the wrong length or did not decode.
    Malformed(&'static str),
    /// A signature did not verify.
    BadSignature,
    /// AES-GCM authentication failed: wrong key, wrong associated data, or a modified byte.
    OpenFailed,
}

impl core::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Malformed(what) => write!(f, "malformed {what}"),
            Self::BadSignature => f.write_str("bad signature"),
            Self::OpenFailed => f.write_str("sealed payload failed to open"),
        }
    }
}

impl std::error::Error for CryptoError {}
