#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! A member whose workspace account has no Citadel identity: the open answers `no_identity`,
//! the `net.host` long-poll is still tried when granted, and whatever then fails, the member
//! sees one screen that says what is missing and what to do, never a generic error.

mod support;

use support::*;
use sw_app_core::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const NO_IDENTITY: &str = sw_app_core::transport::NO_IDENTITY;
const SITE: &str = "https://synchronized.work";

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

fn texts(app: &App<FixedAdvance>) -> String {
    let lines: Vec<&str> = app
        .frame()
        .unwrap()
        .commands
        .iter()
        .filter_map(|c| match c {
            DrawCommand::FillText { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    lines.join(" ")
}

/// Started on a session, the saved link read, and the open refused with `reason`.
fn refused(
    granted: Granted,
    record: Option<Vec<u8>>,
    reason: &str,
) -> (App<FixedAdvance>, Vec<Effect>) {
    let mut app = app(granted, CanvasAbi::Cw5);
    app.start();
    app.handle(Input::LinkLoaded { record });
    let out = app.handle(Input::SessionOpenFailed {
        reason: reason.into(),
    });
    (app, out)
}

fn opens(out: &[Effect]) -> bool {
    out.iter().any(|e| matches!(e, Effect::SessionOpen { .. }))
}

fn fetches(out: &[Effect]) -> bool {
    out.iter().any(|e| matches!(e, Effect::HostFetchOpen(_)))
}

const NO_IDENTITY_SCREEN: Screen = Screen::Error(AppError::NoIdentity);

#[test]
fn without_net_host_the_member_sees_what_is_missing_and_what_to_do() {
    let (app, _) = refused(granted(true, false), Some(linked_to("p1")), NO_IDENTITY);
    assert_eq!(app.screen(), &NO_IDENTITY_SCREEN);
    assert_eq!(
        regions(&app),
        [
            ("retry".to_owned(), "Try again".to_owned()),
            (
                "open-site".to_owned(),
                "Copy the synchronized.work address".to_owned()
            ),
        ]
    );
    let shown = texts(&app);
    let words = shown.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(words.contains("no Citadel identity"), "{words}");
    assert!(
        words.contains("so your Citadel agent can connect to synchronized.work for you"),
        "{words}"
    );
    assert!(
        words.contains("Nothing is wrong with your workspace or your data"),
        "{words}"
    );
    assert!(words.contains("account settings"), "{words}");
    let lower = words.to_lowercase();
    for banned in [
        "portable",
        "binding",
        "cid",
        "session",
        "net.host",
        "long-poll",
        "fallback",
        "support",
    ] {
        assert!(!lower.contains(banned), "{banned:?} in {words}");
    }
}

#[test]
fn the_secondary_action_hands_over_the_site_address() {
    let (mut app, _) = refused(granted(true, false), None, NO_IDENTITY);
    let out = app.handle(Input::Activate {
        id: "open-site".into(),
    });
    assert_eq!(out, [Effect::CopyText { text: SITE.into() }]);
}

#[test]
fn a_fallback_answering_404_is_the_no_identity_screen() {
    let (mut app, out) = refused(granted(true, true), Some(linked_to("p1")), NO_IDENTITY);
    assert!(fetches(&out), "the long-poll is still tried: {out:?}");
    app.handle(Input::HttpResponse {
        status: 404,
        body: Vec::new(),
    });
    assert_eq!(app.screen(), &NO_IDENTITY_SCREEN);
}

#[test]
fn a_fallback_that_cannot_be_reached_is_the_no_identity_screen() {
    let (mut app, _) = refused(granted(true, true), Some(linked_to("p1")), NO_IDENTITY);
    app.handle(Input::HttpFailed {
        reason: "network".into(),
    });
    assert_eq!(app.screen(), &NO_IDENTITY_SCREEN);
}

#[test]
fn a_fallback_that_answers_is_live_as_before() {
    let (mut app, _) = refused(granted(true, true), Some(linked_to("p1")), NO_IDENTITY);
    let items = vec![sw_protocol::wire::Bytes(task("p1", 1, "p1", "Ship it"))];
    let body = sw_protocol::cbor::encode(&sw_protocol::wire::Wire::to_value(&items)).unwrap();
    let out = app.handle(Input::HttpResponse { status: 200, body });
    assert_eq!(app.screen(), &Screen::Live);
    assert_eq!(app.transport(), Some(Transport::Fallback));
    assert!(fetches(&out), "the long-poll goes on: {out:?}");
}

#[test]
fn a_fallback_without_a_saved_link_is_the_no_identity_screen() {
    let (app, _) = refused(granted(true, true), None, NO_IDENTITY);
    assert_eq!(app.screen(), &NO_IDENTITY_SCREEN);

    // The refusal may come before the saved link is read.
    let mut late = support::app(granted(true, true), CanvasAbi::Cw5);
    late.start();
    late.handle(Input::SessionOpenFailed {
        reason: NO_IDENTITY.into(),
    });
    late.handle(Input::LinkLoaded { record: None });
    assert_eq!(late.screen(), &NO_IDENTITY_SCREEN);
}

#[test]
fn trying_again_starts_over_with_a_fresh_session_open() {
    let (mut app, _) = refused(granted(true, true), Some(linked_to("p1")), NO_IDENTITY);
    app.handle(Input::HttpResponse {
        status: 404,
        body: Vec::new(),
    });
    let out = app.handle(Input::Key {
        key: "Enter".into(),
    });
    assert!(opens(&out), "{out:?}");
    assert!(!fetches(&out), "not the long-poll again: {out:?}");
    assert_eq!(app.screen(), &Screen::Connecting);
    assert_eq!(app.transport(), Some(Transport::Session));

    // The member has an identity now: the normal path.
    let out = app.handle(Input::SessionOpened {
        cid: 0x1234,
        status: HostSessionStatus::Connected,
    });
    assert!(
        sent(&out)
            .iter()
            .any(|p| matches!(p, sw_protocol::payload::Payload::Subscribe(_))),
        "{out:?}"
    );
}

#[test]
fn other_refusals_are_not_the_no_identity_screen() {
    for reason in ["ungranted", "unavailable", "not_configured", "remote: 503"] {
        let (mut app, out) = refused(granted(true, true), Some(linked_to("p1")), reason);
        assert!(!fetches(&out), "{reason}: no long-poll");
        assert_eq!(
            app.screen(),
            &Screen::Error(AppError::OpenFailed {
                reason: reason.into()
            }),
            "{reason}"
        );
        assert_eq!(
            regions(&app).first().map(|(id, _)| id.as_str()),
            Some("retry")
        );
        let out = app.handle(Input::Key {
            key: "Enter".into(),
        });
        assert!(opens(&out), "{reason}: try again reopens");
    }
}
