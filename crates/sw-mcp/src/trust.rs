//! The local allowlist of Controllers (design §5.4): the only thing the gate consults. A kid
//! enters it when a person runs `sw-mcp trust` and confirms the fingerprint, and leaves it with
//! `sw-mcp revoke` or a root-signed `REVOKE`. The node is never asked.
//!
//! `trusted-controllers.cbor` = `{1: body bstr, 2: sig bstr}`, `sig` = the Device key's
//! ML-DSA-65 signature over `"sw-mcp-trust-v1" ‖ 0x00 ‖ body`; `body` = canonical CBOR of
//! `[{1: kid, 2: sign pk, 3: cid, 4: owner id_fp, 5: label, 6: trusted_at ms, 7: paused}]`.
//! A file that does not verify under this Device's key is refused: nobody else edits it.

use ciborium::value::Value;
use sw_crypto::{Kid, SIGN_PUBLIC_KEY_LEN, SigningKey, kid_of, verify_signature};
use sw_model::{Cid, EndpointKind, IdFp};
use sw_protocol::cbor;
use sw_protocol::gate::{KeyDirectory, KeyEntry, KeyRole};

const TRUST_DOMAIN: &[u8] = b"sw-mcp-trust-v1\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedController {
    pub kid: Kid,
    pub sign_pk: Vec<u8>,
    pub cid: Cid,
    pub owner: IdFp,
    pub label: String,
    pub trusted_at_ms: u64,
    /// Paused after repeated refusals (§9.2) until a person runs `sw-mcp trust` again.
    pub paused: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustList {
    entries: Vec<TrustedController>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustError {
    Malformed(&'static str),
    /// The file is not signed by this Device's key.
    NotOurs,
    WrongKeyLength,
}

impl std::fmt::Display for TrustError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(why) => write!(f, "trusted-controllers.cbor: {why}"),
            Self::NotOurs => f.write_str(
                "trusted-controllers.cbor is not signed by this Device's key; it was changed outside sw-mcp",
            ),
            Self::WrongKeyLength => f.write_str("a Controller key is 1952 bytes (ML-DSA-65)"),
        }
    }
}

impl std::error::Error for TrustError {}

/// The six words a person compares between the Controller's screen and this terminal: the
/// first 66 bits of the kid in the recovery-phrase word list (identity-api §8).
pub fn fingerprint_words(kid: &Kid) -> String {
    citadel_identity::phrase_encode(kid)[..6].join(" ")
}

/// The 8-hex prefix logs use for a kid (§6.8): never key material.
pub fn kid_prefix(kid: &Kid) -> String {
    kid[..4].iter().map(|b| format!("{b:02x}")).collect()
}

impl TrustList {
    pub fn entries(&self) -> &[TrustedController] {
        &self.entries
    }

    pub fn get(&self, kid: &Kid) -> Option<&TrustedController> {
        self.entries.iter().find(|e| e.kid == *kid)
    }

    /// Adds (or re-confirms, which also unpauses) a Controller of `owner`.
    pub fn trust(
        &mut self,
        sign_pk: &[u8],
        cid: Cid,
        owner: IdFp,
        label: String,
        now_ms: u64,
    ) -> Result<Kid, TrustError> {
        if sign_pk.len() != SIGN_PUBLIC_KEY_LEN {
            return Err(TrustError::WrongKeyLength);
        }
        let kid = kid_of(sign_pk);
        self.entries.retain(|e| e.kid != kid);
        self.entries.push(TrustedController {
            kid,
            sign_pk: sign_pk.to_vec(),
            cid,
            owner,
            label,
            trusted_at_ms: now_ms,
            paused: false,
        });
        Ok(kid)
    }

    /// Removes every kid that starts with `prefix` (at least 8 hex characters). Returns them.
    pub fn revoke_prefix(&mut self, prefix: &str) -> Vec<Kid> {
        let prefix = prefix.to_ascii_lowercase();
        if prefix.len() < 8 {
            return Vec::new();
        }
        let hex = |k: &Kid| k.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let gone: Vec<Kid> = self
            .entries
            .iter()
            .filter(|e| hex(&e.kid).starts_with(&prefix))
            .map(|e| e.kid)
            .collect();
        self.entries.retain(|e| !gone.contains(&e.kid));
        gone
    }

    pub fn revoke(&mut self, kid: &Kid) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.kid != *kid);
        before != self.entries.len()
    }

    pub fn pause(&mut self, kid: &Kid) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.kid == *kid) {
            e.paused = true;
        }
    }

    pub fn to_bytes(&self, device: &SigningKey) -> Result<Vec<u8>, TrustError> {
        let entries = self.entries.iter().map(entry_value).collect();
        let body =
            cbor::encode(&Value::Array(entries)).map_err(|_| TrustError::Malformed("encode"))?;
        let sig = device.sign(&[TRUST_DOMAIN, &body].concat());
        let file = Value::Map(vec![
            (1u8.into(), Value::Bytes(body)),
            (2u8.into(), Value::Bytes(sig.to_vec())),
        ]);
        cbor::encode(&file).map_err(|_| TrustError::Malformed("encode"))
    }

    pub fn from_bytes(bytes: &[u8], device_pk: &[u8]) -> Result<Self, TrustError> {
        let pair = match cbor::decode(bytes) {
            Ok(Value::Map(pair)) if pair.len() == 2 => pair,
            _ => return Err(TrustError::Malformed("not a {body, sig} map")),
        };
        let (Value::Bytes(body), Value::Bytes(sig)) = (&pair[0].1, &pair[1].1) else {
            return Err(TrustError::Malformed("body and sig must be bytes"));
        };
        verify_signature(device_pk, &[TRUST_DOMAIN, body].concat(), sig)
            .map_err(|_| TrustError::NotOurs)?;
        let Ok(Value::Array(items)) = cbor::decode(body) else {
            return Err(TrustError::Malformed("body is not an array"));
        };
        let entries = items
            .into_iter()
            .map(entry_from)
            .collect::<Result<_, _>>()?;
        Ok(Self { entries })
    }
}

fn entry_value(e: &TrustedController) -> Value {
    Value::Map(vec![
        (1u8.into(), Value::Bytes(e.kid.to_vec())),
        (2u8.into(), Value::Bytes(e.sign_pk.clone())),
        (3u8.into(), Value::from(e.cid)),
        (4u8.into(), Value::Text(e.owner.as_str().to_owned())),
        (5u8.into(), Value::Text(e.label.clone())),
        (6u8.into(), Value::from(e.trusted_at_ms)),
        (7u8.into(), Value::Bool(e.paused)),
    ])
}

fn entry_from(value: Value) -> Result<TrustedController, TrustError> {
    let bad = TrustError::Malformed("an entry is not {kid, pk, cid, owner, label, at, paused}");
    let Value::Map(fields) = value else {
        return Err(bad);
    };
    let get = |k: u8| {
        fields
            .iter()
            .find(|(key, _)| *key == Value::from(k))
            .map(|(_, v)| v)
    };
    let uint = |v: Option<&Value>| match v {
        Some(Value::Integer(i)) => u64::try_from(*i).ok(),
        _ => None,
    };
    let (Some(Value::Bytes(kid)), Some(Value::Bytes(pk)), Some(Value::Text(owner))) =
        (get(1), get(2), get(4))
    else {
        return Err(bad);
    };
    let (Some(Value::Text(label)), Some(Value::Bool(paused))) = (get(5), get(7)) else {
        return Err(bad);
    };
    let (Some(cid), Some(at)) = (uint(get(3)), uint(get(6))) else {
        return Err(bad);
    };
    let kid: Kid = kid
        .clone()
        .try_into()
        .map_err(|_| TrustError::Malformed("kid length"))?;
    if kid_of(pk) != kid || fields.len() != 7 {
        return Err(TrustError::Malformed("an entry's kid is not its key's"));
    }
    Ok(TrustedController {
        kid,
        sign_pk: pk.clone(),
        cid,
        owner: IdFp::parse(owner).map_err(|_| TrustError::Malformed("owner id_fp"))?,
        label: label.clone(),
        trusted_at_ms: at,
        paused: *paused,
    })
}

/// The gate's key directory on this Device: trusted Controllers only, never an identity key.
impl KeyDirectory for TrustList {
    fn key(&self, kid: &Kid) -> Option<KeyEntry> {
        self.get(kid).map(|e| KeyEntry {
            public_key: e.sign_pk.clone(),
            owner: e.owner.clone(),
            role: KeyRole::Endpoint {
                cid: e.cid,
                kind: EndpointKind::Controller,
            },
        })
    }
}
