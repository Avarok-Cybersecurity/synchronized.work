//! A stand-in for `citadel-identity` (lane I1) behind `sw_protocol::ports::IdentityPort`,
//! following master plan §5.1 so fixtures carry realistic `id_fp`s. Replace with the real
//! crate's adapter when it lands; nothing outside tests and fixture generation uses this.

use hmac::{Hmac, KeyInit, Mac};
use sha3::Sha3_256;
use sw_crypto::{SigningKey, sha3_256, verify_signature};
use sw_model::IdFp;
use sw_protocol::ports::{IdentityError, IdentityPort, MessageSigner};

#[derive(Debug)]
pub struct StandInIdentity {
    key: SigningKey,
}

impl StandInIdentity {
    /// `xi = HMAC-SHA3-256(seed, "citadel-identity-sign-v1")`; ML-DSA-65 KeyGen(xi).
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        let mut mac =
            <Hmac<Sha3_256> as KeyInit>::new_from_slice(seed).expect("HMAC accepts any key length");
        mac.update(b"citadel-identity-sign-v1");
        let xi: [u8; 32] = mac.finalize().into_bytes().into();
        Self {
            key: SigningKey::from_seed(&xi),
        }
    }

    pub fn id_fp(&self) -> IdFp {
        fingerprint(self.key.public_key())
    }
}

/// 16 lowercase base32 characters of `SHA3-256("citadel-identity-fp-v1" ‖ 0 ‖ pk)[..10]`.
fn fingerprint(pk: &[u8]) -> IdFp {
    let digest = sha3_256(&[b"citadel-identity-fp-v1".as_slice(), &[0], pk].concat());
    let text = data_encoding::BASE32_NOPAD
        .encode(&digest[..10])
        .to_ascii_lowercase();
    IdFp::parse(&text).expect("10 bytes are 16 base32 characters")
}

impl MessageSigner for StandInIdentity {
    fn public_key(&self) -> &[u8] {
        self.key.public_key()
    }
    fn sign(&self, message: &[u8]) -> Vec<u8> {
        self.key.sign(message).to_vec()
    }
}

/// Verifies identity signatures for any key (the port is stateless).
#[derive(Debug, Clone, Copy)]
pub struct StandInIdentityPort;

impl IdentityPort for StandInIdentityPort {
    fn id_fp(&self, public_key: &[u8]) -> Result<IdFp, IdentityError> {
        if public_key.len() != sw_crypto::SIGN_PUBLIC_KEY_LEN {
            return Err(IdentityError::Malformed);
        }
        Ok(fingerprint(public_key))
    }
    fn verify(&self, pk: &[u8], message: &[u8], sig: &[u8]) -> Result<(), IdentityError> {
        verify_signature(pk, message, sig).map_err(|_| IdentityError::BadSignature)
    }
}
