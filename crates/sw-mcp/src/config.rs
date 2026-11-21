//! `~/.synchronized/config.toml` (design §9.4). Every field is required and none has a default
//! (PCND): a missing or unknown field stops every subcommand with exit status 2 and the
//! parser's message, which names the field.

use citadel_sdk::prelude::CertVerification;
use serde::Deserialize;

use citadel_identity::{CanonicalDomain, canonical_domain, make_username};
use sw_model::IdFp;

/// The exit status for a configuration that cannot be used.
pub const CONFIG_EXIT: u8 = 2;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The synchronized.work node: `wss://host/path`, or `ws://` to a node on this machine's
    /// loopback (a local `wrangler dev` of sw-node). Its host is the node's canonical domain.
    pub server: String,
    /// This Device's slug: `[a-z0-9-]`, the `<slug>` of `<id_fp>.<slug>@<node>`.
    pub device_slug: String,
    /// The "Sign in with Citadel" issuer's canonical domain (`work.avarok.net`).
    pub sign_in_issuer: String,
    /// A newly trusted Controller's prompts wait for a local approval for this long (§6.3).
    pub approval_mode_new_controller_hours: u64,
    /// How long an agent has to `ack_signal` before the Controller sees `executed: unknown`.
    pub ack_timeout_s: u64,
    /// Seconds between attempts to reach the node after the session drops.
    pub reconnect_delay_s: u64,
    /// At least three distinct `host:port` STUN servers: the SDK requires them of every node
    /// (NAT traversal for the P2P path, design §4.8).
    pub stun_servers: Vec<String>,
    pub log_level: LogLevel,
    /// How a verified prompt reaches the agent (§9.3).
    pub harness: Harness,
    /// Where the Device keys, the Citadel password and the sign-in assertion are kept.
    pub keystore: KeyStoreKind,
    pub rate: RateConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateConfig {
    /// Signals accepted per Controller key per minute (D4).
    pub per_kid_per_min: u32,
    /// Verified signals waiting for an agent or an approval, across every Controller.
    pub pending_max: u32,
    /// Refusals per Controller key within a minute that pause it until `sw-mcp trust`.
    pub refusals_per_min_before_pause: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Harness {
    /// The Claude Code `Stop` (and, for `interrupt`, `PreToolUse`) hook (§9.3 adapter 1).
    StopHook,
    /// The agent calls `await_signal` when idle (§9.3 adapter 2).
    ToolPoll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyStoreKind {
    /// Files under `keys/`, protected by owner-only permissions only: mode 0600 in a 0700
    /// directory on Unix, an owner-only, non-inherited ACL on Windows (`crate::home`).
    /// The OS-keychain wrapping of design §5.4 is not built yet; this kind says so at startup.
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// TOML syntax, a missing field or an unknown one (the message names it).
    Parse(String),
    Invalid {
        field: &'static str,
        why: String,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(why) => write!(f, "config.toml: {why}"),
            Self::Invalid { field, why } => write!(f, "config.toml: `{field}`: {why}"),
        }
    }
}

impl std::error::Error for ConfigError {}

fn invalid(field: &'static str, why: impl Into<String>) -> ConfigError {
    ConfigError::Invalid {
        field,
        why: why.into(),
    }
}

impl Config {
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(text).map_err(|e| ConfigError::Parse(e.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        self.node_domain()?;
        let slug_ok = (1..=64).contains(&self.device_slug.len())
            && self
                .device_slug
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !self.device_slug.starts_with('-')
            && !self.device_slug.ends_with('-');
        if !slug_ok {
            return Err(invalid(
                "device_slug",
                "must be [a-z0-9-], 1..=64, no edge dash",
            ));
        }
        let mut stun = self.stun_servers.clone();
        stun.sort();
        stun.dedup();
        if stun.len() < 3 || stun.len() != self.stun_servers.len() {
            return Err(invalid(
                "stun_servers",
                "needs at least three distinct host:port entries",
            ));
        }
        canonical_domain(&self.sign_in_issuer)
            .map_err(|e| invalid("sign_in_issuer", e.to_string()))?;
        let positive: [(&'static str, u64); 6] = [
            (
                "approval_mode_new_controller_hours",
                self.approval_mode_new_controller_hours,
            ),
            ("ack_timeout_s", self.ack_timeout_s),
            ("reconnect_delay_s", self.reconnect_delay_s),
            ("rate.per_kid_per_min", u64::from(self.rate.per_kid_per_min)),
            ("rate.pending_max", u64::from(self.rate.pending_max)),
            (
                "rate.refusals_per_min_before_pause",
                u64::from(self.rate.refusals_per_min_before_pause),
            ),
        ];
        match positive.iter().find(|(_, v)| *v == 0) {
            Some((field, _)) => Err(invalid(field, "must be at least 1")),
            None => Ok(()),
        }
    }

    /// The node's canonical domain: the host (and port) of `server`. Only `wss://` leaves the
    /// machine; plain `ws://` is accepted for a loopback host alone.
    pub fn node_domain(&self) -> Result<CanonicalDomain, ConfigError> {
        let loopback = |rest: &&str| {
            let host = rest.split('/').next().unwrap_or_default();
            let host = host.rsplit_once(':').map_or(host, |(h, _)| h);
            matches!(host, "127.0.0.1" | "localhost")
        };
        let rest = match self.server.strip_prefix("wss://") {
            Some(rest) => rest,
            None => self
                .server
                .strip_prefix("ws://")
                .filter(loopback)
                .ok_or_else(|| {
                    invalid(
                        "server",
                        "must be wss://, or ws:// to 127.0.0.1 or localhost",
                    )
                })?,
        };
        let host = rest.split('/').next().unwrap_or_default();
        canonical_domain(host).map_err(|e| invalid("server", e.to_string()))
    }

    /// What the session demands of the node's certificate, by `server`'s scheme: a `wss://`
    /// node must present one the OS roots verify; a loopback `ws://` node has no TLS to verify.
    pub fn cert_verification(&self) -> Result<CertVerification, ConfigError> {
        self.node_domain()?;
        Ok(if self.server.starts_with("wss://") {
            CertVerification::Required
        } else {
            CertVerification::SelfSignedAccepted
        })
    }

    /// The Device's Citadel username on the node: `<id_fp>.<slug>@<node>` (contract §6).
    pub fn device_username(&self, id_fp: &IdFp) -> Result<String, ConfigError> {
        let handle = format!("{}.{}", id_fp.as_str(), self.device_slug);
        make_username(&handle, &self.node_domain()?)
            .map_err(|e| invalid("device_slug", format!("username {handle:?}: {e}")))
    }
}
