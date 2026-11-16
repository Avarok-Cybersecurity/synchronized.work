use hkdf::Hkdf;
use sha3::{Digest as _, Sha3_256};

use crate::{CryptoError, Kid};

pub fn sha3_256(data: &[u8]) -> [u8; 32] {
    Sha3_256::digest(data).into()
}

/// The `kid` of any public key, signing or sealing: `SHA3-256(key bytes)` (sw-wire contract
/// §2; the batch uses one hash family).
pub fn kid_of(public_key: &[u8]) -> Kid {
    sha3_256(public_key)
}

/// HKDF-SHA3-256 with no salt, expanding `ikm` under `info` to one 32-byte key (the seal KDF,
/// sw-wire contract §2).
pub fn hkdf_sha3_256(ikm: &[u8], info: &[u8]) -> Result<[u8; 32], CryptoError> {
    let mut okm = [0u8; 32];
    Hkdf::<Sha3_256>::new(None, ikm)
        .expand(info, &mut okm)
        .map_err(|_| CryptoError::Malformed("hkdf output length"))?;
    Ok(okm)
}
