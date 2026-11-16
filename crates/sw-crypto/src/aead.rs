use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};

use crate::CryptoError;

pub const AEAD_KEY_LEN: usize = 32;
pub const AEAD_NONCE_LEN: usize = 12;

/// AES-256-GCM over `plaintext` with associated data `aad`. The caller supplies the nonce
/// (from injected entropy); a key is never reused across seals because each seal derives a
/// fresh key from a fresh ML-KEM shared secret.
pub fn aead_seal(
    key: &[u8; AEAD_KEY_LEN],
    nonce: &[u8; AEAD_NONCE_LEN],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::Malformed("AES key"))?;
    let n = Nonce::try_from(&nonce[..]).map_err(|_| CryptoError::Malformed("AES-GCM nonce"))?;
    cipher
        .encrypt(
            &n,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| CryptoError::Malformed("AES-GCM plaintext length"))
}

pub fn aead_open(
    key: &[u8; AEAD_KEY_LEN],
    nonce: &[u8],
    ciphertext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::Malformed("AES key"))?;
    let n = Nonce::try_from(nonce).map_err(|_| CryptoError::Malformed("AES-GCM nonce"))?;
    cipher
        .decrypt(
            &n,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| CryptoError::OpenFailed)
}
