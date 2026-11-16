use sw_crypto::Kid;
use sw_model::{Cid, EndpointKind, IdFp};

use crate::WireError;
use crate::body::Body;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRole {
    /// The Person's portable identity key.
    Identity,
    Endpoint {
        cid: Cid,
        kind: EndpointKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEntry {
    pub public_key: Vec<u8>,
    pub owner: IdFp,
    pub role: KeyRole,
}

/// Resolves a `kid` to the key it names (node: the Person records; Device: its allowlist).
pub trait KeyDirectory {
    fn key(&self, kid: &Kid) -> Option<KeyEntry>;
}

/// Who is verifying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verifier {
    /// The node: `from` must be the session's CID; it accepts its own audience, and an
    /// Endpoint audience only for the types it relays (4, 5, 6, 17).
    Node { session_cid: Cid },
    /// A Device or Controller: only messages addressed to its own CID.
    Endpoint { cid: Cid },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Wire(WireError),
    UnknownAlg,
    UnknownKid,
    BadSignature,
    /// The key's role cannot sign this type (e.g. a Controller key on a `STATUS_UPDATE`).
    WrongSigner,
    /// `iss` is not the key's owner, or the identity key's fingerprint is not `iss`.
    IssuerMismatch,
    /// `from` is not the signing Endpoint's CID, or not the session's CID.
    FromMismatch,
    WrongAudience,
    FromFuture,
    Expired,
    LifetimeTooLong,
    Replay,
    SeqNotIncreasing,
    /// A signature on a type that travels unsigned, or none on one that must be signed.
    SigningMismatch,
    /// `SwMessage[1]` is not `Body[1]`.
    TypMismatch,
    /// A type that never travels over P2P arrived there.
    NotP2pEligible,
}

impl From<WireError> for Refusal {
    fn from(e: WireError) -> Self {
        Self::Wire(e)
    }
}

/// A body that passed every check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    pub body: Body,
    pub kid: Kid,
    pub signal_id: [u8; 32],
    /// For `ENROL` by assertion only: the gate checked the enrolling Device's own signature;
    /// the node must still verify this sign-in assertion (identity contract §7) and that its
    /// identity's `id_fp` is `body.iss` before enrolling anything.
    pub assertion_to_verify: Option<Vec<u8>>,
}
