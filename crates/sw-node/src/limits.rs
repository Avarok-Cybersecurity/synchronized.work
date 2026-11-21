//! The node's hard caps (design §2.6, §4.5), stated by the host: each is a refusal, never a
//! silent drop, and none has a value in code.

use crate::provision::{ProvisionError, Source, number};

pub const VAR_INBOX_MAX_SIGNALS: &str = "SW_INBOX_MAX_SIGNALS";
pub const VAR_INBOX_MAX_BYTES: &str = "SW_INBOX_MAX_BYTES";
pub const VAR_MAX_PERSONS: &str = "SW_MAX_PERSONS";
pub const VAR_MAX_TASKS: &str = "SW_MAX_TASKS";
pub const VAR_PAD_RATE_PER_MIN: &str = "SW_PAD_RATE_PER_MIN";
pub const VAR_PAD_PENDING_MAX: &str = "SW_PAD_PENDING_MAX";

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
        })
    }
}
