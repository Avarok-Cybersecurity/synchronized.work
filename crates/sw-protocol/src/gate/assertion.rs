//! `ENROL` by assertion (sign-in-with-citadel §6): the node accepts a self-signed enrol only
//! if work.avarok.net's pinned server key issued an unexpired assertion, for `sw-mcp` or
//! `synchronized.work`, that binds this very signing key to the Person named by `iss`.

use data_encoding::BASE64URL_NOPAD;
use sw_crypto::kid_of;
use sw_model::IdFp;

use super::Refusal;
use crate::ports::{IdentityPort, SignInClaims};

/// The audiences whose assertions may enrol an Endpoint here.
pub const SIGN_IN_AUDIENCES: [&str; 2] = ["sw-mcp", "synchronized.work"];

const DEVICE_SLUG_MAX: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssertionRefusal {
    /// Malformed, or not signed by the pinned issuer key.
    Invalid,
    WrongAudience,
    Expired,
    /// `device` is not `<slug>#<base64url(SHA3-256(pubkeys.sign))>`.
    WrongDevice,
}

pub(super) fn check_sign_in(
    blob: &[u8],
    issuer: &[u8],
    sign_pk: &[u8],
    iss: &IdFp,
    identity: &dyn IdentityPort,
    now_ms: u64,
) -> Result<SignInClaims, Refusal> {
    let refuse = |r| Err(Refusal::Assertion(r));
    let Ok(claims) = identity.open_assertion(blob, issuer) else {
        return refuse(AssertionRefusal::Invalid);
    };
    if !SIGN_IN_AUDIENCES.contains(&claims.aud.as_str()) {
        return refuse(AssertionRefusal::WrongAudience);
    }
    if now_ms / 1000 >= claims.exp_s {
        return refuse(AssertionRefusal::Expired);
    }
    if !device_names_key(&claims.device, sign_pk) {
        return refuse(AssertionRefusal::WrongDevice);
    }
    let fp = identity
        .id_fp(&claims.identity)
        .map_err(|_| Refusal::IssuerMismatch)?;
    if fp != *iss {
        return Err(Refusal::IssuerMismatch);
    }
    Ok(claims)
}

fn device_names_key(device: &str, sign_pk: &[u8]) -> bool {
    let Some((slug, kid)) = device.split_once('#') else {
        return false;
    };
    let slug_ok = (1..=DEVICE_SLUG_MAX).contains(&slug.len())
        && slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    slug_ok && kid == BASE64URL_NOPAD.encode(&kid_of(sign_pk))
}
