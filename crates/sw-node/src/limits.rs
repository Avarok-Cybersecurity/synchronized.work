//! The node's hard caps (design §2.6, §4.5), stated by the host: each is a refusal, never a
//! silent drop, and none has a value in code.

use std::num::NonZeroUsize;

use citadel_sdk::prelude::{LegacyRegistration, ServerLimits};

use crate::provision::{ProvisionError, Source, number, required};

pub const VAR_INBOX_MAX_SIGNALS: &str = "SW_INBOX_MAX_SIGNALS";
pub const VAR_INBOX_MAX_BYTES: &str = "SW_INBOX_MAX_BYTES";
pub const VAR_MAX_PERSONS: &str = "SW_MAX_PERSONS";
pub const VAR_MAX_TASKS: &str = "SW_MAX_TASKS";
pub const VAR_PAD_RATE_PER_MIN: &str = "SW_PAD_RATE_PER_MIN";
pub const VAR_PAD_PENDING_MAX: &str = "SW_PAD_PENDING_MAX";
pub const VAR_SUBSCRIPTIONS_PER_SESSION: &str = "SW_SUBSCRIPTIONS_PER_SESSION";
pub const VAR_SUBSCRIBERS_PER_PROJECT: &str = "SW_SUBSCRIBERS_PER_PROJECT";
pub const VAR_LINK_REDEEM_PER_MIN: &str = "SW_LINK_REDEEM_PER_MIN";
pub const VAR_LINK_REDEEM_NODE_PER_MIN: &str = "SW_LINK_REDEEM_NODE_PER_MIN";
pub const VAR_IDENTITY_CHALLENGE_INTERVAL_MS: &str = "SW_IDENTITY_CHALLENGE_INTERVAL_MS";

/// `server-identity-challenge.md` §2: a session is answered at most once per second, so a
/// shorter interval is malformed.
pub const MIN_IDENTITY_CHALLENGE_INTERVAL_MS: u64 = 1000;

/// The SDK's server limits (`ServerLimits`): connections held before sign-in, per address and
/// in all; legacy registrations hashed at once, or `refused`; bytes queued per stream and in all.
pub const VAR_PROVISIONAL_PER_IP: &str = "SW_PROVISIONAL_PER_IP";
pub const VAR_PROVISIONAL_TOTAL: &str = "SW_PROVISIONAL_TOTAL";
pub const VAR_LEGACY_REGISTRATION_HASHES: &str = "SW_LEGACY_REGISTRATION_HASHES";
pub const VAR_OUTBOUND_QUEUED_PER_STREAM: &str = "SW_OUTBOUND_QUEUED_PER_STREAM";
pub const VAR_OUTBOUND_QUEUED_TOTAL: &str = "SW_OUTBOUND_QUEUED_TOTAL";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Signals queued for one offline Device.
    pub inbox_max_signals: usize,
    /// Bytes queued for one offline Device.
    pub inbox_max_bytes: usize,
    pub max_persons: usize,
    pub max_tasks: usize,
    /// Scratchpad notes and acks one Person may send per minute.
    pub pad_rate_per_min: usize,
    /// Notes pending (un-acked) for one target Person in one project.
    pub pad_pending_max: usize,
    /// Projects one session may be subscribed to at once.
    pub subscriptions_per_session: usize,
    /// Sessions subscribed to one project at once.
    pub subscribers_per_project: usize,
    /// `LINK_REDEEM` attempts one account may make per minute, and the whole node.
    pub link_redeem_per_min: usize,
    pub link_redeem_node_per_min: usize,
    /// The least time between two answered server identity challenges of one session.
    pub identity_challenge_interval_ms: u64,
}

impl Limits {
    pub fn from_source(source: &dyn Source) -> Result<Self, ProvisionError> {
        Ok(Self {
            inbox_max_signals: number(source, VAR_INBOX_MAX_SIGNALS)?,
            inbox_max_bytes: number(source, VAR_INBOX_MAX_BYTES)?,
            max_persons: number(source, VAR_MAX_PERSONS)?,
            max_tasks: number(source, VAR_MAX_TASKS)?,
            pad_rate_per_min: number(source, VAR_PAD_RATE_PER_MIN)?,
            pad_pending_max: number(source, VAR_PAD_PENDING_MAX)?,
            subscriptions_per_session: number(source, VAR_SUBSCRIPTIONS_PER_SESSION)?,
            subscribers_per_project: number(source, VAR_SUBSCRIBERS_PER_PROJECT)?,
            link_redeem_per_min: number(source, VAR_LINK_REDEEM_PER_MIN)?,
            link_redeem_node_per_min: number(source, VAR_LINK_REDEEM_NODE_PER_MIN)?,
            identity_challenge_interval_ms: challenge_interval(source)?,
        })
    }
}

fn challenge_interval(source: &dyn Source) -> Result<u64, ProvisionError> {
    let ms: u64 = number(source, VAR_IDENTITY_CHALLENGE_INTERVAL_MS)?;
    if ms < MIN_IDENTITY_CHALLENGE_INTERVAL_MS {
        return Err(ProvisionError::Malformed {
            name: VAR_IDENTITY_CHALLENGE_INTERVAL_MS,
            why: format!("{ms} ms answers more than one challenge a second"),
        });
    }
    Ok(ms)
}

/// The SDK's limits for this server; each value is a positive count, zero is malformed.
pub fn server_limits(source: &dyn Source) -> Result<ServerLimits, ProvisionError> {
    let legacy_registration = match required(source, VAR_LEGACY_REGISTRATION_HASHES)?.as_str() {
        "refused" => LegacyRegistration::Refused,
        _ => LegacyRegistration::Accepted {
            concurrent_hashes: number::<NonZeroUsize>(source, VAR_LEGACY_REGISTRATION_HASHES)?,
        },
    };
    Ok(ServerLimits {
        provisional_per_ip: number(source, VAR_PROVISIONAL_PER_IP)?,
        provisional_total: number(source, VAR_PROVISIONAL_TOTAL)?,
        legacy_registration,
        outbound_queued_per_stream: number(source, VAR_OUTBOUND_QUEUED_PER_STREAM)?,
        outbound_queued_total: number(source, VAR_OUTBOUND_QUEUED_TOTAL)?,
    })
}
