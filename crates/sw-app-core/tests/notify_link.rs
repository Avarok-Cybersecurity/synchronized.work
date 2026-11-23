#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! NOTIFY to host notifications, and the link code's wire and record.

mod support;

use support::{NODE, ctx, granted};
use sw_app_core::link::{CodeEntry, LinkOutcome, LinkRecord, Refused, outcome, redeem};
use sw_app_core::notify::{HostNotify, NotifyMap};
use sw_app_core::*;
use sw_graph::Motion;
use sw_protocol::payload::{Link, LinkOp, Notify, PadReceipt, Payload};
use sw_protocol::signed::SwMessage;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn n(id: u8) -> Notify {
    Notify {
        event_id: [id; 32],
        kind: "person_joined_task".into(),
        subject: "Ship it".into(),
        text: "bob joined Ship it".into(),
        at: 1,
    }
}

#[test]
fn a_notify_is_one_host_notification_targeting_the_apps_node() {
    let mut map = NotifyMap::new(8);
    assert_eq!(
        map.map(&n(1), NODE),
        Some(HostNotify {
            event_id: [1; 32],
            title: "Ship it".into(),
            body: "bob joined Ship it".into(),
            node_id: NODE.into(),
        })
    );
}

#[test]
fn a_repeated_event_id_is_shown_once() {
    let mut map = NotifyMap::new(8);
    assert!(map.map(&n(1), NODE).is_some());
    assert!(map.map(&n(1), NODE).is_none());
    assert!(map.map(&n(2), NODE).is_some());
}

#[test]
fn the_oldest_event_id_is_forgotten_past_the_memory() {
    let mut map = NotifyMap::new(2);
    for id in 1..=3 {
        assert!(map.map(&n(id), NODE).is_some());
    }
    assert!(map.map(&n(3), NODE).is_none());
    assert!(map.map(&n(1), NODE).is_some(), "id 1 was evicted by 3");
}

#[test]
fn the_code_entry_takes_eight_letters_or_digits() {
    let mut e = CodeEntry::new();
    for key in ["a", "B", "3", "Shift", " ", "é", "!"] {
        e.key(key);
    }
    assert_eq!(e.code(), "aB3");
    for key in ["4", "5", "6", "7", "8", "9"] {
        e.key(key);
    }
    assert_eq!(e.code(), "aB345678");
    assert!(e.is_complete());
    assert!(!e.key("0"), "a ninth character is refused");
    assert!(e.key("Backspace"));
    assert_eq!(e.code(), "aB34567");
}

#[test]
fn the_redeem_carries_the_code_and_the_three_context_fields_only() {
    let c = ctx(granted(true, true), CanvasAbi::Cw5, Motion::Lite);
    let (bytes, id) = redeem("ABCD1234", &c).unwrap();
    assert_eq!(id, sw_crypto::sha3_256(&bytes));
    let SwMessage::Unsigned(Payload::Link(link)) = SwMessage::from_bytes(&bytes).unwrap() else {
        panic!("not an unsigned LINK")
    };
    assert_eq!(
        link,
        Link {
            op: LinkOp::Redeem,
            code: "ABCD1234".into(),
            acting_username: Some("alice".into()),
            install_id: Some("inst-0123456789".into()),
            node_id: Some(NODE.into()),
        }
    );
}

#[test]
fn only_the_receipt_for_this_redeem_settles_it() {
    let mine = [7; 32];
    let r = |id, typ, stored, project: &str| PadReceipt {
        project: project.into(),
        id,
        typ,
        stored,
        reason: (!stored).then(|| "InviteInvalid".into()),
    };
    assert_eq!(outcome(&r([8; 32], 16, true, "p1"), &mine), None);
    assert_eq!(outcome(&r(mine, 10, true, "p1"), &mine), None);
    assert_eq!(
        outcome(&r(mine, 16, true, "p1"), &mine),
        Some(LinkOutcome::Linked {
            project: Some("p1".into())
        })
    );
    assert_eq!(
        outcome(&r(mine, 16, true, ""), &mine),
        Some(LinkOutcome::Linked { project: None })
    );
    assert_eq!(
        outcome(&r(mine, 16, false, ""), &mine),
        Some(LinkOutcome::Refused(Refused {
            reason: Some("InviteInvalid".into())
        }))
    );
}

#[test]
fn the_link_record_round_trips_and_refuses_another_version() {
    for project in [Some("p1".to_owned()), None] {
        let record = LinkRecord { project };
        assert_eq!(
            LinkRecord::from_bytes(&record.to_bytes().unwrap()).unwrap(),
            record
        );
    }
    let mut map = sw_protocol::wire::MapOut::default();
    map.field("v", &2u64);
    let v2 = sw_protocol::cbor::encode(&map.build()).unwrap();
    assert!(LinkRecord::from_bytes(&v2).is_err());
}
