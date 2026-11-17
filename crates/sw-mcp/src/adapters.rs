//! The native adapters for the pure crates' ports: the OS CSPRNG (`Entropy`), the wall clock
//! (`Clock`) and lane I1's `citadel-identity` (`IdentityPort`).

use citadel_identity::{MlDsa65Pk, fingerprint};
use sw_model::IdFp;
use sw_protocol::ports::{Clock, IdentityError, IdentityPort, SignInClaims};

/// The operating system's CSPRNG. `getrandom` failing means the OS cannot supply secure
/// randomness; nothing here may continue without it, so the process stops.
#[derive(Debug, Clone, Copy)]
pub struct OsEntropy;

impl sw_crypto::Entropy for OsEntropy {
    fn fill(&mut self, out: &mut [u8]) {
        if let Err(e) = getrandom::fill(out) {
            eprintln!("sw-mcp: the OS random source failed ({e}); stopping");
            std::process::abort();
        }
    }
}

impl OsEntropy {
    pub fn array<const N: usize>(&mut self) -> [u8; N] {
        let mut out = [0u8; N];
        sw_crypto::Entropy::fill(self, &mut out);
        out
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        let since = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
    }
}

/// `citadel-identity` behind `IdentityPort`: the fingerprint, signature and assertion formulas
/// live there once (SSOT).
#[derive(Debug, Clone, Copy)]
pub struct CitadelIdentity;

fn pk(bytes: &[u8]) -> Result<MlDsa65Pk, IdentityError> {
    MlDsa65Pk::from_bytes(bytes).map_err(|_| IdentityError::Malformed)
}

impl IdentityPort for CitadelIdentity {
    fn id_fp(&self, public_key: &[u8]) -> Result<IdFp, IdentityError> {
        IdFp::parse(&fingerprint(&pk(public_key)?)).map_err(|_| IdentityError::Malformed)
    }

    fn verify(&self, public_key: &[u8], message: &[u8], sig: &[u8]) -> Result<(), IdentityError> {
        pk(public_key)?;
        sw_crypto::verify_signature(public_key, message, sig)
            .map_err(|_| IdentityError::BadSignature)
    }

    /// The port leaves expiry to its caller (the node's gate compares `exp_s` with its own
    /// clock), so the crate's expiry check runs here against the epoch, which no issued
    /// assertion predates.
    fn open_assertion(&self, blob: &[u8], issuer: &[u8]) -> Result<SignInClaims, IdentityError> {
        let opened = citadel_identity::open_assertion(blob, &pk(issuer)?, 0)
            .map_err(|_| IdentityError::BadSignature)?;
        Ok(SignInClaims {
            identity: opened.identity.as_bytes().to_vec(),
            device: opened.device,
            aud: opened.aud,
            exp_s: opened.exp_s,
        })
    }
}
