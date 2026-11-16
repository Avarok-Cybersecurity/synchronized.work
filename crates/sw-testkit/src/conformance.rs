//! The conformance check (design §11.1 gate 6, §11.4): one function run natively by
//! `cargo test` and `xtask conformance`, and on wasm32 by `wasm-bindgen-test`, so "decodes
//! identically on both targets" is the same code judging the same bytes.

use sw_crypto::sha3_256;
use sw_protocol::body::Audience;
use sw_protocol::cbor;
use sw_protocol::gate::{Verifier, verify};
use sw_protocol::p2p::decode_p2p;
use sw_protocol::payload::{ControlInner, Payload, PromptInner};
use sw_protocol::replay::ReplayGuard;
use sw_protocol::signed::SwMessage;

use crate::{Cast, FIXTURE_NOW_MS, StandInIdentityPort};

/// The diagnostic-notation file that pins a fixture's decoded form.
pub fn diag_name(fixture: &str) -> String {
    format!("{}.diag", fixture.trim_end_matches(".cbor"))
}

pub fn diagnostic(bytes: &[u8]) -> Result<String, String> {
    let value = cbor::decode(bytes).map_err(|e| format!("not canonical CBOR: {e}"))?;
    Ok(cbor::diagnostic(&value))
}

/// `MANIFEST`: one `<sha3-256 hex>  <name>` line per fixture, sorted by name.
pub fn manifest(fixtures: &[(&str, Vec<u8>)]) -> String {
    let mut lines: Vec<String> = fixtures
        .iter()
        .map(|(name, bytes)| {
            let hex: String = sha3_256(bytes).iter().map(|b| format!("{b:02x}")).collect();
            format!("{hex}  {name}\n")
        })
        .collect();
    lines.sort_by(|a, b| a[66..].cmp(&b[66..]));
    lines.concat()
}

/// Accepts a fixture only if it is canonical, its diagnostic text is `expected_diag`, it
/// round-trips byte for byte, and — when signed — it passes the gate, and its sealed signal
/// opens for the Device it names.
pub fn check(name: &str, bytes: &[u8], expected_diag: &str) -> Result<(), String> {
    let fail = |why: String| Err(format!("{name}: {why}"));
    let diag = diagnostic(bytes).map_err(|why| format!("{name}: {why}"))?;
    if diag.trim() != expected_diag.trim() {
        return fail("decodes to a different diagnostic than its .diag file".into());
    }
    let p2p = name.ends_with(".p2p.cbor");
    let inner = if p2p {
        decode_p2p(bytes).map_err(|e| format!("{name}: p2p envelope: {e}"))?
    } else {
        bytes.to_vec()
    };
    let message = SwMessage::from_bytes(&inner).map_err(|e| format!("{name}: decode: {e}"))?;
    if message
        .to_bytes()
        .map_err(|e| format!("{name}: re-encode: {e}"))?
        != inner
    {
        return fail("does not re-encode to the same bytes".into());
    }
    let SwMessage::Signed(signed) = message else {
        return Ok(());
    };
    let cast = Cast::new();
    let dir = cast.node_directory();
    let from = sw_protocol::body::Body::from_bytes(&signed.body.0)
        .map_err(|e| format!("{name}: body: {e}"))?
        .from;
    let verifier = if p2p {
        Verifier::Device {
            cid: cast.alice_device_cid,
        }
    } else {
        Verifier::Node { session_cid: from }
    };
    let mut replay = ReplayGuard::empty();
    let verified = verify(
        &signed,
        &dir,
        &StandInIdentityPort,
        &mut replay,
        verifier,
        FIXTURE_NOW_MS,
    )
    .map_err(|r| format!("{name}: gate refused: {r:?}"))?;
    if p2p && !verified.body.payload.p2p_eligible() {
        return fail("type is not eligible for P2P".into());
    }
    let device = Verifier::Device {
        cid: cast.alice_device_cid,
    };
    match (&verified.body.payload, verified.body.aud) {
        (Payload::Prompt(s), Audience::Device(_)) => {
            verify(
                &signed,
                &cast.device_allowlist(),
                &StandInIdentityPort,
                &mut ReplayGuard::empty(),
                device,
                FIXTURE_NOW_MS,
            )
            .map_err(|r| format!("{name}: device gate refused: {r:?}"))?;
            let (aud, _) = s
                .sealed
                .open::<PromptInner>(&cast.alice_device_kem)
                .map_err(|e| format!("{name}: open: {e:?}"))?;
            (aud == cast.alice_device_cid)
                .then_some(())
                .ok_or(format!("{name}: inner aud"))
        }
        (Payload::Control(s), Audience::Device(_)) => {
            let (aud, _) = s
                .sealed
                .open::<ControlInner>(&cast.alice_device_kem)
                .map_err(|e| format!("{name}: open: {e:?}"))?;
            (aud == cast.alice_device_cid)
                .then_some(())
                .ok_or(format!("{name}: inner aud"))
        }
        _ => Ok(()),
    }
}
