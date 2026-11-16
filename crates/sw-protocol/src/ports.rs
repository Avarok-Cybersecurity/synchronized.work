//! Ports the protocol layer is written against (SBIO): adapters supply time and identity.

use sw_model::IdFp;

/// Wall-clock milliseconds since the Unix epoch.
pub trait Clock {
    fn now_ms(&self) -> u64;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityError {
    Malformed,
    BadSignature,
}

/// What `sw-protocol` needs from the portable identity (`citadel-identity`, lane I1): the
/// fingerprint of an identity public key, and verification of a signature made by one.
/// Deliberately narrow so the identity crate plugs in as an adapter without this crate
/// depending on it, and no second copy of its formulas lives here (SSOT).
pub trait IdentityPort {
    fn id_fp(&self, public_key: &[u8]) -> Result<IdFp, IdentityError>;
    fn verify(
        &self,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), IdentityError>;
}

/// Anything that signs a `Body`: an Endpoint's ML-DSA-65 key here, the identity holder (the
/// Citadel agent, the browser, `sw-mcp`) for `ENROL` / `REVOKE`.
pub trait MessageSigner {
    fn public_key(&self) -> &[u8];
    fn sign(&self, message: &[u8]) -> Vec<u8>;
}

impl MessageSigner for sw_crypto::SigningKey {
    fn public_key(&self) -> &[u8] {
        sw_crypto::SigningKey::public_key(self)
    }
    fn sign(&self, message: &[u8]) -> Vec<u8> {
        sw_crypto::SigningKey::sign(self, message).to_vec()
    }
}
