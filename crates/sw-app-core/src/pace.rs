//! The App's own host-call budget. The host rate-limits every call an App makes together
//! (canvas draws and measures, notifications, log, storage, session) and refuses calls over its
//! limit; the App keeps itself under a budget well below that limit so the host never has to.
//!
//! Time is frame time: the sum of `Frame { dt_ms }` the App was given (the core has no clock).
//! Between frames the clock stands still, which only over-counts: a call is never emitted
//! sooner than the budget allows in real time.
//!
//! Three tiers share one window. Draws and text measures are at most `per_s - reserve` of it,
//! so sends, notifications and the like always find room; asking for frames has the last
//! slot, so the App can always ask for the frames that move its clock forward again.
//!
//! Within the draws' share, room is kept for draws at least every `WINDOW_MS / draw_floor`
//! ([`Ledger::draw`]). A draw that is due (that long since the last) needs one free slot. Any
//! other low call — a text measure, a draw sooner than that — is made only if it leaves room
//! for every due draw of the next window, counting the calls that will by then have left it
//! ([`Ledger::spare`]); since nothing else spends low calls, a due draw always finds its slot.
//! So while the screen changes the host gets a draw at least every `WINDOW_MS / draw_floor`
//! plus one frame, whatever the text costs (unless sends and notifications take more than
//! `reserve - 1` slots of the window). Frames are paced to the share's sustained rate, one
//! draw per `WINDOW_MS / (per_s - reserve)`, so a fast display spreads its draws over the
//! window rather than spending the share in a burst; a draw an input causes is not held
//! back, but uses the pace up.

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
    /// Of the draws' share, the slots kept for draws alone: one is due every
    /// `WINDOW_MS / draw_floor` ms. Below `per_s - reserve`.
    pub draw_floor: u32,
}

/// Which share of the budget a host call may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// `RequestFrames { on: true }`: the whole budget.
    Wake,
    /// Sends, notifications, storage, the session, faults: all but the last slot.
    Essential,
    /// Draws, text measures, `RequestFrames { on: false }`: at most `per_s - reserve` of
    /// these, and never the last slot.
    Low,
}

/// The host calls made in the current window, by frame time: all of them, and the low ones.
#[derive(Debug, Clone)]
pub struct Ledger {
    budget: HostCallBudget,
    now_ms: f64,
    calls: VecDeque<f64>,
    low: VecDeque<f64>,
    last_draw_ms: Option<f64>,
    /// Frame time not yet used by a draw, at most two draw intervals.
    pace_ms: f64,
}

fn count(calls: &VecDeque<f64>) -> i64 {
    i64::try_from(calls.len()).unwrap_or(i64::MAX)
}

impl Ledger {
    pub fn new(budget: HostCallBudget) -> Self {
        Self {
            budget,
            now_ms: 0.0,
            calls: VecDeque::new(),
            low: VecDeque::new(),
            last_draw_ms: None,
            pace_ms: 0.0,
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
            self.pace_ms = (self.pace_ms + f64::from(dt_ms)).min(2.0 * self.draw_interval_ms());
        }
    }

    /// The draws' and measures' share of the window.
    fn low_share(&self) -> u32 {
        self.budget.per_s.saturating_sub(self.budget.reserve)
    }

    fn low_cap(&self) -> i64 {
        i64::from(self.low_share())
    }

    /// The draws' share spread evenly over the window.
    fn draw_interval_ms(&self) -> f64 {
        WINDOW_MS / f64::from(self.low_share().max(1))
    }

    fn floor_ms(&self) -> f64 {
        WINDOW_MS / f64::from(self.budget.draw_floor.max(1))
    }

    fn prune(&mut self) {
        let since = self.now_ms - WINDOW_MS;
        for calls in [&mut self.calls, &mut self.low] {
            while calls.front().is_some_and(|at| *at <= since) {
                calls.pop_front();
            }
        }
    }

    /// How many more calls of `tier` the current window takes.
    pub fn room(&mut self, tier: Tier) -> u32 {
        self.prune();
        let per_s = i64::from(self.budget.per_s);
        let all = count(&self.calls);
        let room = match tier {
            Tier::Wake => per_s - all,
            Tier::Essential => per_s - 1 - all,
            Tier::Low => (self.low_cap() - count(&self.low)).min(per_s - 1 - all),
        };
        u32::try_from(room.max(0)).unwrap_or(0)
    }

    /// How many low calls may be made now and still leave a slot for each due draw of the next
    /// window: the `k`-th comes no sooner than `k × floor` from now, when the low calls older
    /// than that less a window have left.
    pub fn spare(&mut self) -> u32 {
        let mut spare = i64::from(self.room(Tier::Low)) - 1;
        for k in 1..self.budget.draw_floor {
            let leaves = self.now_ms + f64::from(k) * self.floor_ms() - WINDOW_MS;
            let gone = self.low.partition_point(|at| *at <= leaves);
            let left = count(&self.low) - i64::try_from(gone).unwrap_or(i64::MAX);
            spare = spare.min(self.low_cap() - 1 - i64::from(k) - left);
        }
        u32::try_from(spare.max(0)).unwrap_or(0)
    }

    /// Whether a draw may go out now, and if so how many text measures it may make. `frame`:
    /// the draw is a frame's, which waits for the pace; an input's goes now if it can.
    pub fn draw(&mut self, frame: bool) -> Option<u32> {
        let floor_ms = self.floor_ms();
        let due = self
            .last_draw_ms
            .is_none_or(|at| self.now_ms - at >= floor_ms);
        let paced = !frame || self.pace_ms >= self.draw_interval_ms();
        let spare = self.spare();
        let measures = spare.saturating_sub(1);
        ((spare > 0 && paced) || (due && self.room(Tier::Low) > 0)).then_some(measures)
    }

    /// Records a draw made now (its call is spent separately).
    pub fn drew(&mut self) {
        self.last_draw_ms = Some(self.now_ms);
        self.pace_ms = (self.pace_ms - self.draw_interval_ms()).max(0.0);
    }

    /// Records `n` calls of `tier` made now.
    pub fn spend(&mut self, tier: Tier, n: u32) {
        for _ in 0..n {
            self.calls.push_back(self.now_ms);
            if tier == Tier::Low {
                self.low.push_back(self.now_ms);
            }
        }
    }
}
