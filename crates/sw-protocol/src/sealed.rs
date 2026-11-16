//! `Sealed`: a signal's inner body encrypted to one Device's ML-KEM-768 key (design §4.2,
//! §4.4). Key = HKDF-SHA3-256(ML-KEM shared secret, "sw/seal/1"); AES-256-GCM binds the
//! recipient, the KEM ciphertext and the nonce as associated data.

use sw_crypto::{
    AEAD_NONCE_LEN, Entropy, KEM_CIPHERTEXT_LEN, KemSecretKey, aead_open, aead_seal, hkdf_sha3_256,
    kem_encapsulate, kid_of,
};
use sw_model::Cid;

use crate::wire::{Bytes, MapIn, MapOut, Raw, Wire, wire_map};
use crate::{WireError, cbor};

pub const KEM_ALG: &str = "ML-KEM-768";
pub const AEAD_ALG: &str = "AES-256-GCM";
pub const SEAL_INFO: &[u8] = b"sw/seal/1";

wire_map! {
    pub struct Sealed {
        0 => kem: String,
        /// `kid` of the Device's sealing (encapsulation) key.
        1 => rcpt: [u8; 32],
        2 => ct: Bytes,
        3 => aead: String,
        4 => n: [u8; 12],
        5 => enc: Bytes,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealError {
    UnknownAlg,
    WrongRecipient,
    Malformed,
    OpenFailed,
    Wire(WireError),
}

fn aad(rcpt: &[u8; 32], ct: &[u8], n: &[u8; 12]) -> Vec<u8> {
    [SEAL_INFO, rcpt, ct, n].concat()
}

impl Sealed {
    /// Seals `inner` for the Device whose encapsulation key is `encapsulation_key` and whose
    /// CID is `aud`. All randomness (KEM message, AEAD nonce) comes from `entropy`.
    pub fn seal(
        encapsulation_key: &[u8],
        aud: Cid,
        inner: &impl Wire,
        entropy: &mut dyn Entropy,
    ) -> Result<Self, SealError> {
        let (ct, ss) =
            kem_encapsulate(encapsulation_key, entropy).map_err(|_| SealError::Malformed)?;
        let key = hkdf_sha3_256(ss.as_slice(), SEAL_INFO).map_err(|_| SealError::Malformed)?;
        let mut n = [0u8; AEAD_NONCE_LEN];
        entropy.fill(&mut n);
        let rcpt = kid_of(encapsulation_key);
        let mut map = MapOut::default();
        map.field(0, &aud);
        map.field(1, &Raw(inner.to_value()));
        let plaintext = cbor::encode(&map.build()).map_err(SealError::Wire)?;
        let enc = aead_seal(&key, &n, &plaintext, &aad(&rcpt, &ct, &n))
            .map_err(|_| SealError::Malformed)?;
        Ok(Self {
            kem: KEM_ALG.to_owned(),
            rcpt,
            ct: Bytes(ct.to_vec()),
            aead: AEAD_ALG.to_owned(),
            n,
            enc: Bytes(enc),
        })
    }

    /// The algorithm names and ciphertext length the node can check without opening.
    pub fn check_shape(&self) -> Result<(), SealError> {
        if self.kem != KEM_ALG || self.aead != AEAD_ALG {
            return Err(SealError::UnknownAlg);
        }
        if self.ct.0.len() != KEM_CIPHERTEXT_LEN {
            return Err(SealError::Malformed);
        }
        Ok(())
    }

    /// Opens with this Device's key, returning the inner audience and payload.
    pub fn open<T: Wire>(&self, key: &KemSecretKey) -> Result<(Cid, T), SealError> {
        self.check_shape()?;
        if kid_of(key.encapsulation_key()) != self.rcpt {
            return Err(SealError::WrongRecipient);
        }
        let ss = key
            .decapsulate(&self.ct.0)
            .map_err(|_| SealError::Malformed)?;
        let aead_key = hkdf_sha3_256(ss.as_slice(), SEAL_INFO).map_err(|_| SealError::Malformed)?;
        let plaintext = aead_open(
            &aead_key,
            &self.n,
            &self.enc.0,
            &aad(&self.rcpt, &self.ct.0, &self.n),
        )
        .map_err(|_| SealError::OpenFailed)?;
        let mut map = MapIn::new(cbor::decode(&plaintext).map_err(SealError::Wire)?)
            .map_err(SealError::Wire)?;
        let aud: Cid = map.field(0).map_err(SealError::Wire)?;
        let inner: T = map.field(1).map_err(SealError::Wire)?;
        map.finish().map_err(SealError::Wire)?;
        Ok((aud, inner))
    }
}
