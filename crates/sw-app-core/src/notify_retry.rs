//! Host notifications until the host takes them. A `NOTIFY` maps to one [`HostNotify`] once per
//! `event_id` ([`crate::notify::NotifyMap`]); this queue holds it until the host answers. A
//! refusal (the host's call limit, most often) puts it back, due again after a spacing of frame
//! time; after a bounded number of attempts it is given up. Only a refusal re-sends, so one
//! event is delivered at most once.

use std::collections::VecDeque;

use crate::notify::HostNotify;

/// Explicit (PCND): how notifications are retried.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotifyRetry {
    /// Notifications held at once (waiting or awaiting the host's answer).
    pub queue: usize,
    /// Deliveries tried per notification before it is given up.
    pub attempts: u32,
    /// Frame time from a refusal to the next attempt.
    pub spacing_ms: f32,
}

#[derive(Debug, Clone)]
struct Held {
    note: HostNotify,
    tries: u32,
    due_ms: f64,
    in_flight: bool,
}

#[derive(Debug, Clone)]
pub struct NotifyQueue {
    retry: NotifyRetry,
    held: VecDeque<Held>,
}

/// A notification that left the queue without being delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GaveUp {
    /// The queue was full when it arrived.
    Full,
    /// Refused on every one of this many attempts.
    Refused { attempts: u32 },
}

impl NotifyQueue {
    pub fn new(retry: NotifyRetry) -> Self {
        Self {
            retry,
            held: VecDeque::new(),
        }
    }

    /// Holds `note`, due at once.
    pub fn push(&mut self, note: HostNotify, now_ms: f64) -> Result<(), GaveUp> {
        if self.held.len() >= self.retry.queue {
            return Err(GaveUp::Full);
        }
        self.held.push_back(Held {
            note,
            tries: 0,
            due_ms: now_ms,
            in_flight: false,
        });
        Ok(())
    }

    /// The first notification due now, marked as handed to the host.
    pub fn due(&mut self, now_ms: f64) -> Option<HostNotify> {
        let held = self
            .held
            .iter_mut()
            .find(|h| !h.in_flight && h.due_ms <= now_ms)?;
        held.in_flight = true;
        held.tries += 1;
        Some(held.note.clone())
    }

    /// The host showed it.
    pub fn delivered(&mut self, event_id: &[u8; 32]) {
        self.held
            .retain(|h| !(h.in_flight && h.note.event_id == *event_id));
    }

    /// The host refused it: due again after the spacing, or given up after the last attempt.
    pub fn refused(&mut self, event_id: &[u8; 32], now_ms: f64) -> Option<GaveUp> {
        let i = self
            .held
            .iter()
            .position(|h| h.in_flight && h.note.event_id == *event_id)?;
        let held = &mut self.held[i];
        if held.tries >= self.retry.attempts {
            let attempts = held.tries;
            self.held.remove(i);
            return Some(GaveUp::Refused { attempts });
        }
        held.in_flight = false;
        held.due_ms = now_ms + f64::from(self.retry.spacing_ms);
        None
    }

    /// Whether a notification waits for frame time (one awaiting the host's answer does not).
    pub fn waiting(&self) -> bool {
        self.held.iter().any(|h| !h.in_flight)
    }
}
