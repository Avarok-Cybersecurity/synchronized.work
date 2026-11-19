//! What the node must be told before it starts (PCND): every key, cost and limit is named by
//! the host, none has a default, and the node refuses to start while any is missing or
//! malformed. The host is the Worker (`[vars]` in wrangler.toml, secrets by
//! `wrangler secret put`); the names below are the SSOT for both.

use citadel_identity::{CanonicalDomain, MlDsa65Pk, canonical_domain};

use sw_model::{Approval, PersonPolicy};

use crate::limits::Limits;

/// Secrets: never in wrangler.toml, never logged.
pub const SECRET_SERVER_KEY_SEED: &str = "SW_SERVER_KEY_SEED";
pub const SECRET_OPRF_SEED: &str = "SW_OPRF_SEED";

/// `[vars]`.
pub const VAR_SERVER_KEY_ISSUED_AT: &str = "SW_SERVER_KEY_ISSUED_AT";
pub const VAR_FORMER_DOMAINS: &str = "SW_FORMER_DOMAINS";
pub const VAR_CITADEL_WSS: &str = "SW_CITADEL_WSS";
pub const VAR_SIGN_IN_ISSUER_PK: &str = "SW_SIGN_IN_ISSUER_PK";
pub const VAR_ARGON_LANES: &str = "ARGON_LANES";
pub const VAR_ARGON_MEM_KIB: &str = "ARGON_MEM_KIB";
pub const VAR_ARGON_TIME_COST: &str = "ARGON_TIME_COST";
pub const VAR_KSF_MEM_KIB: &str = "PQ_KSF_MEM_KIB";
pub const VAR_KSF_ITERATIONS: &str = "PQ_KSF_ITERATIONS";
pub const VAR_KSF_LANES: &str = "PQ_KSF_LANES";
/// At least three distinct `host:port` STUN servers (the SDK's own minimum), comma-separated.
pub const VAR_STUN_SERVERS: &str = "STUN_SERVERS";
/// The SDK's `PathPolicy` as JSON (its serde form): which paths peer connections may take.
pub const VAR_PATH_POLICY: &str = "SW_PATH_POLICY";
/// `manual` or `auto`, and a rate: the record a Person the node first meets is given (D4).
pub const VAR_PERSON_APPROVAL: &str = "SW_PERSON_APPROVAL";
pub const VAR_PERSON_RATE_PER_MIN: &str = "SW_PERSON_RATE_PER_MIN";

/// The node's canonical domain: its `[identity]`, the audience every node-addressed `Body`
/// carries (sw-wire §2), and the domain of every account on it.
pub const IDENTITY_DOMAIN: &str = sw_protocol::body::NODE_AUDIENCE;

/// Where the host's values come from.
pub trait Source {
    fn get(&self, name: &str) -> Option<String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisionError {
    Missing(&'static str),
    Malformed { name: &'static str, why: String },
}

impl std::fmt::Display for ProvisionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(name) => write!(f, "{name} is not provisioned"),
            Self::Malformed { name, why } => write!(f, "{name} is malformed: {why}"),
        }
    }
}

/// Server-side Argon2 for legacy (non-PQ) accounts, sized by the host for its isolate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgonCost {
    pub lanes: u32,
    pub mem_kib: u32,
    pub time_cost: u32,
}

/// The Argon2id cost clients stretch a PQ password factor with (the server never runs it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KsfCost {
    pub mem_kib: u32,
    pub iterations: u32,
    pub lanes: u32,
}

pub struct Provisioning {
    pub identity: CanonicalDomain,
    pub server_key_seed: zeroize::Zeroizing<[u8; 32]>,
    pub server_key_issued_at_s: u64,
    pub former_domains: Vec<CanonicalDomain>,
    pub citadel_wss: String,
    /// work.avarok.net's pinned server key: the only issuer of `ENROL` assertions.
    pub sign_in_issuer: MlDsa65Pk,
    pub oprf_seed: zeroize::Zeroizing<[u8; 32]>,
    pub argon: ArgonCost,
    pub ksf: KsfCost,
    pub stun_servers: Vec<String>,
    pub path_policy: citadel_sdk::prelude::PathPolicy,
    pub new_person_policy: PersonPolicy,
    pub limits: Limits,
}

impl std::fmt::Debug for Provisioning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Provisioning")
            .field("identity", &self.identity)
            .field("server_key_issued_at_s", &self.server_key_issued_at_s)
            .field("citadel_wss", &self.citadel_wss)
            .field("sign_in_issuer", &self.sign_in_issuer)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl Provisioning {
    pub fn from_source(source: &dyn Source) -> Result<Self, ProvisionError> {
        let identity =
            canonical_domain(IDENTITY_DOMAIN).map_err(|e| ProvisionError::Malformed {
                name: "IDENTITY_DOMAIN",
                why: format!("{e:?}"),
            })?;
        let issuer = required(source, VAR_SIGN_IN_ISSUER_PK)?;
        Ok(Self {
            identity,
            server_key_seed: zeroize::Zeroizing::new(seed(source, SECRET_SERVER_KEY_SEED)?),
            server_key_issued_at_s: number(source, VAR_SERVER_KEY_ISSUED_AT)?,
            former_domains: domains(source, VAR_FORMER_DOMAINS)?,
            citadel_wss: required(source, VAR_CITADEL_WSS)?,
            sign_in_issuer: MlDsa65Pk::from_base64url(&issuer).map_err(|e| {
                ProvisionError::Malformed {
                    name: VAR_SIGN_IN_ISSUER_PK,
                    why: e.to_string(),
                }
            })?,
            oprf_seed: zeroize::Zeroizing::new(seed(source, SECRET_OPRF_SEED)?),
            argon: ArgonCost {
                lanes: number(source, VAR_ARGON_LANES)?,
                mem_kib: number(source, VAR_ARGON_MEM_KIB)?,
                time_cost: number(source, VAR_ARGON_TIME_COST)?,
            },
            ksf: KsfCost {
                mem_kib: number(source, VAR_KSF_MEM_KIB)?,
                iterations: number(source, VAR_KSF_ITERATIONS)?,
                lanes: number(source, VAR_KSF_LANES)?,
            },
            stun_servers: required(source, VAR_STUN_SERVERS)?
                .split(',')
                .map(|s| s.trim().to_owned())
                .collect(),
            path_policy: serde_json::from_str(&required(source, VAR_PATH_POLICY)?).map_err(
                |e| ProvisionError::Malformed {
                    name: VAR_PATH_POLICY,
                    why: e.to_string(),
                },
            )?,
            new_person_policy: PersonPolicy {
                approval: match required(source, VAR_PERSON_APPROVAL)?.as_str() {
                    "manual" => Approval::Manual,
                    "auto" => Approval::Auto,
                    other => {
                        return Err(ProvisionError::Malformed {
                            name: VAR_PERSON_APPROVAL,
                            why: format!("{other} is neither manual nor auto"),
                        });
                    }
                },
                rate_per_min: number(source, VAR_PERSON_RATE_PER_MIN)?,
            },
            limits: Limits::from_source(source)?,
        })
    }
}

pub(crate) fn required(source: &dyn Source, name: &'static str) -> Result<String, ProvisionError> {
    match source.get(name) {
        Some(value) if !value.trim().is_empty() => Ok(value.trim().to_owned()),
        _ => Err(ProvisionError::Missing(name)),
    }
}

pub(crate) fn number<T: std::str::FromStr>(
    source: &dyn Source,
    name: &'static str,
) -> Result<T, ProvisionError>
where
    T::Err: std::fmt::Display,
{
    required(source, name)?
        .parse()
        .map_err(|e: T::Err| ProvisionError::Malformed {
            name,
            why: e.to_string(),
        })
}

/// 32 bytes as 64 hex characters.
fn seed(source: &dyn Source, name: &'static str) -> Result<[u8; 32], ProvisionError> {
    let text = zeroize::Zeroizing::new(required(source, name)?);
    let malformed = |why: &str| ProvisionError::Malformed {
        name,
        why: why.to_owned(),
    };
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(malformed("not 64 hex characters"));
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte =
            u8::from_str_radix(&text[2 * i..2 * i + 2], 16).map_err(|_| malformed("not hex"))?;
    }
    Ok(out)
}

/// A comma-separated list; the variable must be present, and `-` states "none".
fn domains(
    source: &dyn Source,
    name: &'static str,
) -> Result<Vec<CanonicalDomain>, ProvisionError> {
    let text = required(source, name)?;
    if text == "-" {
        return Ok(Vec::new());
    }
    text.split(',')
        .map(|d| {
            canonical_domain(d.trim()).map_err(|e| ProvisionError::Malformed {
                name,
                why: format!("{d}: {e:?}"),
            })
        })
        .collect()
}
