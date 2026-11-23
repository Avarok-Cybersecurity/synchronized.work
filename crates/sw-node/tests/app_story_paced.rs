#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The committed story (`fixtures/app-session/joined.json`) played to the shipped App
//! configuration and theme: at 60 frames a second under a host that refuses every call over
//! the manifest's rate it stays within its own budget and ends drawn at rest; and at each pane
//! size the recorded tree is fitted inside the pane, legible and reachable.

mod app_link;
mod app_story;
mod feed_support;
mod support;

#[path = "../../sw-app-core/tests/support/mod.rs"]
mod app_support;

use std::collections::BTreeMap;

use app_story::fixture::Fixture;
use app_story::paced::{Paced, busiest_second, ctx_of, expected_sends};
use app_support::FixedAdvance;
use app_support::fit_checks;
use sw_app::config;
use sw_app_core::screens::{Composer, composer_footer};
use sw_app_core::*;
use sw_graph::Motion;

const FRAME_MS: f32 = 1000.0 / 60.0;

fn fixture() -> Fixture {
    let text = std::fs::read_to_string(app_story::PATH).unwrap();
    Fixture::from_json(&text)
}

fn shipped(f: &Fixture, motion: Motion, canvas: CanvasAbi, size: Size) -> AppCtx {
    let mut ctx = ctx_of(f);
    ctx.theme = sw_app::theme::theme_of(&BTreeMap::new(), motion);
    ctx.canvas = canvas;
    ctx.size = size;
    ctx
}

#[test]
fn the_story_at_60_fps_under_the_manifest_rate_stays_in_budget_and_ends_drawn_at_rest() {
    let f = fixture();
    let size = Size {
        width: 390.0,
        height: 844.0,
    };
    let ctx = shipped(&f, sw_app::context::motion(), CanvasAbi::Cw5, size);
    let rate = f64::from(config::MAX_CALL_RATE_PER_S);
    let mut p = Paced::new(ctx, config::app_config(), rate);
    p.play(&f, FRAME_MS, 3000.0);
    let busiest = busiest_second(&p.host.borrow().calls);
    assert!(
        busiest <= config::HOST_CALLS.per_s as usize,
        "{busiest} host calls in one second, budget {}",
        config::HOST_CALLS.per_s
    );
    assert_eq!(p.host.borrow().refused, 0, "the host refused nothing");
    assert_eq!(
        p.sends,
        expected_sends(&f),
        "the App sent the fixture's bytes"
    );
    let [n] = p.notified.as_slice() else {
        panic!("one notification, not {:?}", p.notified);
    };
    assert_eq!((&n.title, &n.body), (&f.title, &f.body));
    assert!(!p.frames_on, "at rest, frames are off");
    let drawn = p.drawn.clone().unwrap();
    p.feed(Input::Resize { size });
    assert_eq!(
        p.drawn,
        Some(drawn),
        "the last frame taken shows the tree at rest"
    );
}

const PANES: [(f32, f32); 4] = [
    (390.0, 844.0),
    (768.0, 1024.0),
    (1280.0, 800.0),
    (320.0, 320.0),
];

fn recorded_at(f: &Fixture, canvas: CanvasAbi, size: Size) -> Paced {
    let ctx = shipped(f, Motion::Lite, canvas, size);
    let mut p = Paced::new(ctx, config::app_config(), 1e9);
    p.play(f, FRAME_MS, 3000.0);
    assert_eq!(p.app.screen(), &Screen::Live);
    p
}

#[test]
fn the_recorded_tree_is_fitted_inside_every_pane_legibly_and_named_in_full() {
    let f = fixture();
    for (width, height) in PANES {
        let pane = Size { width, height };
        let p = recorded_at(&f, CanvasAbi::Cw5, pane);
        let theme = &p.app.ctx().theme;
        let footer = composer_footer(Composer::ReadOnly, theme, pane, &FixedAdvance).len();
        let commands = &p.app.frame().unwrap().commands;
        let s = fit_checks::shown(
            commands,
            footer,
            config::FIT.label_descent_em,
            &FixedAdvance,
        );
        if let Err(e) =
            fit_checks::check(&s, pane, config::FIT.region_min, config::FIT.label_min_px)
        {
            panic!("{pane:?}: {e}");
        }
        let mut names: Vec<&str> = s.regions.iter().map(|(n, _)| n.as_str()).collect();
        let mut want: Vec<&str> = f.after_join.iter().map(String::as_str).collect();
        names.sort_unstable();
        want.sort_unstable();
        assert_eq!(
            names, want,
            "{pane:?}: every node's region carries its full label"
        );
        for (i, (_, a)) in s.regions.iter().enumerate() {
            for (_, b) in &s.regions[i + 1..] {
                assert!(
                    !fit_checks::overlap(*a, *b),
                    "{pane:?}: regions {a:?} {b:?}"
                );
            }
        }
    }
}

#[test]
fn an_abi1_pointer_on_a_drawn_node_of_the_recorded_tree_hits_that_node() {
    let f = fixture();
    let pane = Size {
        width: 390.0,
        height: 844.0,
    };
    let p = recorded_at(&f, CanvasAbi::Abi1, pane);
    let frame = p.app.frame().unwrap();
    let g = p.app.ctx().theme.graph;
    let fills = [g.project, g.task, g.person, g.agent, g.progress];
    let mut keys: Vec<&str> = frame
        .commands
        .iter()
        .filter_map(|c| match c {
            DrawCommand::FillRect { x, y, w, h, color } if fills.contains(color) => {
                assert!(fit_checks::inside([*x, *y, *w, *h], pane));
                frame.hit(x + w / 2.0, y + h / 2.0)
            }
            _ => None,
        })
        .collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(
        keys.len(),
        f.after_join.len(),
        "each drawn node hits its own region"
    );
}
