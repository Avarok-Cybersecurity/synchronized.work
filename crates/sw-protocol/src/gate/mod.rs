//! The verification gate (sw-wire contract §2, design §6.1). One function for the node and
//! every Endpoint, on the C2S path and the P2P path alike; only the key directory and the
//! expected audience differ. On a Device the directory *is* its local allowlist.

mod assertion;
mod checks;
mod pad;
mod types;

pub use assertion::{AssertionRefusal, SIGN_IN_AUDIENCES};
pub use pad::{BoundDevice, verify_pad, verify_proof};
pub use types::{KeyDirectory, KeyEntry, KeyRole, Refusal, Verified, Verifier};

use sw_crypto::verify_signature;
use sw_model::Cid;

use crate::body::Body;
use crate::payload::Payload;
use crate::ports::IdentityPort;
use crate::replay::{ReplayGuard, ReplayRefusal};
use crate::signed::{SIGN_ALG, SwMessage, signed_message};
use checks::{check_audience, check_signer, check_time, enrolling_endpoint};

/// Accepts an unsigned `SwMessage` only for a type that travels unsigned.
pub fn accept_unsigned(message: SwMessage) -> Result<Payload, Refusal> {
    match message {
        SwMessage::Unsigned(p) if p.signer().is_none() => Ok(p),
        _ => Err(Refusal::SigningMismatch),
    }
}

/// The P2P path: the same gate, after refusing what never travels peer to peer.
pub fn verify_p2p(
    message: &SwMessage,
    directory: &dyn KeyDirectory,
    identity: &dyn IdentityPort,
    replay: &mut ReplayGuard,
    own_cid: Cid,
    now_ms: u64,
) -> Result<Verified, Refusal> {
    let Some(typ) = signed_typ(message) else {
        return Err(Refusal::SigningMismatch);
    };
    if !matches!(typ, 3 | 4 | 5 | 6 | 17) {
        return Err(Refusal::NotP2pEligible);
    }
    let verifier = Verifier::Endpoint { cid: own_cid };
    verify(message, directory, identity, replay, verifier, now_ms)
}

fn signed_typ(message: &SwMessage) -> Option<u8> {
    match message {
        SwMessage::Signed { typ, .. } => Some(*typ),
        SwMessage::Unsigned(_) => None,
    }
}

/// Verifies a signed `SwMessage` and records it in `replay`. The signature is checked over
/// the exact body bytes before they are decoded; nothing is recorded unless every check
/// passed, so a forged envelope cannot burn a nonce.
pub fn verify(
    message: &SwMessage,
    directory: &dyn KeyDirectory,
    identity: &dyn IdentityPort,
    replay: &mut ReplayGuard,
    verifier: Verifier<'_>,
    now_ms: u64,
) -> Result<Verified, Refusal> {
    let SwMessage::Signed { typ, signed } = message else {
        return Err(Refusal::SigningMismatch);
    };
    if signed.alg != SIGN_ALG {
        return Err(Refusal::UnknownAlg);
    }
    let message = signed_message(&signed.body.0);
    // An ENROL by assertion always takes the assertion path, even from a key the directory
    // already holds: re-enrolling a Device refreshes the proof its scratchpad notes carry.
    let by_assertion = matches!(
        Body::from_bytes(&signed.body.0).map(|b| b.payload),
        Ok(Payload::Enrol(ref e)) if e.assertion.is_some()
    );
    let known = directory.key(&signed.kid).filter(|_| !by_assertion);
    let (body, sign_in) = match known {
        Some(entry) => {
            match entry.role {
                KeyRole::Identity => identity
                    .verify(&entry.public_key, &message, &signed.sig.0)
                    .map_err(|_| Refusal::BadSignature)?,
                KeyRole::Endpoint { .. } => {
                    verify_signature(&entry.public_key, &message, &signed.sig.0)
                        .map_err(|_| Refusal::BadSignature)?
                }
            }
            let body = Body::from_bytes(&signed.body.0)?;
            check_signer(&body, &entry, identity)?;
            (body, None)
        }
        None => enrolling_endpoint(signed, &message, verifier, identity, now_ms)?,
    };
    if body.payload.typ() != *typ {
        return Err(Refusal::TypMismatch);
    }
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
        sign_in,
    })
}
