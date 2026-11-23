#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The tree always inside the pane: one uniform view transform over sw-graph's layout, labels
//! no smaller than the minimum and cut to their node, every region at least the minimum size
//! and named in full, and ABI-1 pointer hits against the same transformed rectangles.

mod support;

use support::fit_checks::{self, Shown};
use support::*;
use sw_app_core::screens::{Composer, composer_footer};
use sw_app_core::*;
use sw_graph::Motion;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const PANES: [(f32, f32); 4] = [
    (390.0, 844.0),
    (768.0, 1024.0),
    (1280.0, 800.0),
    (320.0, 320.0),
];

fn titles(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| format!("Deliverable number {i} of the quarter"))
        .collect()
}

/// A Project of `n` Tasks (each with its owner), wider than a phone, drawn at `pane`.
fn wide(canvas: CanvasAbi, pane: Size, n: usize) -> App<FixedAdvance> {
    let mut c = ctx(granted(true, true), canvas, Motion::Lite);
    c.size = pane;
    let mut app = App::new(c, config(), FixedAdvance);
    connected(&mut app, Some(linked_to("p1")));
    app.handle(Input::SessionMessage {
        body: task("p1", 1, "p1", "The project"),
    });
    for (i, title) in titles(n).iter().enumerate() {
        app.handle(Input::SessionMessage {
            body: task("p1", i as u64 + 2, &format!("t{i}"), title),
        });
    }
    assert_eq!(app.screen(), &Screen::Live);
    app
}

fn shown(app: &App<FixedAdvance>) -> Shown {
    let t = theme(Motion::Lite);
    let footer = composer_footer(Composer::ReadOnly, &t, app.ctx().size, &FixedAdvance).len();
    let commands = &app.frame().unwrap().commands;
    fit_checks::shown(
        commands,
        footer,
        config().fit.label_descent_em,
        &FixedAdvance,
    )
}

#[test]
fn a_tree_wider_than_the_pane_is_fitted_inside_it_at_every_size() {
    let fit = config().fit;
    for (width, height) in PANES {
        let pane = Size { width, height };
        let app = wide(CanvasAbi::Cw5, pane, 9);
        let s = shown(&app);
        if let Err(e) = fit_checks::check(&s, pane, fit.region_min, fit.label_min_px) {
            panic!("{pane:?}: {e}");
        }
        let names: Vec<&str> = s.regions.iter().map(|(n, _)| n.as_str()).collect();
        for title in titles(9) {
            assert!(
                names.contains(&title.as_str()),
                "{pane:?}: {title:?} is named in full"
            );
        }
    }
}

#[test]
fn at_a_phone_width_labels_are_cut_with_an_ellipsis_not_shrunk() {
    let app = wide(
        CanvasAbi::Cw5,
        Size {
            width: 390.0,
            height: 844.0,
        },
        3,
    );
    let s = shown(&app);
    let cut = s
        .labels
        .iter()
        .filter(|(t, ..)| t.ends_with(fit::ELLIPSIS))
        .count();
    assert!(cut > 0, "{:?}", s.labels);
    assert!(
        s.labels
            .iter()
            .all(|(.., px)| *px >= config().fit.label_min_px)
    );
}

#[test]
fn an_abi1_pointer_hit_on_a_drawn_node_resolves_to_that_node() {
    let pane = Size {
        width: 390.0,
        height: 844.0,
    };
    let app = wide(CanvasAbi::Abi1, pane, 9);
    let frame = app.frame().unwrap();
    let t = theme(Motion::Lite).graph;
    let mut hit = Vec::new();
    let mut rects = 0;
    for c in &frame.commands {
        if let DrawCommand::FillRect { x, y, w, h, color } = c
            && [t.project, t.task].contains(color)
        {
            let key = frame.hit(x + w / 2.0, y + h / 2.0).unwrap();
            assert!(
                fit_checks::inside([*x, *y, *w, *h], pane),
                "the node is drawn inside"
            );
            hit.push(key.to_owned());
            rects += 1;
        }
    }
    hit.sort();
    hit.dedup();
    assert!(rects > 9, "the Project and its Tasks are drawn");
    assert_eq!(
        hit.len(),
        rects,
        "every drawn node hits its own region: {hit:?}"
    );
}

#[test]
fn past_the_legible_scale_labels_are_hidden_and_regions_keep_the_names() {
    let pane = Size {
        width: 390.0,
        height: 844.0,
    };
    let s = shown(&wide(CanvasAbi::Cw5, pane, 9));
    assert!(
        s.labels.is_empty(),
        "no label row fits between the levels: {:?}",
        s.labels
    );
    let fit = config().fit;
    fit_checks::check(&s, pane, fit.region_min, fit.label_min_px).unwrap();
    assert!(
        s.regions
            .iter()
            .any(|(n, _)| n == "Deliverable number 8 of the quarter")
    );
}
