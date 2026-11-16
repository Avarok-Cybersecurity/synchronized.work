//! `Sealed {1: kem, 2: rcpt, 3: ct, 4: aead, 5: n, 6: enc}`: a signal's inner map encrypted to
//! one Device's ML-KEM-768 key (sw-wire contract §2). `key = HKDF-SHA-256(ss, "sw/seal/1")`;
//! `enc = AES-256-GCM(key, n, aad = ∅, canonical CBOR of the inner map)`; `rcpt = SHA-256(ek)`.
//! The inner map carries `aud` (the Device CID) plus the type's payload fields.

use sw_crypto::{
    AEAD_NONCE_LEN, Entropy, KEM_CIPHERTEXT_LEN, KemSecretKey, aead_open, aead_seal, hkdf_sha256,
    kem_encapsulate, kid_of,
};

use crate::wire::{Bytes, Wire, wire_map};
use crate::{WireError, cbor};

pub const KEM_ALG: &str = "ML-KEM-768";
pub const AEAD_ALG: &str = "AES-256-GCM";
pub const SEAL_INFO: &[u8] = b"sw/seal/1";

wire_map! {
    pub struct Sealed {
        1 => kem: String,
        /// `SHA-256` of the Device's encapsulation key.
        2 => rcpt: [u8; 32],
        3 => ct: Bytes,
        4 => aead: String,
        5 => n: [u8; 12],
        6 => enc: Bytes,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealError {
    UnknownAlg,
    WrongRecipient,
    Malformed,
    OpenFailed,
    Wire(WireError),
}

impl Sealed {
    /// Seals `inner` to `encapsulation_key`. Entropy is drawn in a fixed order — the 32-byte
    /// ML-KEM message `m`, then the 12-byte AES-GCM nonce — so a known-answer stream
    /// reproduces a fixture byte for byte.
    pub fn seal(
        encapsulation_key: &[u8],
        inner: &impl Wire,
        entropy: &mut dyn Entropy,
    ) -> Result<Self, SealError> {
        let (ct, ss) =
            kem_encapsulate(encapsulation_key, entropy).map_err(|_| SealError::Malformed)?;
        let key = hkdf_sha256(ss.as_slice(), SEAL_INFO).map_err(|_| SealError::Malformed)?;
        let mut n = [0u8; AEAD_NONCE_LEN];
        entropy.fill(&mut n);
        let plaintext = cbor::encode(&inner.to_value()).map_err(SealError::Wire)?;
        let enc = aead_seal(&key, &n, &plaintext, &[]).map_err(|_| SealError::Malformed)?;
        Ok(Self {
            kem: KEM_ALG.to_owned(),
            rcpt: kid_of(encapsulation_key),
            ct: Bytes(ct.to_vec()),
            aead: AEAD_ALG.to_owned(),
            n,
            enc: Bytes(enc),
        })
    }

    /// The algorithm names and ciphertext length, checkable by the node without opening.
    pub fn check_shape(&self) -> Result<(), SealError> {
        if self.kem != KEM_ALG || self.aead != AEAD_ALG {
            return Err(SealError::UnknownAlg);
        }
        if self.ct.0.len() != KEM_CIPHERTEXT_LEN {
            return Err(SealError::Malformed);
        }
        Ok(())
    }

    /// The inner plaintext bytes, for this Device's key only.
    pub fn open_bytes(&self, key: &KemSecretKey) -> Result<Vec<u8>, SealError> {
        self.check_shape()?;
        if kid_of(key.encapsulation_key()) != self.rcpt {
            return Err(SealError::WrongRecipient);
        }
        let ss = key
            .decapsulate(&self.ct.0)
            .map_err(|_| SealError::Malformed)?;
        let aead_key = hkdf_sha256(ss.as_slice(), SEAL_INFO).map_err(|_| SealError::Malformed)?;
        aead_open(&aead_key, &self.n, &self.enc.0, &[]).map_err(|_| SealError::OpenFailed)
    }

    /// Opens and decodes the inner map. The caller checks its `aud` against its own CID.
    pub fn open<T: Wire>(&self, key: &KemSecretKey) -> Result<T, SealError> {
        let plaintext = self.open_bytes(key)?;
        let value = cbor::decode(&plaintext).map_err(SealError::Wire)?;
        T::from_value(value).map_err(SealError::Wire)
    }
}
