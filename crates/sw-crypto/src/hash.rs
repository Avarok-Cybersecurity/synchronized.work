use hkdf::Hkdf;
use sha2::Digest as _;
use sha3::Sha3_256;

use crate::{CryptoError, Kid};

pub fn sha3_256(data: &[u8]) -> [u8; 32] {
    Sha3_256::digest(data).into()
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    sha2::Sha256::digest(data).into()
}

/// The `kid` of any public key, signing or sealing: `SHA-256(key bytes)` (design §4.2).
pub fn kid_of(public_key: &[u8]) -> Kid {
    sha256(public_key)
}

/// HKDF-SHA3-256 with no salt, expanding `ikm` under `info` to one 32-byte key.
pub fn hkdf_sha3_256(ikm: &[u8], info: &[u8]) -> Result<[u8; 32], CryptoError> {
    let mut okm = [0u8; 32];
    Hkdf::<Sha3_256>::new(None, ikm)
        .expand(info, &mut okm)
        .map_err(|_| CryptoError::Malformed("hkdf output length"))?;
    Ok(okm)
}
