#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Text that costs more than a frame can afford never stops the drawing: the budget keeps a
//! draw due every `WINDOW_MS / draw_floor` whatever else is spent, a frame that cannot
//! measure all its text is drawn with estimates, the widths fill in over the frames that
//! follow and the frame at rest is the exact one; and a slightly larger tree is drawn at the
//! same scale, so its labels need no new measures.

mod support;

use support::*;
use sw_app_core::fit::{self, Area};
use sw_app_core::pace::{HostCallBudget, Ledger, Tier, WINDOW_MS};
use sw_app_core::*;
use sw_graph::{Motion, Pos};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// The shipped budget's shape: draws keep 32 slots of their 64.
const BUDGET: HostCallBudget = HostCallBudget {
    per_s: 80,
    reserve: 16,
    draw_floor: 32,
};
const FRAME_MS: f32 = 16.0;

#[test]
fn whatever_measures_take_a_due_draw_always_finds_its_slot() {
    let mut l = Ledger::new(BUDGET);
    let floor_ms = WINDOW_MS / f64::from(BUDGET.draw_floor);
    let (mut last, mut longest, mut drawn) = (0.0f64, 0.0f64, 0);
    for frame in 0..600 {
        l.advance(FRAME_MS);
        // Sends at most `reserve - 1` a window, in bursts.
        if frame % 63 == 0 {
            for _ in 0..BUDGET.reserve - 1 {
                assert!(l.room(Tier::Essential) > 0);
                l.spend(Tier::Essential, 1);
            }
        }
        // Every frame wants to draw and to measure all it may.
        if let Some(measures) = l.draw(true) {
            assert!(l.room(Tier::Low) > measures, "the draw's own slot is left");
            l.spend(Tier::Low, measures + 1);
            l.drew();
            longest = longest.max(l.now_ms() - last);
            last = l.now_ms();
            drawn += 1;
        }
    }
    let bound = floor_ms + f64::from(FRAME_MS);
    assert!(
        longest <= bound,
        "{longest} ms between draws, bound {bound}"
    );
    assert!(
        drawn >= 600 * 16 / 1000 * BUDGET.draw_floor as usize / 2,
        "{drawn} draws"
    );
}

/// A tree of `n` Tasks under the shipped budget's shape, every label new to the cache.
fn tree(budget: HostCallBudget, n: u64, motion: Motion) -> (App<Counted>, Counted, Vec<Drawn>) {
    let m = Counted::default();
    let mut c = config();
    c.host_calls = budget;
    let mut app = App::new(
        ctx(granted(true, true), CanvasAbi::Cw5, motion),
        c,
        m.clone(),
    );
    app.start();
    app.handle(Input::LinkLoaded {
        record: Some(linked_to("p1")),
    });
    app.handle(Input::SessionOpened {
        cid: 0x1234,
        status: HostSessionStatus::Connected,
    });
    let mut drawn = Vec::new();
    for i in 0..n {
        let id = if i == 0 {
            "p1".to_owned()
        } else {
            format!("t{i}")
        };
        let title = format!("Milestone {i} of the launch plan");
        let out = app.handle(Input::SessionMessage {
            body: task("p1", i + 1, &id, &title),
        });
        if draws(&out) > 0 {
            drawn.push(drawn_now(&app, m.now.get()));
        }
    }
    for _ in 0..240 {
        m.now.set(m.now.get() + f64::from(FRAME_MS));
        if draws(&app.handle(Input::Frame { dt_ms: FRAME_MS })) > 0 {
            drawn.push(drawn_now(&app, m.now.get()));
        }
    }
    (app, m, drawn)
}

/// When a draw was taken, how many nodes it names, and where it puts their shapes.
type Drawn = (f64, usize, Vec<[f32; 4]>);

fn drawn_now(app: &App<Counted>, at: f64) -> Drawn {
    let commands = &app.frame().unwrap().commands;
    let regions = commands
        .iter()
        .filter(|c| matches!(c, DrawCommand::Region { id, .. } if id != "open-site"))
        .count();
    let shapes = commands
        .iter()
        .filter_map(|c| match c {
            DrawCommand::FillRect { x, y, w, h, .. } => Some([*x, *y, *w, *h]),
            _ => None,
        })
        .collect();
    (at, regions, shapes)
}

#[test]
fn a_frame_short_of_measures_is_drawn_with_estimates_and_is_exact_at_rest() {
    const N: u64 = 8;
    let unbounded = config().host_calls;
    let (exact, ..) = tree(unbounded, N, FULL);
    let (app, m, drawn) = tree(BUDGET, N, FULL);
    let floor_ms = WINDOW_MS / f64::from(BUDGET.draw_floor);
    let moved = drawn
        .windows(2)
        .rposition(|w| w[0].2 != w[1].2)
        .expect("the tree moved");
    for w in drawn[..=moved + 1].windows(2) {
        let gap = w[1].0 - w[0].0;
        assert!(
            gap <= floor_ms + f64::from(FRAME_MS),
            "{gap} ms between draws while moving"
        );
    }
    let first_full = drawn.iter().find(|(_, r, _)| *r >= N as usize).unwrap().0;
    let last_measure = m.at.borrow().iter().copied().fold(0.0, f64::max);
    assert!(
        first_full < last_measure,
        "the whole tree was drawn ({first_full} ms) before its last width was measured \
         ({last_measure} ms)"
    );
    assert_eq!(
        app.frame().unwrap().commands,
        exact.frame().unwrap().commands,
        "at rest the frame is the one every width measured gives"
    );
}

#[test]
fn without_movement_a_frame_drawn_with_estimates_is_still_finished() {
    const N: u64 = 8;
    let (exact, ..) = tree(config().host_calls, N, Motion::Lite);
    let (app, _, drawn) = tree(BUDGET, N, Motion::Lite);
    assert!(
        drawn.len() > 1,
        "drawn with estimates, then again once measured"
    );
    assert_eq!(
        app.frame().unwrap().commands,
        exact.frame().unwrap().commands,
        "at rest the frame is the one every width measured gives"
    );
}

#[test]
fn a_slightly_wider_tree_is_drawn_at_the_same_scale_and_label_size() {
    let p = config().fit;
    let area = Area {
        x: 0.0,
        y: 0.0,
        w: 850.0,
        h: 800.0,
    };
    let row = |n: usize, gap: f32| -> Vec<(Pos, (f32, f32))> {
        (0..n)
            .map(|i| {
                (
                    Pos {
                        x: i as f32 * (200.0 + gap),
                        y: 0.0,
                    },
                    (200.0, 64.0),
                )
            })
            .collect()
    };
    let narrow = fit::fit(&row(5, 0.0), area, 12.0, &p);
    let wider = fit::fit(&row(5, 4.0), area, 12.0, &p);
    assert!(narrow.scale < 1.0 && wider.scale <= narrow.scale);
    assert_eq!(
        (narrow.scale, narrow.label_px),
        (wider.scale, wider.label_px),
        "1.6 % more tree is not a new label size"
    );
    let exact = area.w / (5.0 * 204.0 - 4.0);
    assert!(
        wider.scale <= exact,
        "the stepped fit stays inside the pane"
    );
}
