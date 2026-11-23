#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The App's own host-call budget: within any 1000 ms of frame time it makes at most `per_s`
//! host calls (effects and measures); sends and notifications go before draws; a draw that
//! has to wait is of the latest state when it goes.

mod support;

use support::*;
use sw_app_core::pace::HostCallBudget;
use sw_app_core::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// Small enough that a tree animating at 60 frames a second meets it.
const BUDGET: HostCallBudget = HostCallBudget {
    per_s: 48,
    reserve: 8,
};

struct Run {
    app: App<Counted>,
    m: Counted,
    /// Each effect with the frame time it was emitted at.
    calls: Vec<(f64, Effect)>,
}

impl Run {
    fn new() -> Self {
        let m = Counted::default();
        let mut c = config();
        c.host_calls = BUDGET;
        let mut app = App::new(ctx(granted(true, true), CanvasAbi::Cw5, FULL), c, m.clone());
        let mut calls = Vec::new();
        let mut out = app.start();
        out.extend(app.handle(Input::LinkLoaded {
            record: Some(linked_to("p1")),
        }));
        out.extend(app.handle(Input::SessionOpened {
            cid: 0x1234,
            status: HostSessionStatus::Connected,
        }));
        calls.extend(out.into_iter().map(|e| (0.0, e)));
        Self { app, m, calls }
    }

    fn feed(&mut self, input: Input) -> Vec<Effect> {
        if let Input::Frame { dt_ms } = input {
            self.m.now.set(self.m.now.get() + f64::from(dt_ms));
        }
        let mut out = self.app.handle(input);
        // The host shows every notification it is given.
        let shown: Vec<[u8; 32]> = out
            .iter()
            .filter_map(|e| match e {
                Effect::Notify(n) => Some(n.event_id),
                _ => None,
            })
            .collect();
        for event_id in shown {
            out.extend(self.app.handle(Input::NotifyDelivered { event_id }));
        }
        let now = self.m.now.get();
        self.calls.extend(out.iter().cloned().map(|e| (now, e)));
        out
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.feed(Input::Frame { dt_ms: 16.0 });
        }
    }

    /// The most host calls (effects and measures) in any window `(t - 1000, t]`.
    fn busiest_second(&self) -> usize {
        let mut at: Vec<f64> = self.calls.iter().map(|(t, _)| *t).collect();
        at.extend(self.m.at.borrow().iter());
        at.sort_by(f64::total_cmp);
        at.iter()
            .map(|t| at.iter().filter(|u| **u > t - 1000.0 && **u <= *t).count())
            .max()
            .unwrap_or(0)
    }
}

#[test]
fn an_animating_tree_with_messages_stays_within_the_budget() {
    let mut r = Run::new();
    for i in 0..6u64 {
        let id = if i == 0 {
            "p1".to_owned()
        } else {
            format!("t{i}")
        };
        r.feed(Input::SessionMessage {
            body: task("p1", i + 1, &id, &format!("Task number {i} of the launch")),
        });
        r.feed(Input::SessionMessage {
            body: notify(i as u8 + 1, "bob joined"),
        });
        r.frames(20);
    }
    r.frames(300);
    let draws = r
        .calls
        .iter()
        .filter(|(_, e)| matches!(e, Effect::Draw(_)))
        .count();
    assert!(draws > 30, "the tree animated ({draws} draws)");
    let busiest = r.busiest_second();
    assert!(
        busiest <= BUDGET.per_s as usize,
        "{busiest} host calls in one second, budget {}",
        BUDGET.per_s
    );
    let notified = r
        .calls
        .iter()
        .filter(|(_, e)| matches!(e, Effect::Notify(_)))
        .count();
    assert_eq!(notified, 6, "every notification still went out");
}

#[test]
fn with_draws_held_back_a_send_still_goes_and_the_draw_shows_the_latest_state() {
    let mut r = Run::new();
    for i in 0..6u64 {
        let id = if i == 0 {
            "p1".to_owned()
        } else {
            format!("t{i}")
        };
        r.feed(Input::SessionMessage {
            body: task("p1", i + 1, &id, "x"),
        });
    }
    // Frames of the moving tree measure nothing new: a frame without a draw is one the
    // draws' share of the window has no room for.
    let mut held = false;
    for _ in 0..120 {
        let out = r.feed(Input::Frame { dt_ms: 16.0 });
        assert!(
            !out.contains(&Effect::RequestFrames { on: false }),
            "still moving"
        );
        if draws(&out) == 0 {
            held = true;
            break;
        }
    }
    assert!(held, "the budget held a draw back");
    let out = r.feed(Input::SessionMessage {
        body: notify(9, "bob joined"),
    });
    assert!(
        out.iter().any(|e| matches!(e, Effect::Notify(_))),
        "the notification went out while draws wait: {out:?}"
    );
    assert_eq!(draws(&out), 0);
    r.feed(Input::SessionMessage {
        body: task("p1", 900, "last", "The latest one"),
    });
    let mut drawn = Vec::new();
    for _ in 0..200 {
        let out = r.feed(Input::Frame { dt_ms: 16.0 });
        if let Some(Effect::Draw(c)) = out.iter().find(|e| matches!(e, Effect::Draw(_))) {
            drawn = c.clone();
            break;
        }
    }
    let latest = drawn
        .iter()
        .any(|c| matches!(c, DrawCommand::Region { label, .. } if label == "The latest one"));
    assert!(
        latest,
        "the first draw after the wait shows the latest state"
    );
}
