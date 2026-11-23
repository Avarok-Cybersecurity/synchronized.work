#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Host events and replies to the core's `Input`s: session status both ways, refusals to the
//! reason the core reads (`no_identity` intact), the fallback request, and the stream read.

use citadel_app_sdk_types::calls::{HttpMethod, SessionEvent, SessionStatus, StreamChunk};
use citadel_app_sdk_types::{
    AppEvent, Bytes, KeyEvent, Modifiers, PointerEvent, PointerKind, PointerType, Refused,
};
use sw_app::map::status_of;
use sw_app::map::{StreamStep, frame_input, http_request_of, input_of, read_step, reason_of};
use sw_app_core::fallback::{HttpGet, PlanError};
use sw_app_core::transport::{self, NO_IDENTITY, Transport};
use sw_app_core::{Granted, HostSessionStatus, Input, Size};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
    meta: false,
};

fn pointer(kind: PointerKind, button: i16) -> AppEvent {
    AppEvent::Pointer(PointerEvent {
        kind,
        pointer_type: PointerType::Mouse,
        x: 12.0,
        y: 34.0,
        button,
        buttons: 0,
        modifiers: NONE,
    })
}

fn key(down: bool) -> AppEvent {
    AppEvent::Key(KeyEvent {
        down,
        key: "a".into(),
        code: "KeyA".into(),
        repeat: false,
        modifiers: NONE,
    })
}

/// Every SDK status and the core status it is; no two SDK statuses share one.
fn statuses() -> Vec<(SessionStatus, HostSessionStatus)> {
    vec![
        (SessionStatus::Connecting, HostSessionStatus::Connecting),
        (SessionStatus::Connected, HostSessionStatus::Connected),
        (
            SessionStatus::Closed {
                reason: "bye".into(),
            },
            HostSessionStatus::Closed {
                reason: "bye".into(),
            },
        ),
    ]
}

#[test]
fn session_status_maps_one_to_one_in_both_directions() {
    let pairs = statuses();
    for (sdk, core) in &pairs {
        assert_eq!(status_of(sdk.clone()), *core);
        let back: Vec<&SessionStatus> = pairs
            .iter()
            .filter(|(_, c)| c == core)
            .map(|(s, _)| s)
            .collect();
        assert_eq!(back, [sdk], "{core:?} comes from exactly one SDK status");
    }
}

#[test]
fn a_no_identity_refusal_reaches_the_core_as_no_identity_and_falls_back() {
    let reason = reason_of(&Refused::Unavailable(NO_IDENTITY.into()));
    assert_eq!(reason, NO_IDENTITY);
    let granted = Granted {
        citadel_session: true,
        net_host: true,
        notifications: false,
        storage_user: true,
        storage_local: false,
    };
    assert_eq!(
        transport::after_open_failure(&reason, &granted),
        Ok(Transport::Fallback)
    );
}

#[test]
fn every_other_refusal_keeps_its_kind_in_the_reason() {
    assert_eq!(reason_of(&Refused::Ungranted), "ungranted");
    assert_eq!(
        reason_of(&Refused::Unavailable("session".into())),
        "session"
    );
    assert_eq!(
        reason_of(&Refused::Limit("session_rate".into())),
        "limit: session_rate"
    );
    assert_eq!(
        reason_of(&Refused::Policy("server".into())),
        "policy: server"
    );
    assert_eq!(reason_of(&Refused::Remote(503)), "remote: 503");
    assert_ne!(reason_of(&Refused::Policy(NO_IDENTITY.into())), NO_IDENTITY);
}

#[test]
fn session_events_of_the_apps_own_session_become_inputs_and_no_others_do() {
    let message = |session| AppEvent::Session {
        session,
        event: SessionEvent::Message {
            body: Bytes(vec![1, 2]),
        },
    };
    assert_eq!(
        input_of(message(7), Some(7)),
        Some(Input::SessionMessage { body: vec![1, 2] })
    );
    assert_eq!(input_of(message(8), Some(7)), None);
    assert_eq!(input_of(message(7), None), None);
    for (sdk, core) in statuses() {
        let state = AppEvent::Session {
            session: 7,
            event: SessionEvent::State { state: sdk },
        };
        assert_eq!(
            input_of(state, Some(7)),
            Some(Input::SessionState { status: core })
        );
    }
}

#[test]
fn input_events_become_the_cores_inputs() {
    let resize = AppEvent::Resize {
        width: 640,
        height: 480,
        dpr: 2.0,
    };
    let size = Size {
        width: 640.0,
        height: 480.0,
    };
    assert_eq!(input_of(resize, None), Some(Input::Resize { size }));
    let up = Some(Input::PointerUp { x: 12.0, y: 34.0 });
    assert_eq!(input_of(pointer(PointerKind::Up, 0), None), up);
    assert_eq!(input_of(pointer(PointerKind::Down, 0), None), None);
    assert_eq!(input_of(pointer(PointerKind::Up, 2), None), None);
    assert_eq!(
        input_of(key(true), None),
        Some(Input::Key { key: "a".into() })
    );
    assert_eq!(input_of(key(false), None), None);
    let activate = AppEvent::Activate { id: "retry".into() };
    assert_eq!(
        input_of(activate, None),
        Some(Input::Activate { id: "retry".into() })
    );
    assert_eq!(frame_input(16), Input::Frame { dt_ms: 16.0 });
    assert_eq!(input_of(AppEvent::Init, None), None);
    assert_eq!(input_of(AppEvent::Focus { focused: true }, None), None);
}

#[test]
fn the_fallback_request_is_a_get_with_the_cores_url_and_headers_and_no_body() {
    let get = HttpGet {
        url: "https://synchronized.work/v1/projects/p/stream?cursor=3&max_ms=25000".into(),
        headers: vec![("Accept".into(), "application/cbor".into())],
    };
    let request = http_request_of(get.clone()).unwrap();
    assert_eq!(request.method, HttpMethod::Get);
    assert_eq!((request.url, request.headers), (get.url, get.headers));
    assert!(request.body.0.is_empty());
}

#[test]
fn a_websocket_header_never_reaches_the_host_whatever_the_core_asks() {
    for name in ["Upgrade", "connection", "Sec-WebSocket-Key"] {
        let get = HttpGet {
            url: "https://synchronized.work/".into(),
            headers: vec![(name.into(), "x".into())],
        };
        assert_eq!(
            http_request_of(get),
            Err(PlanError::ForbiddenHeader(name.into()))
        );
    }
}

fn chunk(bytes: &[u8], end: bool) -> StreamChunk {
    StreamChunk {
        bytes: Bytes(bytes.to_vec()),
        end,
    }
}

#[test]
fn the_stream_is_read_to_its_end_and_refused_past_the_bound() {
    let mut body = Vec::new();
    assert_eq!(
        read_step(&mut body, chunk(b"ab", false), 4),
        StreamStep::More
    );
    assert_eq!(read_step(&mut body, chunk(b"cd", true), 4), StreamStep::End);
    assert_eq!(body, b"abcd");
    let mut body = b"abc".to_vec();
    assert_eq!(
        read_step(&mut body, chunk(b"de", false), 4),
        StreamStep::TooLarge
    );
    assert_eq!(body, b"abc");
}
