//! The scratchpad reader's gate (types 18, 19). A reader trusts nothing the node says about
//! keys: each message carries its sender Device's ENROL-by-assertion, whose assertion the
//! pinned issuer signed, and the message is verified against that Device alone.

use sw_crypto::kid_of;
use sw_model::{Cid, EndpointKind, IdFp};

use super::checks::enrolling_endpoint;
use super::{KeyDirectory, KeyEntry, KeyRole, Refusal, Verified, Verifier, verify};
use crate::body::Body;
use crate::payload::{EnrolOp, Payload};
use crate::ports::IdentityPort;
use crate::replay::ReplayGuard;
use crate::signed::{SwMessage, signed_message};

/// A Device whose keys an issuer-signed assertion binds to a Person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundDevice {
    pub id_fp: IdFp,
    pub cid: Cid,
    pub sign_pk: Vec<u8>,
    /// ML-KEM-1024 encapsulation key.
    pub seal_pk: Vec<u8>,
}

/// Verifies a Device's ENROL by assertion as a proof of its keys. The assertion is judged
/// at the ENROL's own `iat`: it had to be valid when the Device enrolled; removing a Device
/// later is `REVOKE`'s job, not expiry's.
pub fn verify_proof(
    bytes: &[u8],
    sign_in_issuer: &[u8],
    identity: &dyn IdentityPort,
) -> Result<BoundDevice, Refusal> {
    let Ok(SwMessage::Signed { typ: 11, signed }) = SwMessage::from_bytes(bytes) else {
        return Err(Refusal::BadProof);
    };
    let at = Body::from_bytes(&signed.body.0).map_err(|_| Refusal::BadProof)?;
    let verifier = Verifier::Node {
        session_cid: at.from,
        sign_in_issuer,
    };
    let message = signed_message(&signed.body.0);
    let (body, _) = enrolling_endpoint(&signed, &message, verifier, identity, at.iat)?;
    let Payload::Enrol(e) = body.payload else {
        return Err(Refusal::BadProof);
    };
    match (e.op, e.kind, e.pubkeys, e.endpoint_cid) {
        (EnrolOp::Enrol, Some(EndpointKind::Device), Some(keys), Some(cid)) => {
            let seal_pk = keys.seal.ok_or(Refusal::BadProof)?.0;
            Ok(BoundDevice {
                id_fp: body.iss,
                cid,
                sign_pk: keys.sign.0,
                seal_pk,
            })
        }
        _ => Err(Refusal::BadProof),
    }
}

struct OneDevice(KeyEntry);

impl KeyDirectory for OneDevice {
    fn key(&self, kid: &sw_crypto::Kid) -> Option<KeyEntry> {
        (kid_of(&self.0.public_key) == *kid).then(|| self.0.clone())
    }
}

/// The proof a scratchpad message carries, before anything is verified.
fn carried_proof(message: &SwMessage) -> Result<Vec<u8>, Refusal> {
    let SwMessage::Signed {
        typ: 18 | 19,
        signed,
    } = message
    else {
        return Err(Refusal::TypMismatch);
    };
    match Body::from_bytes(&signed.body.0)?.payload {
        Payload::PadNote(n) => Ok(n.proof.0),
        Payload::PadAck(a) => Ok(a.proof.0),
        _ => Err(Refusal::TypMismatch),
    }
}

/// Verifies a relayed 18 / 19 end to end: its proof, then the shared gate against the one
/// Device that proof binds (signature, `iss`, `from`, audience, lifetime, time). Replay is
/// the caller's: a reader de-duplicates by `signal_id` until `exp`.
pub fn verify_pad(
    message: &SwMessage,
    sign_in_issuer: &[u8],
    identity: &dyn IdentityPort,
    now_ms: u64,
) -> Result<(Verified, BoundDevice), Refusal> {
    let device = verify_proof(&carried_proof(message)?, sign_in_issuer, identity)?;
    let directory = OneDevice(KeyEntry {
        public_key: device.sign_pk.clone(),
        owner: device.id_fp.clone(),
        role: KeyRole::Endpoint {
            cid: device.cid,
            kind: EndpointKind::Device,
        },
    });
    let mut replay = ReplayGuard::empty();
    let verified = verify(
        message,
        &directory,
        identity,
        &mut replay,
        Verifier::Reader,
        now_ms,
    )?;
    Ok((verified, device))
}
