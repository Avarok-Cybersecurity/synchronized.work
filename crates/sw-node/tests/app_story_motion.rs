#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The committed story played in time under a host that refuses every call over the manifest's
//! rate, with the shipped spring: while the tree moves after the catch-up (and after a
//! reload's catch-up) the host takes a draw at least every [`MAX_GAP_MS`]; within that of a
//! node entering the fold the host has a draw naming it (its region overlay is never left
//! partial); at rest the frame is the exact one; and the App stays within its own budget.

mod app_link;
mod app_story;
mod feed_support;
mod support;

#[path = "../../sw-app-core/tests/support/mod.rs"]
mod app_support;

use std::collections::BTreeMap;

use app_link::Step;
use app_story::fixture::Fixture;
use app_story::motion::{self, region_labels};
use app_story::paced::{Paced, busiest_second, ctx_of};
use sw_app::config;
use sw_app_core::*;

/// The longest the host may go without a new draw while the tree moves: past 50 ms (20 draws a
/// second, three frames at 60 Hz) movement stops reading as motion and reads as a series of
/// jumps.
const MAX_GAP_MS: f64 = 50.0;
/// The frame rates hosts drive the App at.
const RATES: [f32; 2] = [60.0, 120.0];
const PANES: [(f32, f32); 2] = [(1280.0, 800.0), (390.0, 844.0)];
/// Long enough for the spring to come to rest and the budget window to empty.
const SETTLE_MS: f32 = 3000.0;

fn fixture() -> Fixture {
    let text = std::fs::read_to_string(app_story::PATH).unwrap();
    Fixture::from_json(&text)
}

fn shipped(f: &Fixture, size: Size) -> AppCtx {
    let mut ctx = ctx_of(f);
    ctx.theme = sw_app::theme::theme_of(&BTreeMap::new(), sw_app::context::motion());
    ctx.canvas = CanvasAbi::Cw5;
    ctx.size = size;
    ctx
}

/// The shipped configuration under the manifest's rate; or, `unbounded`, with a budget and a
/// host rate nothing reaches, which measures every width the first time (the reference for
/// what the tree looks like at rest).
fn configured(unbounded: bool) -> (AppConfig, f64) {
    let mut c = config::app_config();
    if unbounded {
        c.host_calls.per_s = 1_000_000;
        return (c, 1e9);
    }
    (c, f64::from(config::MAX_CALL_RATE_PER_S))
}

fn played(f: &Fixture, size: Size, dt_ms: f32, unbounded: bool) -> Paced {
    let (c, rate) = configured(unbounded);
    let mut p = Paced::new(shipped(f, size), c, rate);
    p.play(f, dt_ms, SETTLE_MS);
    p
}

/// When the catch-up was folded, when the tail began.
fn phases(f: &Fixture, p: &Paced) -> (f64, f64) {
    let tail = f
        .tail
        .iter()
        .filter(|(s, _)| matches!(s, Step::Deliver(_)))
        .count();
    let n = p.delivered.len();
    (p.delivered[n - tail - 1], p.delivered[n - tail])
}

/// The last draw taken before `until_ms`.
fn at_rest(p: &Paced, until_ms: f64) -> Vec<DrawCommand> {
    let i = p.accepted.partition_point(|a| a.at < until_ms);
    p.accepted[i - 1].commands.clone()
}

/// The movement from `from_ms` to rest: drawn often, every draw complete, and at rest the
/// frame an unbounded host gets.
fn check_movement(what: &str, p: &Paced, (from_ms, until_ms): (f64, f64), exact: &[DrawCommand]) {
    let draws = motion::moving(&p.accepted, from_ms, until_ms);
    let mut problems = Vec::new();
    if draws.len() < 2 {
        problems.push(format!(
            "the movement was not drawn ({} draws)",
            draws.len()
        ));
    }
    let gap = motion::longest_gap(from_ms, draws);
    if gap > MAX_GAP_MS {
        problems.push(format!("{gap:.1} ms without a draw while the tree moved"));
    }
    let late: Vec<_> = motion::shown_after(p)
        .into_iter()
        .filter(|(_, after)| after.is_none_or(|ms| ms > MAX_GAP_MS))
        .collect();
    if !late.is_empty() {
        problems.push(format!(
            "the host showed a partial tree for longer, (delivered ms, shown whole after): {late:?}"
        ));
    }
    if at_rest(p, until_ms) != exact {
        problems.push("at rest, the frame is not the one every width measured gives".into());
    }
    assert!(problems.is_empty(), "{what}: {problems:#?}");
}

fn check_budget(what: &str, p: &Paced) {
    let busiest = busiest_second(&p.host.borrow().calls);
    assert!(
        busiest <= config::HOST_CALLS.per_s as usize,
        "{what}: {busiest} host calls in one second, budget {}",
        config::HOST_CALLS.per_s
    );
    assert_eq!(
        p.host.borrow().refused,
        0,
        "{what}: the host refused nothing"
    );
}

#[test]
fn after_the_catch_up_the_movement_is_drawn_whole_and_often_within_budget() {
    let f = fixture();
    for rate in RATES {
        for (width, height) in PANES {
            let size = Size { width, height };
            let what = format!("{size:?} at {rate} fps");
            let dt_ms = 1000.0 / rate;
            let reference = played(&f, size, dt_ms, true);
            let exact = at_rest(&reference, phases(&f, &reference).1);
            let p = played(&f, size, dt_ms, false);
            check_movement(&what, &p, phases(&f, &p), &exact);
            check_budget(&what, &p);
        }
    }
}

fn reloaded(f: &Fixture, size: Size, dt_ms: f32, unbounded: bool) -> Paced {
    let stored = played(f, size, dt_ms, unbounded)
        .stored
        .expect("the App saved its link");
    let (c, rate) = configured(unbounded);
    motion::remount(f, shipped(f, size), (c, rate), stored, (dt_ms, SETTLE_MS))
}

#[test]
fn after_a_reload_the_catch_up_is_drawn_whole_and_often_within_budget() {
    let f = fixture();
    let mut want = f.after_catch_up.clone();
    want.sort_unstable();
    for rate in RATES {
        for (width, height) in PANES {
            let size = Size { width, height };
            let what = format!("{size:?} at {rate} fps, reload");
            let dt_ms = 1000.0 / rate;
            let exact = reloaded(&f, size, dt_ms, true).drawn.unwrap();
            let p = reloaded(&f, size, dt_ms, false);
            assert_eq!(p.app.screen(), &Screen::Live, "{what}");
            let caught_up = *p.delivered.last().unwrap();
            check_movement(&what, &p, (caught_up, f64::INFINITY), &exact);
            check_budget(&what, &p);
            assert_eq!(
                region_labels(p.drawn.as_ref().unwrap()),
                want,
                "{what}: at rest"
            );
        }
    }
}

#[test]
fn the_last_draw_the_host_takes_after_the_join_names_every_node_at_60_fps_under_the_manifest_rate()
{
    let f = fixture();
    let mut want = f.after_join.clone();
    want.sort_unstable();
    for (width, height) in [(1280.0, 800.0), (390.0, 844.0)] {
        let size = Size { width, height };
        let p = played(&f, size, 1000.0 / 60.0, false);
        let drawn = p.drawn.clone().unwrap();
        assert_eq!(
            region_labels(&drawn),
            want,
            "{size:?}: the last draw taken names every node"
        );
    }
}
