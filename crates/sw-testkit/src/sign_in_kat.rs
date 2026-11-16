//! The values of `fixtures/sign-in-kat.json` (C0) that `ENROL` by assertion needs: the
//! issuer key rebuilt from its `xi`, and the published assertion, re-minted here byte for
//! byte so the stand-in codec is proven to be the contract's.

use serde_json::Value as Json;
use sw_crypto::SigningKey;
use sw_model::EndpointKind;
use sw_protocol::body::{Audience, Body};
use sw_protocol::payload::{Enrol, EnrolOp, Payload, PubKeys};
use sw_protocol::ports::{IdentityPort, SignInClaims};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Bytes;

use crate::assertion::sign_assertion;
use crate::kat::{Kat, fixed, same, text, uint};
use crate::model::unhex;
use crate::{FIXTURE_NOW_MS, StandInIdentityPort};

#[derive(Debug)]
pub struct SignInKat {
    /// work.avarok.net's server identity key (the pinned issuer).
    pub issuer: SigningKey,
    pub blob: Vec<u8>,
    pub claims: SignInClaims,
    pub nonce: [u8; 32],
}

fn b64(j: &Json, path: &[&str]) -> Result<Vec<u8>, String> {
    data_encoding::BASE64URL_NOPAD
        .decode(text(j, path)?.as_bytes())
        .map_err(|e| format!("sign-in kat: {}: {e}", path.join(".")))
}

impl SignInKat {
    pub fn parse(json: &str) -> Result<Self, String> {
        let j: Json = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let issuer = SigningKey::from_seed(&fixed::<32>(&j, &["issuer", "key", "xi_hex"])?);
        same(
            "issuer pk",
            issuer.public_key(),
            text(&j, &["issuer", "key", "pk_hex"])?,
        )?;
        let claims = SignInClaims {
            identity: b64(&j, &["assertion", "identity_pk_b64url"])?,
            device: text(&j, &["assertion", "device"])?.to_owned(),
            aud: text(&j, &["assertion", "aud"])?.to_owned(),
            exp_s: uint(&j, &["assertion", "exp_s"])?,
        };
        let fp = StandInIdentityPort
            .id_fp(&claims.identity)
            .map_err(|e| format!("sign-in kat: {e:?}"))?;
        if fp.as_str() != text(&j, &["assertion", "identity_id_fp"])? {
            return Err("sign-in kat: identity id_fp does not match".into());
        }
        let nonce = fixed::<32>(&j, &["assertion", "nonce_hex"])?;
        let blob = b64(&j, &["assertion", "blob_b64url"])?;
        if sign_assertion(&issuer, &claims, &nonce) != blob {
            return Err("sign-in kat: re-minting the assertion gives other bytes".into());
        }
        let opened = StandInIdentityPort
            .open_assertion(&blob, issuer.public_key())
            .map_err(|e| format!("sign-in kat: open: {e:?}"))?;
        if opened != claims
            || unhex(text(&j, &["assertion", "body_hex"])?)?
                != blob[1..blob.len() - sw_crypto::SIGNATURE_LEN]
        {
            return Err("sign-in kat: the blob does not open to its published fields".into());
        }
        Ok(Self {
            issuer,
            blob,
            claims,
            nonce,
        })
    }
}

/// The KAT Device's `ENROL` by assertion: signed by its own key (which the published
/// assertion names), from its own CID, carrying `assertion`.
pub fn enrol_by_assertion(kat: &Kat, assertion: &[u8]) -> SwMessage {
    let payload = Payload::Enrol(Enrol {
        op: EnrolOp::Enrol,
        epoch: 1,
        endpoint_cid: Some(kat.device_cid),
        kind: Some(EndpointKind::Device),
        pubkeys: Some(PubKeys {
            sign: Bytes(kat.device.public_key().to_vec()),
            seal: Some(Bytes(kat.device_kem.encapsulation_key().to_vec())),
        }),
        kid: None,
        assertion: Some(Bytes(assertion.to_vec())),
    });
    let body = Body {
        iss: kat.person.clone(),
        from: kat.device_cid,
        aud: Audience::Node,
        iat: FIXTURE_NOW_MS,
        exp: FIXTURE_NOW_MS + 60_000,
        nonce: [0x5a; 16],
        seq: 1,
        payload,
    };
    SwMessage::signed(&body, &kat.device).expect("the KAT enrol encodes")
}
