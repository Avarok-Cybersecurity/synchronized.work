#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Every width through one cache: after a screen's first frame, redrawing it asks the host for
//! no width at all — the animating tree, and the wrapped screens alike.

mod support;

use support::*;
use sw_app_core::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn live(m: &Counted, motion: sw_graph::Motion) -> App<Counted> {
    let mut app = App::new(
        ctx(granted(true, true), CanvasAbi::Cw5, motion),
        config(),
        m.clone(),
    );
    open(&mut app, Some(linked_to("p1")));
    app
}

#[test]
fn after_warm_up_frames_of_an_animating_tree_measure_nothing() {
    let m = Counted::default();
    let mut app = live(&m, FULL);
    for (cursor, id, title) in [
        (1, "p1", "Ship the release"),
        (2, "t1", "Write the migration notes for every tenant"),
        (3, "t2", "Benchmark the cold start"),
    ] {
        app.handle(Input::SessionMessage {
            body: task("p1", cursor, id, title),
        });
    }
    let warm = m.calls();
    assert!(warm > 0, "the first frames measured their text");
    let mut drawn = 0;
    for _ in 0..10 {
        drawn += draws(&app.handle(Input::Frame { dt_ms: 16.0 }));
    }
    assert_eq!(drawn, 10, "the tree was still moving: every frame drew");
    assert_eq!(m.calls(), warm, "no frame after the first measured text");
}

#[test]
fn redrawing_a_wrapped_screen_measures_nothing() {
    let m = Counted::default();
    let mut app = App::new(
        ctx(granted(true, true), CanvasAbi::Cw5, sw_graph::Motion::Lite),
        config(),
        m.clone(),
    );
    open(&mut app, None);
    assert!(matches!(app.screen(), Screen::NeedsLink(_)));
    let warm = m.calls();
    let size = app.ctx().size;
    let out = app.handle(Input::Resize { size });
    assert_eq!(draws(&out), 1);
    assert_eq!(m.calls(), warm, "the link screen's wrap hit the cache");
}

fn open(app: &mut App<Counted>, record: Option<Vec<u8>>) {
    app.start();
    app.handle(Input::LinkLoaded { record });
    app.handle(Input::SessionOpened {
        cid: 0x1234,
        status: HostSessionStatus::Connected,
    });
}

#[test]
fn a_refused_width_is_estimated_and_asked_again_later() {
    let m = Counted::default();
    m.refuse.set(true);
    let mut app = App::new(
        ctx(granted(true, true), CanvasAbi::Cw5, sw_graph::Motion::Lite),
        config(),
        m.clone(),
    );
    open(&mut app, None);
    assert!(app.text().is_empty(), "no refused width is remembered");
    m.refuse.set(false);
    let before = m.calls();
    let size = app.ctx().size;
    app.handle(Input::Resize { size });
    assert!(m.calls() > before, "the widths were asked again");
    assert!(!app.text().is_empty());
}

#[test]
fn the_cache_holds_at_most_its_capacity() {
    let m = Counted::default();
    let mut c = config();
    c.measure.capacity = 8;
    let mut app = App::new(
        ctx(granted(true, true), CanvasAbi::Cw5, sw_graph::Motion::Lite),
        c,
        m.clone(),
    );
    open(&mut app, None);
    assert!(
        m.calls() > 8,
        "the screen needed more widths than the cache holds"
    );
    assert_eq!(app.text().len(), 8);
}
