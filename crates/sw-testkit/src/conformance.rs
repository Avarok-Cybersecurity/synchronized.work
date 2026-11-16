//! The conformance check against C0's frozen fixtures (`fixtures/sw-wire/`, mirrored byte for
//! byte from `docs/contracts/batch-1116/fixtures/sw-wire/`). One function, run natively by
//! `cargo test` and `xtask conformance` and on wasm32 by `wasm-bindgen-test`, so "decodes
//! identically on both targets" is the same code judging the same bytes.

use serde_json::Value as Json;
use sw_protocol::body::{Audience, Body};
use sw_protocol::cbor;
use sw_protocol::gate::{Verifier, verify, verify_p2p};
use sw_protocol::p2p::decode_p2p;
use sw_protocol::payload::{Payload, PromptInner};
use sw_protocol::record::{record_from_bytes, record_to_bytes};
use sw_protocol::replay::ReplayGuard;
use sw_protocol::signed::{SwMessage, signed_message};

use crate::kat::Kat;
use crate::model::{hex, to_model, unhex};
use crate::sign_in_kat::{SignInKat, enrol_by_assertion};
use crate::{ReplayedEntropy, StandInIdentityPort};

/// Judges one fixture file. An `invalid-*` fixture passes only if it is refused.
pub fn check(file: &str, fixture: &str, kat: &Kat, sign_in: &SignInKat) -> Result<(), String> {
    let j: Json = serde_json::from_str(fixture).map_err(|e| format!("{file}: {e}"))?;
    let field = |k: &str| {
        j.get(k)
            .and_then(Json::as_str)
            .ok_or(format!("{file}: no {k}"))
    };
    let bytes = unhex(field("cbor_hex")?)?;
    let model = j.get("model").ok_or(format!("{file}: no model"))?;
    let judged = judge(&j, &bytes, model, kat, sign_in);
    match (file.starts_with("invalid-"), judged) {
        (false, Ok(())) | (true, Err(_)) => Ok(()),
        (false, Err(e)) => Err(format!("{file}: {e}")),
        (true, Ok(())) => Err(format!("{file}: accepted bytes it must refuse")),
    }
}

fn judge(
    j: &Json,
    bytes: &[u8],
    model: &Json,
    kat: &Kat,
    sign_in: &SignInKat,
) -> Result<(), String> {
    let value = cbor::decode(bytes).map_err(|e| format!("not canonical CBOR: {e}"))?;
    if &to_model(&value)? != model {
        return Err("decodes to something other than its model".into());
    }
    match (
        j.get("typ").and_then(Json::as_u64),
        j.get("name").and_then(Json::as_str),
    ) {
        (Some(typ), _) => message(bytes, typ, j, kat, sign_in),
        (None, Some("p2p-envelope")) => {
            let inner = decode_p2p(bytes).map_err(|e| format!("p2p envelope: {e}"))?;
            let msg = round_trip(&inner)?;
            let body = body_of(&msg)?;
            let own = endpoint_aud(&body)?;
            verify_p2p(
                &msg,
                &kat.node_directory(),
                &StandInIdentityPort,
                &mut ReplayGuard::empty(),
                own,
                body.iat,
            )
            .map(|_| ())
            .map_err(|r| format!("p2p gate refused: {r:?}"))
        }
        (None, Some("person-record")) => {
            let record = record_from_bytes(bytes).map_err(|e| format!("record: {e}"))?;
            let again = record_to_bytes(&record).map_err(|e| format!("record: {e}"))?;
            let keys_match = record.id_fp == kat.person && record.pk == kat.identity_pk;
            (again == bytes && keys_match)
                .then_some(())
                .ok_or("record does not round-trip to the KAT".into())
        }
        (None, other) => Err(format!("no rule for untyped fixture {other:?}")),
    }
}

fn round_trip(bytes: &[u8]) -> Result<SwMessage, String> {
    let msg = SwMessage::from_bytes(bytes).map_err(|e| format!("decode: {e}"))?;
    if msg.to_bytes().map_err(|e| format!("re-encode: {e}"))? != bytes {
        return Err("does not re-encode to the same bytes".into());
    }
    Ok(msg)
}

fn body_of(msg: &SwMessage) -> Result<Body, String> {
    match msg {
        SwMessage::Signed { signed, .. } => {
            Body::from_bytes(&signed.body.0).map_err(|e| format!("body: {e}"))
        }
        SwMessage::Unsigned(_) => Err("expected a signed message".into()),
    }
}

fn endpoint_aud(body: &Body) -> Result<u64, String> {
    match body.aud {
        Audience::Endpoint(cid) => Ok(cid),
        Audience::Node => Err("expected an Endpoint audience".into()),
    }
}

fn message(bytes: &[u8], typ: u64, j: &Json, kat: &Kat, sign_in: &SignInKat) -> Result<(), String> {
    let msg = round_trip(bytes)?;
    let SwMessage::Signed { typ: outer, signed } = &msg else {
        let SwMessage::Unsigned(p) = &msg else {
            unreachable!()
        };
        return (u64::from(p.typ()) == typ)
            .then_some(())
            .ok_or("typ".into());
    };
    if u64::from(*outer) != typ {
        return Err("SwMessage[1] is not the fixture's typ".into());
    }
    let body = body_of(&msg)?;
    let dir = kat.node_directory();
    let node = Verifier::Node {
        session_cid: body.from,
        sign_in_issuer: sign_in.issuer.public_key(),
    };
    verify(
        &msg,
        &dir,
        &StandInIdentityPort,
        &mut ReplayGuard::empty(),
        node,
        body.iat,
    )
    .map_err(|r| format!("node gate refused: {r:?}"))?;
    if let Audience::Endpoint(cid) = body.aud {
        verify(
            &msg,
            &dir,
            &StandInIdentityPort,
            &mut ReplayGuard::empty(),
            Verifier::Endpoint { cid },
            body.iat,
        )
        .map_err(|r| format!("endpoint gate refused: {r:?}"))?;
    }
    let resign = |key: &sw_crypto::SigningKey| key.sign(&signed_message(&signed.body.0)).to_vec();
    let same_sig = if signed.kid == sw_crypto::kid_of(kat.device.public_key()) {
        resign(&kat.device) == signed.sig.0
    } else if signed.kid == sw_crypto::kid_of(kat.controller.public_key()) {
        resign(&kat.controller) == signed.sig.0
    } else {
        true // the identity's seed is not in the sw KAT; its signature was verified above
    };
    if !same_sig {
        return Err("deterministic re-signing gives different bytes".into());
    }
    match &body.payload {
        Payload::Prompt(sealed) => prompt_kat(sealed, j, kat),
        _ => Ok(()),
    }
}

/// `04-prompt-sealed`: the Device opens it to the published plaintext, and sealing that
/// plaintext again with the published `m` and nonce gives the same `Sealed` bytes.
fn prompt_kat(sealed: &sw_protocol::sealed::Sealed, j: &Json, kat: &Kat) -> Result<(), String> {
    let want = |k: &str| j.get(k).and_then(Json::as_str).ok_or(format!("no {k}"));
    let ss = kat
        .device_kem
        .decapsulate(&sealed.ct.0)
        .map_err(|e| e.to_string())?;
    if hex(ss.as_slice()) != want("shared_secret_hex")? {
        return Err("shared secret differs from the KAT".into());
    }
    let key = sw_protocol::sealed::seal_key(ss.as_slice()).map_err(|e| format!("{e:?}"))?;
    if hex(&key) != want("aead_key_hex")? {
        return Err("AEAD key differs from the KAT".into());
    }
    let plaintext = sealed
        .open_bytes(&kat.device_kem)
        .map_err(|e| format!("open: {e:?}"))?;
    if hex(&plaintext) != want("inner_plaintext_hex")? {
        return Err("inner plaintext differs from the KAT".into());
    }
    let inner: PromptInner = sealed
        .open(&kat.device_kem)
        .map_err(|e| format!("open: {e:?}"))?;
    if inner.aud != kat.device_cid {
        return Err("inner aud is not the Device".into());
    }
    let mut entropy =
        ReplayedEntropy::new([unhex(want("kem_m_hex")?)?, sealed.n.to_vec()].concat());
    let again =
        sw_protocol::sealed::Sealed::seal(kat.device_kem.encapsulation_key(), &inner, &mut entropy)
            .map_err(|e| format!("reseal: {e:?}"))?;
    (again == *sealed && entropy.exhausted())
        .then_some(())
        .ok_or("resealing does not reproduce the fixture".into())
}

/// `ENROL` by assertion with the published sign-in assertion: the KAT Device, unknown to the
/// node, enrols itself under the KAT Person, and the gate returns the assertion's claims.
pub fn check_sign_in_enrol(kat: &Kat, sign_in: &SignInKat) -> Result<(), String> {
    let msg = enrol_by_assertion(kat, &sign_in.blob);
    let verified = verify(
        &msg,
        &crate::Directory::default(),
        &StandInIdentityPort,
        &mut ReplayGuard::empty(),
        Verifier::Node {
            session_cid: kat.device_cid,
            sign_in_issuer: sign_in.issuer.public_key(),
        },
        crate::FIXTURE_NOW_MS,
    )
    .map_err(|r| format!("enrol by assertion refused: {r:?}"))?;
    (verified.sign_in.as_ref() == Some(&sign_in.claims))
        .then_some(())
        .ok_or("the gate did not return the assertion's claims".into())
}
