#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The two transports as SessionPorts under the one `LiveProject`: the pushed CW-6 session
//! and the net.host long-poll, and the choice between them.

mod support;

use support::{config, granted, task};
use sw_app_core::fallback::{FallbackPort, PlanError, check_headers, plan};
use sw_app_core::session::PushedSession;
use sw_app_core::transport::{NO_IDENTITY, after_open_failure, choose};
use sw_app_core::*;
use sw_client::live::LiveProject;
use sw_client::{SessionError, SessionPort, SessionState};
use sw_protocol::cbor;
use sw_protocol::wire::{Bytes, Wire};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const LIMITS: SessionLimits = SessionLimits {
    max_msg_bytes: 64,
    max_msgs_per_s: 8,
};

#[test]
fn the_pushed_session_sends_only_when_connected_and_within_max_msg_bytes() {
    let mut s = PushedSession::new(LIMITS);
    assert_eq!(s.send(vec![1]), Err(SessionError::NotConnected));
    s.opened(77, HostSessionStatus::Connected);
    assert_eq!(s.state(), SessionState::Connected { cid: 77 });
    assert_eq!(s.send(vec![0; 65]), Err(SessionError::TooLarge { max: 64 }));
    s.send(vec![0; 64]).unwrap();
    assert_eq!(s.take_outbox(), vec![vec![0; 64]]);
    assert!(s.take_outbox().is_empty());
}

#[test]
fn the_pushed_session_refuses_an_inbound_body_over_max_msg_bytes() {
    let mut s = PushedSession::new(LIMITS);
    assert_eq!(
        s.message(vec![0; 65]),
        Err(SessionError::TooLarge { max: 64 })
    );
    s.message(vec![9; 3]).unwrap();
    assert_eq!(s.poll_recv(), Ok(Some(vec![9; 3])));
    assert_eq!(s.poll_recv(), Ok(None));
}

#[test]
fn a_closed_session_reports_its_reason_and_sends_nothing() {
    let mut s = PushedSession::new(LIMITS);
    s.opened(77, HostSessionStatus::Connected);
    s.status(HostSessionStatus::Closed {
        reason: "server gone".into(),
    });
    assert_eq!(
        s.state(),
        SessionState::Disconnected {
            reason: "server gone".into()
        }
    );
    assert_eq!(s.send(vec![1]), Err(SessionError::NotConnected));
}

#[test]
fn the_stream_url_carries_the_encoded_project_the_cursor_and_max_ms() {
    let get = plan(&config().poll, "p 1/é", 42).unwrap();
    assert_eq!(
        get.url,
        "https://synchronized.work/v1/projects/p%201%2F%C3%A9/stream?cursor=42&max_ms=25000"
    );
    check_headers(&get.headers).unwrap();
}

#[test]
fn upgrade_connection_and_sec_headers_are_never_produced() {
    for name in [
        "Upgrade",
        "connection",
        "Sec-WebSocket-Key",
        "sec-fetch-mode",
    ] {
        let headers = vec![(name.to_owned(), "x".to_owned())];
        assert_eq!(
            check_headers(&headers),
            Err(PlanError::ForbiddenHeader(name.into()))
        );
    }
    check_headers(&[("Accept".into(), "application/cbor".into())]).unwrap();
}

fn body(messages: &[Vec<u8>]) -> Vec<u8> {
    let items: Vec<Bytes> = messages.iter().cloned().map(Bytes).collect();
    cbor::encode(&items.to_value()).unwrap()
}

#[test]
fn the_long_poll_resumes_from_the_folds_cursor_and_skips_none() {
    let poll = config().poll;
    let mut live = LiveProject::new("p1", "sw-app@test");
    let mut port = FallbackPort::new();
    assert_eq!(port.next_request(&poll, 0), Err(PlanError::NoProject));
    live.subscribe(&mut port).unwrap();
    assert_eq!(port.project(), Some("p1"));
    let first = port.next_request(&poll, live.state().cursor).unwrap();
    assert!(first.url.contains("cursor=0&"), "{}", first.url);

    port.response(&body(&[task("p1", 1, "t1", "A"), task("p1", 2, "t2", "B")]))
        .unwrap();
    live.pump(&mut port).unwrap();
    let next = port.next_request(&poll, live.state().cursor).unwrap();
    assert!(next.url.contains("cursor=2&"), "{}", next.url);

    // An empty answer (the node held the request for max_ms) asks again from the same place.
    port.response(&body(&[])).unwrap();
    live.pump(&mut port).unwrap();
    assert_eq!(port.next_request(&poll, live.state().cursor).unwrap(), next);
    assert_eq!(live.state().tasks.len(), 2);
}

#[test]
fn the_fallback_carries_hello_and_subscribe_only() {
    let mut port = FallbackPort::new();
    let link = sw_app_core::link::redeem(
        "ABCD1234",
        &support::ctx(granted(true, true), CanvasAbi::Cw5, sw_graph::Motion::Lite),
    )
    .unwrap()
    .0;
    assert!(matches!(port.send(link), Err(SessionError::Transport(_))));
    assert!(matches!(port.state(), SessionState::Disconnected { .. }));
}

#[test]
fn a_granted_session_is_the_main_path_and_net_host_the_fallback() {
    assert_eq!(choose(&granted(true, true)), Ok(Transport::Session));
    assert_eq!(choose(&granted(false, true)), Ok(Transport::Fallback));
    assert_eq!(choose(&granted(false, false)), Err(AppError::NoTransport));
}

#[test]
fn no_identity_falls_back_to_net_host_and_any_other_failure_is_shown() {
    assert_eq!(
        after_open_failure(NO_IDENTITY, &granted(true, true)),
        Ok(Transport::Fallback)
    );
    assert_eq!(
        after_open_failure(NO_IDENTITY, &granted(true, false)),
        Err(AppError::NoIdentity)
    );
    assert_eq!(
        after_open_failure("refused", &granted(true, true)),
        Err(AppError::OpenFailed {
            reason: "refused".into()
        })
    );
}
