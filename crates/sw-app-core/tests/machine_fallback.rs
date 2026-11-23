#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The transport the App falls back to: no grant at all, `no_identity`, and the `net.host`
//! long-poll's cursor across responses and errors.

mod support;

use support::*;
use sw_app_core::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn neither_grant_is_an_error_whose_next_step_is_the_site() {
    let mut app = app(granted(false, false), CanvasAbi::Cw5);
    app.start();
    assert_eq!(app.screen(), &Screen::Error(AppError::NoTransport));
    let out = app.handle(Input::Key {
        key: "Enter".into(),
    });
    assert!(out.contains(&Effect::CopyText {
        text: "https://synchronized.work".into()
    }));
}

#[test]
fn no_identity_degrades_to_the_long_poll_of_the_saved_project() {
    let mut app = app(granted(true, true), CanvasAbi::Cw5);
    app.start();
    app.handle(Input::LinkLoaded {
        record: Some(linked_to("p1")),
    });
    let out = app.handle(Input::SessionOpenFailed {
        reason: "no_identity".into(),
    });
    assert_eq!(app.transport(), Some(Transport::Fallback));
    let [Effect::HostFetchOpen(get)] = &out[..1] else {
        panic!("{out:?}")
    };
    assert!(
        get.url
            .ends_with("/v1/projects/p1/stream?cursor=0&max_ms=25000"),
        "{}",
        get.url
    );
}

#[test]
fn the_fallback_follows_the_cursor_and_retries_from_it_after_an_error() {
    let mut app = app(granted(false, true), CanvasAbi::Cw5);
    let start = app.start();
    assert!(
        !start
            .iter()
            .any(|e| matches!(e, Effect::SessionOpen { .. }))
    );
    app.handle(Input::LinkLoaded {
        record: Some(linked_to("p1")),
    });
    let items: Vec<_> = (1..=3)
        .map(|c| task("p1", c, &format!("t{c}"), "T"))
        .map(sw_protocol::wire::Bytes)
        .collect();
    let body = sw_protocol::cbor::encode(&sw_protocol::wire::Wire::to_value(&items)).unwrap();
    let out = app.handle(Input::HttpResponse { status: 200, body });
    let url = |out: &[Effect]| {
        out.iter().find_map(|e| match e {
            Effect::HostFetchOpen(g) => Some(g.url.clone()),
            _ => None,
        })
    };
    assert!(url(&out).unwrap().contains("cursor=3&"));
    app.handle(Input::HttpResponse {
        status: 503,
        body: Vec::new(),
    });
    assert_eq!(app.screen(), &Screen::Error(AppError::Http { status: 503 }));
    let out = app.handle(Input::Activate { id: "retry".into() });
    assert!(url(&out).unwrap().contains("cursor=3&"));
}

#[test]
fn the_fallback_without_a_saved_link_says_so() {
    let mut app = app(granted(false, true), CanvasAbi::Cw5);
    app.start();
    app.handle(Input::LinkLoaded { record: None });
    assert_eq!(app.screen(), &Screen::Error(AppError::NotLinkedOnFallback));
}
