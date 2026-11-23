#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! What the App draws: every screen reachable by keyboard and screen reader, the live tree from
//! sw-view and sw-graph, the read-only composer, and reduced motion.

mod support;

use support::*;
use sw_app_core::*;
use sw_graph::Motion;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn regions(app: &App<FixedAdvance>) -> Vec<(String, String)> {
    app.frame()
        .unwrap()
        .commands
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Region { id, label, .. } => Some((id.clone(), label.clone())),
            _ => None,
        })
        .collect()
}

fn live_app(canvas: CanvasAbi, motion: Motion) -> App<FixedAdvance> {
    let mut app = App::new(
        ctx(granted(true, true), canvas, motion),
        config(),
        FixedAdvance,
    );
    connected(&mut app, Some(linked_to("p1")));
    app
}

#[test]
fn every_screen_offers_its_actions_as_labelled_regions() {
    let mut app = app(granted(true, true), CanvasAbi::Cw5);
    app.start();
    assert!(app.frame().is_some(), "the connecting screen is drawn");
    connected(&mut app, None);
    let ids: Vec<String> = regions(&app).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, ["link-code", "submit-code", "open-site"]);
    app.handle(Input::SessionState {
        status: HostSessionStatus::Closed { reason: "x".into() },
    });
    assert_eq!(
        regions(&app),
        [("retry".to_owned(), copy::ACTION_RETRY.to_owned())]
    );
}

#[test]
fn on_an_abi_1_host_a_pointer_on_a_button_performs_it() {
    let mut app = app(granted(true, true), CanvasAbi::Abi1);
    connected(&mut app, None);
    app.handle(Input::SessionState {
        status: HostSessionStatus::Closed { reason: "x".into() },
    });
    assert!(regions(&app).is_empty(), "no Region command on ABI-1");
    let retry = app
        .frame()
        .unwrap()
        .regions
        .iter()
        .find(|r| r.key == "retry")
        .unwrap()
        .clone();
    let out = app.handle(Input::PointerUp {
        x: retry.x + 1.0,
        y: retry.y + 1.0,
    });
    assert!(out.contains(&Effect::SessionOpen {
        server: "synchronized.work".into()
    }));
}

#[test]
fn the_live_tree_is_sw_views_points_with_an_accessible_name_each() {
    let mut app = live_app(CanvasAbi::Cw5, Motion::Lite);
    app.handle(Input::SessionMessage {
        body: task("p1", 1, "p1", "Ship it"),
    });
    app.handle(Input::SessionMessage {
        body: task("p1", 2, "t2", "Write docs"),
    });
    let state = app.live().unwrap().state();
    let points = sw_view::points::points(state, &sw_app_core::graph::project_name(state));
    let labels: Vec<String> = regions(&app).into_iter().map(|(_, l)| l).collect();
    let expected: Vec<String> = points.iter().map(|p| p.label.clone()).collect();
    assert_eq!(labels, expected);
    assert!(labels.contains(&"Write docs".to_owned()));
}

#[test]
fn the_composer_is_read_only_until_a_controller_key_is_paired() {
    let mut app = live_app(CanvasAbi::Cw5, Motion::Lite);
    app.handle(Input::SessionMessage {
        body: task("p1", 1, "p1", "Ship it"),
    });
    assert_eq!(app.composer(), sw_app_core::screens::Composer::ReadOnly);
    let shows = app.frame().unwrap().commands.iter().any(
        |c| matches!(c, DrawCommand::FillText { text, .. } if text == copy::COMPOSER_READ_ONLY),
    );
    assert!(shows, "the read-only copy is on the live screen");
}

#[test]
fn reduced_motion_lands_at_once_and_full_motion_animates_to_rest() {
    let mut lite = live_app(CanvasAbi::Cw5, Motion::Lite);
    let out = lite.handle(Input::SessionMessage {
        body: task("p1", 1, "p1", "Ship it"),
    });
    assert!(
        !out.contains(&Effect::RequestFrames { on: true }),
        "{out:?}"
    );

    let mut full = live_app(CanvasAbi::Cw5, FULL);
    let out = full.handle(Input::SessionMessage {
        body: task("p1", 1, "p1", "Ship it"),
    });
    assert!(out.contains(&Effect::RequestFrames { on: true }), "{out:?}");
    let mut stopped = false;
    for _ in 0..600 {
        let out = full.handle(Input::Frame { dt_ms: 16.0 });
        assert_eq!(draws(&out), 1);
        if out.contains(&Effect::RequestFrames { on: false }) {
            stopped = true;
            break;
        }
    }
    assert!(stopped, "the spring came to rest and frames were stopped");
}

#[test]
fn every_frame_the_app_draws_is_within_the_canvas_limits() {
    for canvas in [CanvasAbi::Cw5, CanvasAbi::Abi1] {
        let mut app = live_app(canvas, Motion::Lite);
        let mut bodies = vec![task("p1", 1, "p1", "Ship it")];
        // A 300-character progress line: longer than a region label may be.
        bodies.extend(agent_with_line("p1", 2, "p1", &"x".repeat(300)));
        for body in bodies {
            let out = app.handle(Input::SessionMessage { body });
            assert!(
                !out.iter().any(|e| matches!(e, Effect::Fault { .. })),
                "{out:?}"
            );
        }
        assert_eq!(app.screen(), &Screen::Live);
        let commands = &app.frame().unwrap().commands;
        let mut labels = 0;
        for c in commands {
            if let DrawCommand::Region { id, label, .. } = c {
                assert!(sw_app_core::command::is_region_id(id), "{id}");
                assert!(
                    label.chars().count() <= 256,
                    "{} chars",
                    label.chars().count()
                );
                labels += 1;
            }
        }
        assert_eq!(labels > 0, canvas == CanvasAbi::Cw5);
    }
}
