//! The node's hard caps (design §2.6, §4.5), stated by the host: each is a refusal, never a
//! silent drop, and none has a value in code.

use crate::provision::{ProvisionError, Source, number};

pub const VAR_INBOX_MAX_SIGNALS: &str = "SW_INBOX_MAX_SIGNALS";
pub const VAR_INBOX_MAX_BYTES: &str = "SW_INBOX_MAX_BYTES";
pub const VAR_MAX_PERSONS: &str = "SW_MAX_PERSONS";
pub const VAR_MAX_TASKS: &str = "SW_MAX_TASKS";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Signals queued for one offline Device.
    pub inbox_max_signals: usize,
    /// Bytes queued for one offline Device.
    pub inbox_max_bytes: usize,
    pub max_persons: usize,
    pub max_tasks: usize,
}

impl Limits {
    pub fn from_source(source: &dyn Source) -> Result<Self, ProvisionError> {
        Ok(Self {
            inbox_max_signals: number(source, VAR_INBOX_MAX_SIGNALS)?,
            inbox_max_bytes: number(source, VAR_INBOX_MAX_BYTES)?,
            max_persons: number(source, VAR_MAX_PERSONS)?,
            max_tasks: number(source, VAR_MAX_TASKS)?,
        })
    }
}
