//! `sw_protocol::ports::IdentityPort` over the portable identity crate (`citadel-identity`,
//! frozen identity-api contract): the production adapter the node verifies with.

use citadel_identity::{MlDsa65Pk, fingerprint};
use sw_model::IdFp;
use sw_protocol::ports::{IdentityError, IdentityPort, SignInClaims};

#[derive(Debug, Clone, Copy, Default)]
pub struct CitadelIdentity;

impl IdentityPort for CitadelIdentity {
    fn id_fp(&self, public_key: &[u8]) -> Result<IdFp, IdentityError> {
        let pk = MlDsa65Pk::from_bytes(public_key).map_err(|_| IdentityError::Malformed)?;
        IdFp::parse(&fingerprint(&pk)).map_err(|_| IdentityError::Malformed)
    }

    /// ML-DSA-65, pure, empty context: the primitive identity signatures use (identity-api §2),
    /// which `sw-crypto` implements for every party.
    fn verify(
        &self,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), IdentityError> {
        sw_crypto::verify_signature(public_key, message, signature)
            .map_err(|_| IdentityError::BadSignature)
    }

    /// The port leaves expiry to the gate, which holds the clock; the crate's `open_assertion`
    /// checks it against the `now_s` it is given, so it is given the epoch, before every
    /// assertion's `exp`.
    fn open_assertion(&self, blob: &[u8], issuer: &[u8]) -> Result<SignInClaims, IdentityError> {
        let issuer = MlDsa65Pk::from_bytes(issuer).map_err(|_| IdentityError::Malformed)?;
        let a = citadel_identity::open_assertion(blob, &issuer, 0)
            .map_err(|_| IdentityError::BadSignature)?;
        Ok(SignInClaims {
            identity: a.identity.as_bytes().to_vec(),
            device: a.device,
            aud: a.aud,
            exp_s: a.exp_s,
        })
    }
}
