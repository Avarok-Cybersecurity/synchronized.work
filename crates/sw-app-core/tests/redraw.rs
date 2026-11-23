#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! A draw the host refused is drawn again — once per frame, from the state as it is then —
//! until the host takes one; and nothing the member does waits for a draw.

mod support;

use support::*;
use sw_app_core::*;
use sw_graph::Motion;
use sw_protocol::payload::Payload;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn live_at_rest() -> App<FixedAdvance> {
    let mut app = App::new(
        ctx(granted(true, true), CanvasAbi::Cw5, Motion::Lite),
        config(),
        FixedAdvance,
    );
    connected(&mut app, Some(linked_to("p1")));
    let out = app.handle(Input::SessionMessage {
        body: task("p1", 1, "p1", "Ship it"),
    });
    assert_eq!(draws(&out), 1);
    assert!(
        !out.contains(&Effect::RequestFrames { on: true }),
        "at rest"
    );
    app
}

#[test]
fn a_refused_draw_is_drawn_again_on_the_next_frame_even_at_rest() {
    let mut app = live_at_rest();
    let shown = app.frame().cloned();
    let out = app.handle(Input::SessionMessage {
        body: task("p1", 2, "t1", "Second"),
    });
    assert_eq!(draws(&out), 1);
    let out = app.handle(Input::DrawRefused);
    assert_eq!(
        app.frame().cloned(),
        shown,
        "the host still shows the frame before"
    );
    assert_eq!(draws(&out), 0, "no attempt outside a frame");
    assert!(out.contains(&Effect::RequestFrames { on: true }), "{out:?}");
    assert_eq!(draws(&app.handle(Input::Key { key: "x".into() })), 0);
    for _ in 0..3 {
        assert_eq!(draws(&app.handle(Input::Frame { dt_ms: 16.0 })), 1);
        assert_eq!(draws(&app.handle(Input::DrawRefused)), 0);
    }
    let out = app.handle(Input::Frame { dt_ms: 16.0 });
    assert_eq!(
        draws(&out),
        1,
        "one attempt on this frame, which the host takes"
    );
    assert!(
        out.contains(&Effect::RequestFrames { on: false }),
        "{out:?}"
    );
    let out = app.handle(Input::Frame { dt_ms: 16.0 });
    assert_eq!(draws(&out), 0, "taken: nothing more to draw");
    let shows_second = app
        .frame()
        .unwrap()
        .commands
        .iter()
        .any(|c| matches!(c, DrawCommand::Region { label, .. } if label == "Second"));
    assert!(shows_second, "the frame taken shows the current state");
}

#[test]
fn eight_keys_in_a_burst_then_enter_redeem_with_every_draw_refused() {
    let mut app = app(granted(true, false), CanvasAbi::Cw5);
    connected(&mut app, None);
    assert!(matches!(app.screen(), Screen::NeedsLink(_)));
    let mut refused = 0;
    for key in "K7M2Q9XW".chars() {
        let out = app.handle(Input::Key { key: key.into() });
        if draws(&out) > 0 {
            refused += 1;
            app.handle(Input::DrawRefused);
        }
    }
    assert_eq!(refused, 8, "every key's draw was tried and refused");
    let out = app.handle(Input::Key {
        key: "Enter".into(),
    });
    let redeems: Vec<String> = sent(&out)
        .into_iter()
        .filter_map(|p| match p {
            Payload::Link(l) => Some(l.code),
            _ => None,
        })
        .collect();
    assert_eq!(redeems, vec!["K7M2Q9XW".to_owned()]);
}
