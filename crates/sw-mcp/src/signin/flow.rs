//! The I/O half of sign-in: the issuer behind [`IssuerPort`] (HTTPS in production), the issuer
//! key pinned on first use (`/.well-known/citadel`, `check_against_pin` after), and the
//! assertion kept in `keys/assertion.bin` (0600).

use std::future::Future;

use ciborium::value::Value;
use citadel_identity::{
    MlDsa65Pk, WellKnownError, canonical_domain, check_against_pin, open_assertion,
    parse_well_known,
};
use sw_protocol::cbor;

use super::{AcceptError, AcceptedAssertion, CodeError, PendingSignIn, accept, complete_request};
use crate::home::{Home, read_private, write_private};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuerReply {
    pub status: u16,
    pub body: Vec<u8>,
}

/// work.avarok.net's two endpoints the MCP uses.
pub trait IssuerPort {
    /// `GET /.well-known/citadel`.
    fn well_known(&self) -> impl Future<Output = Result<Vec<u8>, String>> + Send;
    /// `POST /sign-in/complete` with a JSON body.
    fn complete(&self, json: Vec<u8>) -> impl Future<Output = Result<IssuerReply, String>> + Send;
}

#[derive(Debug)]
pub enum SignInError {
    Code(CodeError),
    Transport(String),
    WellKnown(WellKnownError),
    /// The issuer answered non-200; `reason` is its reason code (contract §1 step 4).
    Refused {
        status: u16,
        reason: String,
    },
    Accept(AcceptError),
    Store(String),
    /// No assertion is stored, or it no longer verifies.
    NotSignedIn(String),
}

impl std::fmt::Display for SignInError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Code(e) => write!(
                f,
                "the code is not XXXX-XXXX in the sign-in alphabet ({e:?})"
            ),
            Self::Transport(e) => write!(f, "could not reach the issuer: {e}"),
            Self::WellKnown(e) => write!(f, "the issuer's /.well-known/citadel: {e}"),
            Self::Refused { status, reason } => {
                write!(f, "the issuer refused the code ({status}): {reason}")
            }
            Self::Accept(e) => write!(f, "the issuer's assertion was refused: {e:?}"),
            Self::Store(e) => write!(f, "could not store the sign-in: {e}"),
            Self::NotSignedIn(why) => write!(f, "not signed in: {why}"),
        }
    }
}

impl std::error::Error for SignInError {}

fn store_err(e: impl std::fmt::Display) -> SignInError {
    SignInError::Store(e.to_string())
}

/// Bytes a refusal body may show, after which it is cut (it is the issuer's text).
const REASON_BYTES: usize = 120;

pub async fn complete_sign_in<P: IssuerPort>(
    port: &P,
    home: &Home,
    pending: &PendingSignIn,
    code: &str,
    device: &str,
    issuer: &str,
    now_s: u64,
) -> Result<AcceptedAssertion, SignInError> {
    let request = complete_request(pending, code, device).map_err(SignInError::Code)?;
    let issuer_pk = pin_issuer(port, home, issuer).await?;
    let json = serde_json::to_vec(&request).map_err(store_err)?;
    let reply = port.complete(json).await.map_err(SignInError::Transport)?;
    if reply.status != 200 {
        let text = String::from_utf8_lossy(&reply.body);
        let reason = crate::frame::sanitize(&text)
            .chars()
            .take(REASON_BYTES)
            .collect();
        return Err(SignInError::Refused {
            status: reply.status,
            reason,
        });
    }
    let accepted = accept(pending, &reply.body, issuer, &issuer_pk, device, now_s)
        .map_err(SignInError::Accept)?;
    write_private(&home.assertion(), &accepted.blob).map_err(store_err)?;
    Ok(accepted)
}

/// Fetches and verifies the issuer's document; pins `(pk, issued_at)` the first time and
/// refuses a changed key or an older document after (`KeyChanged`, `Stale`).
pub async fn pin_issuer<P: IssuerPort>(
    port: &P,
    home: &Home,
    issuer: &str,
) -> Result<MlDsa65Pk, SignInError> {
    let domain = canonical_domain(issuer).map_err(|e| SignInError::Transport(e.to_string()))?;
    let bytes = port.well_known().await.map_err(SignInError::Transport)?;
    let doc = parse_well_known(&bytes, &domain).map_err(SignInError::WellKnown)?;
    if let Some((pk, issued_at)) = read_pin(home)? {
        check_against_pin(&doc, &pk, issued_at).map_err(SignInError::WellKnown)?;
    }
    let pin = Value::Map(vec![
        (1u8.into(), Value::Bytes(doc.server_key.as_bytes().to_vec())),
        (2u8.into(), Value::from(doc.issued_at_s)),
    ]);
    let pin = cbor::encode(&pin).map_err(|e| store_err(format!("{e:?}")))?;
    write_private(&home.issuer_pin(), &pin).map_err(store_err)?;
    Ok(doc.server_key)
}

fn read_pin(home: &Home) -> Result<Option<(MlDsa65Pk, u64)>, SignInError> {
    let Some(bytes) = read_private(&home.issuer_pin()).map_err(store_err)? else {
        return Ok(None);
    };
    let bad = || SignInError::Store("keys/issuer-pin.cbor is not {pk, issued_at}".into());
    let Ok(Value::Map(fields)) = cbor::decode(&bytes) else {
        return Err(bad());
    };
    match fields.as_slice() {
        [(_, Value::Bytes(pk)), (_, Value::Integer(at))] => {
            let pk = MlDsa65Pk::from_bytes(pk).map_err(|_| bad())?;
            let at = u64::try_from(*at).map_err(|_| bad())?;
            Ok(Some((pk, at)))
        }
        _ => Err(bad()),
    }
}

/// The stored assertion, re-verified against the pinned issuer key at `now_s`.
pub fn load_assertion(home: &Home, now_s: u64) -> Result<AcceptedAssertion, SignInError> {
    let Some((issuer_pk, _)) = read_pin(home)? else {
        return Err(SignInError::NotSignedIn(
            "no issuer is pinned; run sign_in_start".into(),
        ));
    };
    let Some(blob) = read_private(&home.assertion()).map_err(store_err)? else {
        return Err(SignInError::NotSignedIn(
            "no assertion is stored; run sign_in_start".into(),
        ));
    };
    let a = open_assertion(&blob, &issuer_pk, now_s)
        .map_err(|e| SignInError::NotSignedIn(format!("the stored assertion: {e}")))?;
    if a.aud != super::CLIENT {
        return Err(SignInError::NotSignedIn(format!(
            "the stored assertion is for {:?}",
            a.aud
        )));
    }
    Ok(AcceptedAssertion {
        identity_pk: a.identity.as_bytes().to_vec(),
        device: a.device,
        exp_s: a.exp_s,
        blob,
    })
}
