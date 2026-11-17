//! The Device's own secrets (design §5.4; contract §6): an ML-DSA-65 signing key, an ML-KEM-768
//! sealing key and the Citadel account password. All three are *generated* here, never derived
//! from the identity seed, which `sw-mcp` never holds (D-B).
//!
//! `keys/device.cbor` = `{1: sign seed bstr(32), 2: kem seed bstr(64), 3: password tstr}`.

use ciborium::value::Value;
use data_encoding::BASE64URL_NOPAD;
use sw_crypto::{Entropy, KEM_SEED_LEN, KemSecretKey, SIGN_SEED_LEN, SigningKey, kid_of};
use sw_protocol::cbor;
use zeroize::Zeroizing;

/// 32 random bytes as base64url: the Citadel account password (43 characters).
const PASSWORD_BYTES: usize = 32;

pub struct DeviceKeys {
    sign_seed: Zeroizing<[u8; SIGN_SEED_LEN]>,
    kem_seed: Zeroizing<[u8; KEM_SEED_LEN]>,
    pub sign: SigningKey,
    pub kem: KemSecretKey,
    pub citadel_password: Zeroizing<String>,
}

impl std::fmt::Debug for DeviceKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceKeys")
            .field("sign", &self.sign)
            .field("kem", &self.kem)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeysError(pub &'static str);

impl std::fmt::Display for KeysError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "keys/device.cbor: {}", self.0)
    }
}

impl std::error::Error for KeysError {}

impl DeviceKeys {
    pub fn generate(entropy: &mut dyn Entropy) -> Self {
        let mut sign_seed = Zeroizing::new([0u8; SIGN_SEED_LEN]);
        let mut kem_seed = Zeroizing::new([0u8; KEM_SEED_LEN]);
        let mut password = Zeroizing::new([0u8; PASSWORD_BYTES]);
        entropy.fill(sign_seed.as_mut());
        entropy.fill(kem_seed.as_mut());
        entropy.fill(password.as_mut());
        let password = Zeroizing::new(BASE64URL_NOPAD.encode(password.as_ref()));
        Self::from_parts(sign_seed, kem_seed, password)
    }

    fn from_parts(
        sign_seed: Zeroizing<[u8; SIGN_SEED_LEN]>,
        kem_seed: Zeroizing<[u8; KEM_SEED_LEN]>,
        citadel_password: Zeroizing<String>,
    ) -> Self {
        Self {
            sign: SigningKey::from_seed(&sign_seed),
            kem: KemSecretKey::from_seed(&kem_seed),
            sign_seed,
            kem_seed,
            citadel_password,
        }
    }

    pub fn to_bytes(&self) -> Result<Zeroizing<Vec<u8>>, KeysError> {
        let map = Value::Map(vec![
            (1u8.into(), Value::Bytes(self.sign_seed.to_vec())),
            (2u8.into(), Value::Bytes(self.kem_seed.to_vec())),
            (3u8.into(), Value::Text(self.citadel_password.to_string())),
        ]);
        cbor::encode(&map)
            .map(Zeroizing::new)
            .map_err(|_| KeysError("does not encode"))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, KeysError> {
        let Ok(Value::Map(entries)) = cbor::decode(bytes) else {
            return Err(KeysError("is not a canonical CBOR map"));
        };
        let field = |k: u8| {
            entries
                .iter()
                .find(|(key, _)| *key == Value::from(k))
                .map(|(_, v)| v.clone())
        };
        let (Some(Value::Bytes(s)), Some(Value::Bytes(k)), Some(Value::Text(p))) =
            (field(1), field(2), field(3))
        else {
            return Err(KeysError("lacks a field"));
        };
        let (s, k, p) = (Zeroizing::new(s), Zeroizing::new(k), Zeroizing::new(p));
        if entries.len() != 3 {
            return Err(KeysError("has an unknown field"));
        }
        let mut sign_seed = Zeroizing::new([0u8; SIGN_SEED_LEN]);
        let mut kem_seed = Zeroizing::new([0u8; KEM_SEED_LEN]);
        if s.len() != SIGN_SEED_LEN || k.len() != KEM_SEED_LEN {
            return Err(KeysError("holds a seed of the wrong length"));
        }
        sign_seed.copy_from_slice(&s);
        kem_seed.copy_from_slice(&k);
        Ok(Self::from_parts(sign_seed, kem_seed, p))
    }

    /// `<slug>#<base64url(SHA3-256(signing pk))>`: the `device` the sign-in assertion binds.
    pub fn device_label(&self, slug: &str) -> String {
        format!(
            "{slug}#{}",
            BASE64URL_NOPAD.encode(&kid_of(self.sign.public_key()))
        )
    }
}
