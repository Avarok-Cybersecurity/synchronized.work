use sw_crypto::{kid_of, verify_signature};
use sw_model::EndpointKind;

use super::assertion::check_sign_in;
use super::{KeyEntry, KeyRole, Refusal, Verifier};
use crate::body::{Audience, Body};
use crate::payload::{Payload, SignerRole, limits};
use crate::ports::{IdentityPort, SignInClaims};
use crate::signed::Signed;

/// An unknown `kid` is acceptable only for `ENROL` by assertion at the node, signed by the
/// very key it enrols, from the CID it enrols, under an assertion that enrols that key.
pub(super) fn enrolling_endpoint(
    signed: &Signed,
    message: &[u8],
    verifier: Verifier<'_>,
    identity: &dyn IdentityPort,
    now_ms: u64,
) -> Result<(Body, Option<SignInClaims>), Refusal> {
    let Verifier::Node { sign_in_issuer, .. } = verifier else {
        return Err(Refusal::UnknownKid);
    };
    let body = Body::from_bytes(&signed.body.0).map_err(|_| Refusal::UnknownKid)?;
    let Payload::Enrol(enrol) = &body.payload else {
        return Err(Refusal::UnknownKid);
    };
    let (Some(assertion), Some(keys)) = (&enrol.assertion, &enrol.pubkeys) else {
        return Err(Refusal::UnknownKid);
    };
    if kid_of(&keys.sign.0) != signed.kid {
        return Err(Refusal::UnknownKid);
    }
    verify_signature(&keys.sign.0, message, &signed.sig.0).map_err(|_| Refusal::BadSignature)?;
    if enrol.endpoint_cid != Some(body.from) {
        return Err(Refusal::FromMismatch);
    }
    let claims = check_sign_in(
        &assertion.0,
        sign_in_issuer,
        &keys.sign.0,
        &body.iss,
        identity,
        now_ms,
    )?;
    Ok((body, Some(claims)))
}

pub(super) fn check_signer(
    body: &Body,
    entry: &KeyEntry,
    identity: &dyn IdentityPort,
) -> Result<(), Refusal> {
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
        (SignerRole::Identity | SignerRole::EnrollingEndpoint, KeyRole::Endpoint { .. })
        | (_, KeyRole::Identity) => Err(Refusal::WrongSigner),
        (role, KeyRole::Endpoint { cid, kind }) => {
            let kind_ok = match role {
                SignerRole::Device => kind == EndpointKind::Device,
                SignerRole::Controller => kind == EndpointKind::Controller,
                SignerRole::Endpoint => true,
                SignerRole::Identity | SignerRole::EnrollingEndpoint => false,
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

pub(super) fn check_audience(body: &Body, verifier: Verifier<'_>) -> Result<(), Refusal> {
    let relayed = body.payload.addressed_to_endpoint();
    let ok = match (verifier, body.aud) {
        (Verifier::Node { session_cid, .. }, aud) => {
            if body.from != session_cid {
                return Err(Refusal::FromMismatch);
            }
            match aud {
                Audience::Node => !relayed,
                Audience::Endpoint(_) => relayed,
            }
        }
        (Verifier::Endpoint { cid }, Audience::Endpoint(to)) => to == cid,
        (Verifier::Endpoint { .. }, Audience::Node) => false,
        (Verifier::Reader, Audience::Node) => matches!(body.payload.typ(), 18 | 19),
        (Verifier::Reader, Audience::Endpoint(_)) => false,
    };
    if ok {
        Ok(())
    } else {
        Err(Refusal::WrongAudience)
    }
}

pub(super) fn check_time(body: &Body, now_ms: u64) -> Result<(), Refusal> {
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
