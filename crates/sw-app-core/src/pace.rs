//! The App's own host-call budget. The host rate-limits every call an App makes together
//! (canvas draws and measures, notifications, log, storage, session) and refuses calls over its
//! limit; the App keeps itself under a budget well below that limit so the host never has to.
//!
//! Time is frame time: the sum of `Frame { dt_ms }` the App was given (the core has no clock).
//! Between frames the clock stands still, which only over-counts: a call is never emitted
//! sooner than the budget allows in real time.
//!
//! Three tiers share one window. Draws and text measures stop first (`per_s - reserve`), so
//! sends, notifications and the like always find room; asking for frames has the last slot,
//! so the App can always ask for the frames that move its clock forward again.

use std::collections::VecDeque;

/// The window the budget is counted in (design: "per second").
pub const WINDOW_MS: f64 = 1000.0;

/// Explicit (PCND): the App's own ceiling on host calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostCallBudget {
    /// Host calls (effects and text measures) in any [`WINDOW_MS`] of frame time.
    pub per_s: u32,
    /// Of `per_s`, the calls draws and measures leave for everything else. At least 2.
    pub reserve: u32,
}

/// Which share of the budget a host call may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// `RequestFrames { on: true }`: the whole budget.
    Wake,
    /// Sends, notifications, storage, the session, faults: all but the last slot.
    Essential,
    /// Draws, text measures, `RequestFrames { on: false }`: all but the reserve.
    Low,
}

/// The host calls made in the current window, by frame time.
#[derive(Debug, Clone)]
pub struct Ledger {
    budget: HostCallBudget,
    now_ms: f64,
    calls: VecDeque<f64>,
}

impl Ledger {
    pub fn new(budget: HostCallBudget) -> Self {
        Self {
            budget,
            now_ms: 0.0,
            calls: VecDeque::new(),
        }
    }

    /// Frame time so far.
    pub fn now_ms(&self) -> f64 {
        self.now_ms
    }

    /// One frame's time; a negative or non-finite interval moves nothing.
    pub fn advance(&mut self, dt_ms: f32) {
        if dt_ms.is_finite() && dt_ms > 0.0 {
            self.now_ms += f64::from(dt_ms);
        }
    }

    fn cap(&self, tier: Tier) -> u32 {
        let per_s = self.budget.per_s;
        match tier {
            Tier::Wake => per_s,
            Tier::Essential => per_s.saturating_sub(1),
            Tier::Low => per_s.saturating_sub(self.budget.reserve),
        }
    }

    /// How many more calls of `tier` the current window takes.
    pub fn room(&mut self, tier: Tier) -> u32 {
        let since = self.now_ms - WINDOW_MS;
        while self.calls.front().is_some_and(|at| *at <= since) {
            self.calls.pop_front();
        }
        let used = u32::try_from(self.calls.len()).unwrap_or(u32::MAX);
        self.cap(tier).saturating_sub(used)
    }

    /// Records `n` calls made now.
    pub fn spend(&mut self, n: u32) {
        for _ in 0..n {
            self.calls.push_back(self.now_ms);
        }
    }
}
