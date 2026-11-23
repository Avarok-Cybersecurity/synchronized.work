#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! A notification the host refused is tried again after a spacing of frame time, delivered
//! once, never twice for one event, and given up (as a `Fault`) after a bounded number of
//! attempts. The support's config: 3 attempts, 250 ms apart.

mod support;

use support::*;
use sw_app_core::*;
use sw_graph::Motion;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const ID: [u8; 32] = [7; 32];

fn live() -> App<FixedAdvance> {
    let mut app = App::new(
        ctx(granted(true, true), CanvasAbi::Cw5, Motion::Lite),
        config(),
        FixedAdvance,
    );
    connected(&mut app, Some(linked_to("p1")));
    app.handle(Input::SessionMessage {
        body: task("p1", 1, "p1", "Ship it"),
    });
    app
}

fn notifies(out: &[Effect]) -> usize {
    out.iter()
        .filter(|e| matches!(e, Effect::Notify(_)))
        .count()
}

fn faults(out: &[Effect]) -> usize {
    out.iter()
        .filter(|e| matches!(e, Effect::Fault { .. }))
        .count()
}

/// Frames of `dt_ms` for `ms` of frame time: the effects, and the frame time each came at.
fn run(app: &mut App<FixedAdvance>, dt_ms: f32, ms: f32) -> Vec<(f32, Effect)> {
    let mut out = Vec::new();
    let mut t = 0.0;
    while t < ms {
        t += dt_ms;
        out.extend(
            app.handle(Input::Frame { dt_ms })
                .into_iter()
                .map(|e| (t, e)),
        );
    }
    out
}

fn notified(timed: &[(f32, Effect)]) -> Vec<f32> {
    timed
        .iter()
        .filter(|(_, e)| matches!(e, Effect::Notify(_)))
        .map(|(t, _)| *t)
        .collect()
}

#[test]
fn a_refused_notification_is_delivered_later_exactly_once() {
    let mut app = live();
    let out = app.handle(Input::SessionMessage {
        body: notify(ID[0], "bob joined"),
    });
    assert_eq!(notifies(&out), 1);
    let out = app.handle(Input::NotifyRefused { event_id: ID });
    assert_eq!(notifies(&out), 0);
    assert!(out.contains(&Effect::RequestFrames { on: true }), "{out:?}");
    let timed = run(&mut app, 50.0, 1000.0);
    let at = notified(&timed);
    assert_eq!(
        at.len(),
        1,
        "tried again once, while it waits for an answer"
    );
    assert!(at[0] >= 250.0, "not before the spacing: {at:?}");
    app.handle(Input::NotifyDelivered { event_id: ID });
    assert!(notified(&run(&mut app, 50.0, 3000.0)).is_empty());
}

#[test]
fn one_event_id_is_delivered_once_whatever_arrives_again() {
    let mut app = live();
    let mut total = notifies(&app.handle(Input::SessionMessage {
        body: notify(ID[0], "bob joined"),
    }));
    app.handle(Input::NotifyRefused { event_id: ID });
    total += notifies(&app.handle(Input::SessionMessage {
        body: notify(ID[0], "bob joined"),
    }));
    total += notified(&run(&mut app, 50.0, 1000.0)).len();
    app.handle(Input::NotifyDelivered { event_id: ID });
    total += notifies(&app.handle(Input::SessionMessage {
        body: notify(ID[0], "bob joined"),
    }));
    total += notified(&run(&mut app, 50.0, 1000.0)).len();
    assert_eq!(
        total, 2,
        "the first attempt and the one retry, nothing more"
    );
}

#[test]
fn a_notification_refused_every_time_is_given_up_with_a_fault() {
    let mut app = live();
    let mut tries = notifies(&app.handle(Input::SessionMessage {
        body: notify(ID[0], "bob joined"),
    }));
    let mut fault = 0;
    for _ in 0..10 {
        fault += faults(&app.handle(Input::NotifyRefused { event_id: ID }));
        let timed = run(&mut app, 50.0, 400.0);
        tries += notified(&timed).len();
        fault += timed
            .iter()
            .filter(|(_, e)| matches!(e, Effect::Fault { .. }))
            .count();
    }
    assert_eq!(tries, 3, "the configured attempts, then no more");
    assert_eq!(fault, 1, "the give-up is reported once");
}
