use ml_kem::{
    B32, Ciphertext, Decapsulate, EncapsulationKey, KeyExport, MlKem1024, Seed, TryKeyInit,
};
use zeroize::{Zeroize, Zeroizing};

use crate::{CryptoError, Entropy};

pub const KEM_SEED_LEN: usize = 64;
pub const KEM_ENCAPSULATION_KEY_LEN: usize = 1568;
pub const KEM_CIPHERTEXT_LEN: usize = 1568;

/// The 32-byte ML-KEM shared secret, wiped on drop.
pub type SharedSecret = Zeroizing<[u8; 32]>;

/// An ML-KEM-1024 decapsulation (sealing) key expanded from a 64-byte seed.
pub struct KemSecretKey {
    inner: ml_kem::DecapsulationKey<MlKem1024>,
    public: Vec<u8>,
}

impl core::fmt::Debug for KemSecretKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KemSecretKey")
            .field("kid", &crate::kid_of(&self.public)[..4].to_vec())
            .finish_non_exhaustive()
    }
}

impl KemSecretKey {
    pub fn from_seed(seed: &[u8; KEM_SEED_LEN]) -> Self {
        let mut s = Seed::default();
        s.copy_from_slice(seed);
        let inner = ml_kem::DecapsulationKey::<MlKem1024>::from_seed(s);
        s.as_mut_slice().zeroize();
        let public = inner.encapsulation_key().to_bytes().to_vec();
        Self { inner, public }
    }

    pub fn generate(entropy: &mut dyn Entropy) -> Self {
        let mut seed = [0u8; KEM_SEED_LEN];
        entropy.fill(&mut seed);
        let key = Self::from_seed(&seed);
        seed.zeroize();
        key
    }

    /// The 1568-byte encapsulation key a sender seals to.
    pub fn encapsulation_key(&self) -> &[u8] {
        &self.public
    }

    pub fn decapsulate(&self, ciphertext: &[u8]) -> Result<SharedSecret, CryptoError> {
        let ct = Ciphertext::<MlKem1024>::try_from(ciphertext)
            .map_err(|_| CryptoError::Malformed("ML-KEM-1024 ciphertext"))?;
        let shared = self.inner.decapsulate(&ct);
        let mut out = Zeroizing::new([0u8; 32]);
        out.copy_from_slice(shared.as_slice());
        Ok(out)
    }
}

/// Encapsulates to `encapsulation_key` with 32 bytes of injected entropy as the message `m`.
pub fn kem_encapsulate(
    encapsulation_key: &[u8],
    entropy: &mut dyn Entropy,
) -> Result<([u8; KEM_CIPHERTEXT_LEN], SharedSecret), CryptoError> {
    let ek = EncapsulationKey::<MlKem1024>::new_from_slice(encapsulation_key)
        .map_err(|_| CryptoError::Malformed("ML-KEM-1024 encapsulation key"))?;
    let mut m = B32::default();
    entropy.fill(m.as_mut_slice());
    let (ct, shared) = ek.encapsulate_deterministic(&m);
    m.as_mut_slice().zeroize();
    let mut ct_out = [0u8; KEM_CIPHERTEXT_LEN];
    ct_out.copy_from_slice(ct.as_slice());
    let mut ss = Zeroizing::new([0u8; 32]);
    ss.copy_from_slice(shared.as_slice());
    Ok((ct_out, ss))
}
