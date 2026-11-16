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

/// A sign-in assertion (identity-api §7) whose issuer signature verified. Whether its
/// audience, expiry and device suit an `ENROL` is the gate's decision, not the port's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignInClaims {
    /// The signed-in Person's identity public key.
    pub identity: Vec<u8>,
    /// `<slug>#<base64url(SHA3-256(Endpoint signing pk))>`.
    pub device: String,
    pub aud: String,
    pub exp_s: u64,
}

/// What `sw-protocol` needs from the portable identity (`citadel-identity`, lane I1): the
/// fingerprint of an identity public key, verification of a signature made by one, and the
/// parse-and-verify half of a sign-in assertion.
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
    /// Parses `blob` and checks its version and its signature by `issuer`. It does not check
    /// the nonce: the node never saw the sign-in challenge (the MCP checked it).
    fn open_assertion(&self, blob: &[u8], issuer: &[u8]) -> Result<SignInClaims, IdentityError>;
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
