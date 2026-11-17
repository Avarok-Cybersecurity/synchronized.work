//! Per-Controller sliding windows (D4, design §9.2): accepted signals per kid per minute, and
//! refusals per kid per minute, the latter pausing a kid that keeps sending what fails.

use std::collections::{BTreeMap, VecDeque};

use sw_crypto::Kid;

/// The window both limits are counted over: "per minute" in the config names.
pub const WINDOW_MS: u64 = 60_000;

#[derive(Debug, Clone, Default)]
pub struct RateWindows {
    accepted: BTreeMap<Kid, VecDeque<u64>>,
    refused: BTreeMap<Kid, VecDeque<u64>>,
}

fn prune(times: &mut VecDeque<u64>, now_ms: u64) {
    while times
        .front()
        .is_some_and(|t| now_ms.saturating_sub(*t) >= WINDOW_MS)
    {
        times.pop_front();
    }
}

impl RateWindows {
    /// What a fresh daemon starts from.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Counts one more accepted signal for `kid` unless `max` are already in the window.
    pub fn admit(&mut self, kid: &Kid, now_ms: u64, max: u32) -> bool {
        let times = self.accepted.entry(*kid).or_default();
        prune(times, now_ms);
        if times.len() >= max as usize {
            return false;
        }
        times.push_back(now_ms);
        true
    }

    /// Counts a refusal; `true` once `max` refusals fall inside one window (pause the kid).
    pub fn refuse(&mut self, kid: &Kid, now_ms: u64, max: u32) -> bool {
        let times = self.refused.entry(*kid).or_default();
        prune(times, now_ms);
        times.push_back(now_ms);
        times.len() >= max as usize
    }

    /// `sw-mcp trust` re-confirmed this kid: its refusal history starts over.
    pub fn forgive(&mut self, kid: &Kid) {
        self.refused.remove(kid);
    }
}
