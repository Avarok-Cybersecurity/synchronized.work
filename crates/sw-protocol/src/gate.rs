//! The verification gate (design §4.2, §6.1). One function for the node and for every
//! Device, on the C2S path and the P2P path alike; only the key directory and the expected
//! audience differ. On a Device the directory *is* its local allowlist.

use sw_crypto::{Kid, verify_signature};
use sw_model::{Cid, EndpointKind, IdFp};

use crate::WireError;
use crate::body::{Audience, Body};
use crate::payload::{Payload, SignerRole, limits};
use crate::ports::IdentityPort;
use crate::replay::{ReplayGuard, ReplayRefusal};
use crate::signed::{SIGN_ALG, Signed, SwMessage, signed_message};

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
    /// The node: `from` must be the session's CID; it accepts its own audience, and a
    /// Device audience only for the signals it relays.
    Node { session_cid: Cid },
    /// A Device: only messages addressed to its own CID.
    Device { cid: Cid },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

/// Accepts an unsigned `SwMessage` only for a type that travels unsigned.
pub fn accept_unsigned(message: SwMessage) -> Result<Payload, Refusal> {
    match message {
        SwMessage::Unsigned(p) if p.signer().is_none() => Ok(p),
        _ => Err(Refusal::SigningMismatch),
    }
}

/// Verifies `signed` and records it in `replay`. Nothing is recorded unless everything
/// passed, and the replay checks run only after the signature verified.
pub fn verify(
    signed: &Signed,
    directory: &dyn KeyDirectory,
    identity: &dyn IdentityPort,
    replay: &mut ReplayGuard,
    verifier: Verifier,
    now_ms: u64,
) -> Result<Verified, Refusal> {
    if signed.alg != SIGN_ALG {
        return Err(Refusal::UnknownAlg);
    }
    let entry = directory.key(&signed.kid).ok_or(Refusal::UnknownKid)?;
    let message = signed_message(&signed.body.0);
    match entry.role {
        KeyRole::Identity => identity
            .verify(&entry.public_key, &message, &signed.sig.0)
            .map_err(|_| Refusal::BadSignature)?,
        KeyRole::Endpoint { .. } => verify_signature(&entry.public_key, &message, &signed.sig.0)
            .map_err(|_| Refusal::BadSignature)?,
    }
    let body = Body::from_bytes(&signed.body.0)?;
    check_signer(&body, &entry, identity)?;
    check_audience(&body, verifier)?;
    check_time(&body, now_ms)?;
    replay
        .check(&signed.kid, &body.nonce, body.seq)
        .map_err(|r| match r {
            ReplayRefusal::Replay => Refusal::Replay,
            ReplayRefusal::SeqNotIncreasing => Refusal::SeqNotIncreasing,
        })?;
    let signal_id = signed.signal_id()?;
    replay.record(&signed.kid, &body.nonce, body.seq, body.exp);
    Ok(Verified {
        body,
        kid: signed.kid,
        signal_id,
    })
}

fn check_signer(body: &Body, entry: &KeyEntry, identity: &dyn IdentityPort) -> Result<(), Refusal> {
    let needed = body.payload.signer().ok_or(Refusal::SigningMismatch)?;
    if body.iss != entry.owner {
        return Err(Refusal::IssuerMismatch);
    }
    match (needed, entry.role) {
        (SignerRole::Identity, KeyRole::Identity) => {
            let fp = identity
                .id_fp(&entry.public_key)
                .map_err(|_| Refusal::IssuerMismatch)?;
            if fp != body.iss {
                return Err(Refusal::IssuerMismatch);
            }
            Ok(())
        }
        (SignerRole::Identity, KeyRole::Endpoint { .. }) | (_, KeyRole::Identity) => {
            Err(Refusal::WrongSigner)
        }
        (role, KeyRole::Endpoint { cid, kind }) => {
            let kind_ok = match role {
                SignerRole::Device => kind == EndpointKind::Device,
                SignerRole::Controller => kind == EndpointKind::Controller,
                SignerRole::Endpoint | SignerRole::Identity => true,
            };
            if !kind_ok {
                return Err(Refusal::WrongSigner);
            }
            if body.from != cid {
                return Err(Refusal::FromMismatch);
            }
            Ok(())
        }
    }
}

fn check_audience(body: &Body, verifier: Verifier) -> Result<(), Refusal> {
    let relayed = matches!(body.payload, Payload::Prompt(_) | Payload::Control(_));
    let ok = match (verifier, body.aud) {
        (Verifier::Node { session_cid }, aud) => {
            if body.from != session_cid {
                return Err(Refusal::FromMismatch);
            }
            match aud {
                Audience::Node => !relayed,
                Audience::Device(_) => relayed,
            }
        }
        (Verifier::Device { cid }, Audience::Device(to)) => to == cid,
        (Verifier::Device { .. }, Audience::Node) => false,
    };
    if ok {
        Ok(())
    } else {
        Err(Refusal::WrongAudience)
    }
}

fn check_time(body: &Body, now_ms: u64) -> Result<(), Refusal> {
    if body.iat > now_ms.saturating_add(limits::MAX_CLOCK_SKEW_MS) {
        return Err(Refusal::FromFuture);
    }
    if body.exp <= now_ms {
        return Err(Refusal::Expired);
    }
    if body.exp < body.iat || body.exp - body.iat > body.payload.max_lifetime_ms() {
        return Err(Refusal::LifetimeTooLong);
    }
    Ok(())
}
