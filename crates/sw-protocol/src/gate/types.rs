use sw_crypto::Kid;
use sw_model::{Cid, EndpointKind, IdFp};

use super::AssertionRefusal;
use crate::WireError;
use crate::body::Body;
use crate::ports::SignInClaims;

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
pub enum Verifier<'a> {
    /// The node: `from` must be the session's CID; it accepts its own audience, and an
    /// Endpoint audience only for the types it relays (4, 5, 6, 17). `sign_in_issuer` is
    /// work.avarok.net's pinned server identity key, the only issuer of `ENROL` assertions.
    Node {
        session_cid: Cid,
        sign_in_issuer: &'a [u8],
    },
    /// A Device or Controller: only messages addressed to its own CID.
    Endpoint { cid: Cid },
    /// A scratchpad reader: a node-addressed 18 / 19 relayed to it, verified against the
    /// sender's own enrolment proof rather than anything the node supplies.
    Reader,
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
    /// `ENROL` by assertion whose assertion does not enrol this key here.
    Assertion(AssertionRefusal),
    /// A scratchpad proof that is not a Device's ENROL by assertion, or names another key.
    BadProof,
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
    /// For `ENROL` by assertion only: the assertion the gate accepted (pinned issuer,
    /// audience, expiry, device kid, and `id_fp(identity) == body.iss` all checked).
    pub sign_in: Option<SignInClaims>,
}
