#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The first run end to end over the App's inputs and effects: connect, link, subscribe,
//! notify and reconnect (the fallback is `machine_fallback.rs`).

mod support;

use support::*;
use sw_app_core::link::storage_key;
use sw_app_core::*;
use sw_protocol::payload::{Payload, Subscribe};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn subscribed(effects: &[Effect]) -> Option<(String, u64)> {
    sent(effects).into_iter().find_map(|p| match p {
        Payload::Subscribe(Subscribe {
            project_id,
            since_cursor,
            ..
        }) => Some((project_id, since_cursor)),
        _ => None,
    })
}

#[test]
fn the_first_run_reads_the_link_opens_the_session_and_asks_for_a_code() {
    let mut app = app(granted(true, true), CanvasAbi::Cw5);
    let start = app.start();
    assert!(start.contains(&Effect::ReadUserStorage {
        key: storage_key(NODE)
    }));
    assert!(start.contains(&Effect::SessionOpen {
        server: "synchronized.work".into()
    }));
    assert_eq!(app.screen(), &Screen::Connecting);
    let rest = connected(&mut app, None);
    let payloads = sent(&rest);
    assert!(
        matches!(payloads.as_slice(), [Payload::Hello(_)]),
        "{payloads:?}"
    );
    assert!(matches!(app.screen(), Screen::NeedsLink(l) if l.pending.is_none()));
}

#[test]
fn a_redeemed_code_is_saved_and_subscribes_the_bound_project() {
    let mut app = app(granted(true, true), CanvasAbi::Cw5);
    connected(&mut app, None);
    let mut out = Vec::new();
    for key in ["A", "B", "C", "D", "1", "2", "3", "4", "Enter"] {
        out.extend(app.handle(Input::Key { key: key.into() }));
    }
    let Some(Payload::Link(link)) = sent(&out).into_iter().next() else {
        panic!("no LINK_REDEEM: {out:?}")
    };
    assert_eq!(link.code, "ABCD1234");
    let Screen::NeedsLink(screen) = app.screen().clone() else {
        panic!()
    };
    let pending = screen.pending.expect("a redeem in flight");

    let out = app.handle(Input::SessionMessage {
        body: receipt(pending, true, "p1", None),
    });
    assert!(out.contains(&Effect::WriteUserStorage {
        key: storage_key(NODE),
        value: linked_to("p1")
    }));
    assert_eq!(subscribed(&out), Some(("p1".into(), 0)));
    assert_eq!(app.screen(), &Screen::Empty(EmptyReason::NothingYet));

    let out = app.handle(Input::SessionMessage {
        body: task("p1", 1, "p1", "Ship it"),
    });
    assert_eq!(app.screen(), &Screen::Live);
    assert_eq!(draws(&out), 1);
}

#[test]
fn a_refused_code_returns_to_the_entry_with_the_nodes_reason() {
    let mut app = app(granted(true, true), CanvasAbi::Cw5);
    connected(&mut app, None);
    for key in ["A", "B", "C", "D", "1", "2", "3", "4", "Enter"] {
        app.handle(Input::Key { key: key.into() });
    }
    let Screen::NeedsLink(screen) = app.screen().clone() else {
        panic!()
    };
    let pending = screen.pending.unwrap();
    // Somebody else's receipt does not settle this redeem.
    app.handle(Input::SessionMessage {
        body: receipt([1; 32], false, "", Some("x")),
    });
    assert!(matches!(app.screen(), Screen::NeedsLink(l) if l.pending == Some(pending)));
    let out = app.handle(Input::SessionMessage {
        body: receipt(pending, false, "", Some("InviteInvalid")),
    });
    let Screen::NeedsLink(screen) = app.screen() else {
        panic!()
    };
    assert_eq!(
        screen.refused.as_ref().unwrap().reason.as_deref(),
        Some("InviteInvalid")
    );
    assert!(screen.entry.code().is_empty() && screen.pending.is_none());
    assert!(
        !out.iter()
            .any(|e| matches!(e, Effect::WriteUserStorage { .. }))
    );
}

#[test]
fn a_saved_link_with_no_project_is_the_empty_state_with_a_way_to_relink() {
    let mut app = app(granted(true, true), CanvasAbi::Cw5);
    let none = sw_app_core::link::LinkRecord { project: None }
        .to_bytes()
        .unwrap();
    connected(&mut app, Some(none));
    assert_eq!(app.screen(), &Screen::Empty(EmptyReason::NoProject));
    assert_eq!(app.screen().actions()[0], NextAction::EnterLinkCode);
}

#[test]
fn a_reconnect_resubscribes_from_the_cursor() {
    let mut app = app(granted(true, true), CanvasAbi::Cw5);
    let out = connected(&mut app, Some(linked_to("p1")));
    assert_eq!(subscribed(&out), Some(("p1".into(), 0)));
    for cursor in 1..=5 {
        app.handle(Input::SessionMessage {
            body: task("p1", cursor, &format!("t{cursor}"), "T"),
        });
    }
    app.handle(Input::SessionState {
        status: HostSessionStatus::Closed {
            reason: "dropped".into(),
        },
    });
    assert_eq!(
        app.screen(),
        &Screen::Error(AppError::SessionClosed {
            reason: "dropped".into()
        })
    );
    let out = app.handle(Input::Activate { id: "retry".into() });
    assert!(out.contains(&Effect::SessionOpen {
        server: "synchronized.work".into()
    }));
    let out = app.handle(Input::SessionOpened {
        cid: 0x1234,
        status: HostSessionStatus::Connected,
    });
    assert_eq!(subscribed(&out), Some(("p1".into(), 5)));
    assert_eq!(app.screen(), &Screen::Live);
}

#[test]
fn a_notify_is_one_host_notification_and_nothing_else() {
    let mut app = app(granted(true, true), CanvasAbi::Cw5);
    connected(&mut app, Some(linked_to("p1")));
    app.handle(Input::SessionMessage {
        body: task("p1", 1, "p1", "Ship it"),
    });
    let screen = app.screen().clone();
    let out = app.handle(Input::SessionMessage {
        body: notify(9, "bob joined Ship it"),
    });
    let [Effect::Notify(note)] = out.as_slice() else {
        panic!("a NOTIFY did more than notify: {out:?}")
    };
    assert_eq!(
        (note.body.as_str(), note.node_id.as_str()),
        ("bob joined Ship it", NODE)
    );
    assert_eq!(app.screen(), &screen);
    assert!(
        app.handle(Input::SessionMessage {
            body: notify(9, "again")
        })
        .is_empty()
    );
}

#[test]
fn without_the_notifications_grant_a_notify_is_dropped() {
    let mut g = granted(true, true);
    g.notifications = false;
    let mut app = app(g, CanvasAbi::Cw5);
    connected(&mut app, Some(linked_to("p1")));
    assert!(
        app.handle(Input::SessionMessage {
            body: notify(9, "x")
        })
        .is_empty()
    );
}
