//! `xtask seal-kat`: regenerates every fixture that carries the Device's sealing key or a
//! `Sealed` envelope from the seeds already in the KATs, after a change to the KEM or the
//! envelope version. Deterministic, so a second run changes nothing:
//!
//! - `sw-wire-kat.json`: the Device's encapsulation key, its kid and the KDF line;
//! - `04-prompt-sealed`: resealed from its published plaintext, KEM `m` and AEAD nonce, then
//!   re-signed by the controller;
//! - `11-enrol`: the Device key it announces, re-signed by the identity (`identity-kat` A);
//! - `person-record`: the Device key it lists.
//!
//! Afterwards `xtask conformance` judges the result like any other mirror.

use std::path::Path;

use ciborium::value::Value;
use serde_json::{Value as Json, json};
use sw_crypto::{KemSecretKey, kid_of};
use sw_protocol::body::Body;
use sw_protocol::cbor;
use sw_protocol::payload::PromptInner;
use sw_protocol::sealed::{KEM_ALG, SEAL_INFO, Sealed, seal_key};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Wire;
use sw_testkit::kat::Kat;
use sw_testkit::model::{hex, to_model, unhex};
use sw_testkit::{ReplayedEntropy, StandInIdentity};

use crate::pin;

fn load(path: &Path) -> Result<Json, String> {
    serde_json::from_str(&pin::read(path)?).map_err(|e| format!("{}: {e}", path.display()))
}

fn store(path: &Path, j: &Json) -> Result<(), String> {
    let text = serde_json::to_string_pretty(j).map_err(|e| e.to_string())? + "\n";
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn field<'a>(j: &'a Json, key: &str) -> Result<&'a str, String> {
    j[key].as_str().ok_or(format!("missing {key}"))
}

/// Replaces every byte string equal to `old` with `new`, anywhere in `value`.
fn swap_bytes(value: &mut Value, old: &[u8], new: &[u8]) {
    match value {
        Value::Bytes(b) if b.as_slice() == old => *b = new.to_vec(),
        Value::Array(items) => items.iter_mut().for_each(|v| swap_bytes(v, old, new)),
        Value::Map(entries) => entries
            .iter_mut()
            .for_each(|(_, v)| swap_bytes(v, old, new)),
        _ => {}
    }
}

/// The AEAD nonce of the (only) sealed envelope in a body value.
fn sealed_nonce(value: &Value) -> Option<Vec<u8>> {
    let Value::Map(entries) = value else {
        return None;
    };
    let alg = entries.iter().find(|(k, _)| k == &Value::Integer(1.into()));
    if let Some((_, Value::Text(t))) = alg
        && t.starts_with("ML-KEM")
    {
        return entries.iter().find_map(|(k, v)| match (k, v) {
            (Value::Integer(i), Value::Bytes(n)) if i128::from(*i) == 5 => Some(n.clone()),
            _ => None,
        });
    }
    entries.iter().find_map(|(_, v)| sealed_nonce(v))
}

fn body_value(msg: &[u8]) -> Result<Value, String> {
    let env = cbor::decode(msg).map_err(|e| format!("{e:?}"))?;
    let Value::Map(outer) = env else {
        return Err("not a map".into());
    };
    let signed = outer
        .into_iter()
        .find_map(|(k, v)| (k == Value::Integer(3.into())).then_some(v))
        .ok_or("no signed part")?;
    let Value::Map(signed) = signed else {
        return Err("signed part is not a map".into());
    };
    let body = signed
        .into_iter()
        .find_map(|(k, v)| match (k, v) {
            (Value::Integer(i), Value::Bytes(b)) if i128::from(i) == 3 => Some(b),
            _ => None,
        })
        .ok_or("no body")?;
    cbor::decode(&body).map_err(|e| format!("{e:?}"))
}

fn resign(
    value: &Value,
    signer: &dyn sw_protocol::ports::MessageSigner,
) -> Result<Vec<u8>, String> {
    let bytes = cbor::encode(value).map_err(|e| format!("{e:?}"))?;
    let body = Body::from_bytes(&bytes).map_err(|e| format!("body: {e:?}"))?;
    SwMessage::signed(&body, signer)
        .and_then(|m| m.to_bytes())
        .map_err(|e| format!("{e:?}"))
}

fn rewrite(path: &Path, j: &mut Json, bytes: &[u8]) -> Result<(), String> {
    j["cbor_hex"] = json!(hex(bytes));
    j["model"] = to_model(&cbor::decode(bytes).map_err(|e| format!("{e:?}"))?)?;
    store(path, j)
}

pub fn run(root: &Path) -> Result<String, String> {
    let dir = root.join("fixtures");
    let kat_path = dir.join("sw-wire-kat.json");
    let mut kat_json = load(&kat_path)?;
    let device = &kat_json["device"];
    let kem = KemSecretKey::from_seed(
        &unhex(field(device, "kem_seed_hex")?)?
            .try_into()
            .map_err(|_| "kem seed is not 64 bytes")?,
    );
    let old_ek = unhex(field(device, "kem_ek_hex")?)?;
    let ek = kem.encapsulation_key().to_vec();
    kat_json["device"]["kem_ek_hex"] = json!(hex(&ek));
    kat_json["device"]["kem_kid_hex"] = json!(hex(&kid_of(&ek)));
    kat_json["seal"]["kdf"] = json!(format!(
        "HKDF-SHA3-256(salt = none, ikm = {KEM_ALG} shared secret, info = \"{}\", L = 32)",
        String::from_utf8_lossy(SEAL_INFO)
    ));
    store(&kat_path, &kat_json)?;
    let kat = Kat::parse(&pin::read(&kat_path)?)?;

    let ids = load(&dir.join("identity-kat.json"))?;
    let seed: [u8; 32] = unhex(field(&ids["identities"]["A"], "seed_hex")?)?
        .try_into()
        .map_err(|_| "identity seed is not 32 bytes")?;
    let identity = StandInIdentity::from_seed(&seed);
    if identity.id_fp() != kat.person {
        return Err("identity-kat A is not the sw KAT's Person".into());
    }

    let p = dir.join("sw-wire/04-prompt-sealed.json");
    let mut j = load(&p)?;
    let mut body = body_value(&unhex(field(&j, "cbor_hex")?)?)?;
    let n = sealed_nonce(&body).ok_or("no sealed envelope in 04")?;
    let m = unhex(field(&j, "kem_m_hex")?)?;
    let plain = unhex(field(&j, "inner_plaintext_hex")?)?;
    let inner = PromptInner::from_value(cbor::decode(&plain).map_err(|e| format!("{e:?}"))?)
        .map_err(|e| format!("{e:?}"))?;
    let mut entropy = ReplayedEntropy::new([m.clone(), n].concat());
    let sealed = Sealed::seal(&ek, &inner, &mut entropy).map_err(|e| format!("{e:?}"))?;
    let ss = kem
        .decapsulate(&sealed.ct.0)
        .map_err(|e| format!("{e:?}"))?;
    j["shared_secret_hex"] = json!(hex(ss.as_slice()));
    j["aead_key_hex"] = json!(hex(&seal_key(ss.as_slice()).map_err(|e| format!("{e:?}"))?));
    let Value::Map(entries) = &mut body else {
        return Err("04 body is not a map".into());
    };
    for (k, v) in entries.iter_mut() {
        if k == &Value::Integer(9.into()) {
            *v = sealed.to_value();
        }
    }
    rewrite(&p, &mut j, &resign(&body, &kat.controller)?)?;

    let p = dir.join("sw-wire/11-enrol.json");
    let mut j = load(&p)?;
    let mut body = body_value(&unhex(field(&j, "cbor_hex")?)?)?;
    swap_bytes(&mut body, &old_ek, &ek);
    rewrite(&p, &mut j, &resign(&body, &identity)?)?;

    let p = dir.join("sw-wire/person-record.json");
    let mut j = load(&p)?;
    let mut record = cbor::decode(&unhex(field(&j, "cbor_hex")?)?).map_err(|e| format!("{e:?}"))?;
    swap_bytes(&mut record, &old_ek, &ek);
    rewrite(
        &p,
        &mut j,
        &cbor::encode(&record).map_err(|e| format!("{e:?}"))?,
    )?;

    Ok(format!("seal-kat: fixtures resealed to {KEM_ALG}"))
}
