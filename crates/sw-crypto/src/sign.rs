use ml_dsa::signature::{Keypair, Signer, Verifier};
use ml_dsa::{EncodedSignature, EncodedVerifyingKey, MlDsa65, Seed, Signature, VerifyingKey};
use zeroize::Zeroize;

use crate::{CryptoError, Entropy};

pub const SIGN_SEED_LEN: usize = 32;
pub const SIGN_PUBLIC_KEY_LEN: usize = 1952;
pub const SIGNATURE_LEN: usize = 3309;

/// An ML-DSA-65 signing key expanded from a 32-byte seed. Signing is deterministic.
pub struct SigningKey {
    inner: ml_dsa::SigningKey<MlDsa65>,
    public: [u8; SIGN_PUBLIC_KEY_LEN],
}

impl core::fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SigningKey")
            .field("kid", &crate::kid_of(&self.public)[..4].to_vec())
            .finish_non_exhaustive()
    }
}

impl SigningKey {
    pub fn from_seed(seed: &[u8; SIGN_SEED_LEN]) -> Self {
        let mut xi = Seed::default();
        xi.copy_from_slice(seed);
        let inner = ml_dsa::SigningKey::<MlDsa65>::from_seed(&xi);
        xi.as_mut_slice().zeroize();
        let mut public = [0u8; SIGN_PUBLIC_KEY_LEN];
        public.copy_from_slice(inner.verifying_key().encode().as_slice());
        Self { inner, public }
    }

    /// A fresh key from 32 bytes of injected entropy.
    pub fn generate(entropy: &mut dyn Entropy) -> Self {
        let mut seed = [0u8; SIGN_SEED_LEN];
        entropy.fill(&mut seed);
        let key = Self::from_seed(&seed);
        seed.zeroize();
        key
    }

    pub fn public_key(&self) -> &[u8; SIGN_PUBLIC_KEY_LEN] {
        &self.public
    }

    pub fn sign(&self, message: &[u8]) -> [u8; SIGNATURE_LEN] {
        let signature: Signature<MlDsa65> = self.inner.sign(message);
        let mut out = [0u8; SIGNATURE_LEN];
        out.copy_from_slice(signature.encode().as_slice());
        out
    }
}

/// Accepts only a valid ML-DSA-65 signature by `public_key` over `message`.
pub fn verify_signature(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), CryptoError> {
    let encoded_key = EncodedVerifyingKey::<MlDsa65>::try_from(public_key)
        .map_err(|_| CryptoError::Malformed("ML-DSA-65 public key"))?;
    let key = VerifyingKey::<MlDsa65>::decode(&encoded_key);
    let encoded_sig = EncodedSignature::<MlDsa65>::try_from(signature)
        .map_err(|_| CryptoError::Malformed("ML-DSA-65 signature"))?;
    let sig = Signature::<MlDsa65>::decode(&encoded_sig)
        .ok_or(CryptoError::Malformed("ML-DSA-65 signature"))?;
    key.verify(message, &sig)
        .map_err(|_| CryptoError::BadSignature)
}
