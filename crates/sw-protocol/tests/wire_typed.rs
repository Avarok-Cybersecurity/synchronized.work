#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The contract's named types (sw-wire §3): `Lane`, `Blocker`, `Readiness`, and the "iff"
//! rules of `CONTROL` and `ACK`.

use ciborium::value::Value;
use sw_model::Lane;
use sw_model::readiness::{Blocker, BlockerKind, Readiness};
use sw_model::tier::TierName;
use sw_protocol::WireError;
use sw_protocol::payload::*;
use sw_protocol::wire::Wire;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn blocker(kind: BlockerKind, task_id: Option<&str>) -> Blocker {
    Blocker {
        kind,
        text: "waiting on review".into(),
        task_id: task_id.map(str::to_owned),
    }
}

fn status(lane: Lane, blockers: Vec<Blocker>) -> StatusUpdate {
    StatusUpdate {
        agent_id: "a".into(),
        task_id: "t".into(),
        lane,
        blockers,
        harness: "h".into(),
    }
}

fn with(v: Value, key: &str, new: Value) -> Value {
    let Value::Map(mut entries) = v else { panic!() };
    for (k, val) in &mut entries {
        if k == &Value::Text(key.into()) {
            *val = new.clone();
        }
    }
    Value::Map(entries)
}

#[test]
fn status_update_carries_a_typed_lane_and_blockers() {
    let s = status(
        Lane::Blocked,
        vec![
            blocker(BlockerKind::Dependency, Some("t2")),
            blocker(BlockerKind::Disk, None),
        ],
    );
    assert_eq!(
        Payload::from_value(2, s.to_value()),
        Ok(Payload::StatusUpdate(s.clone()))
    );
    assert_eq!(
        Payload::from_value(2, with(s.to_value(), "lane", "working".into())),
        Err(WireError::UnknownCode("Lane"))
    );
    let many = status(Lane::Blocked, vec![blocker(BlockerKind::Lock, None); 17]);
    assert_eq!(
        Payload::from_value(2, many.to_value()),
        Err(WireError::Length("payload limit"))
    );
    assert!(
        Payload::from_value(
            2,
            status(Lane::Blocked, vec![blocker(BlockerKind::Lock, None); 16]).to_value()
        )
        .is_ok()
    );
    for bad in [
        blocker(BlockerKind::Dependency, None),
        blocker(BlockerKind::Human, Some("t2")),
        Blocker {
            text: "x".repeat(281),
            ..blocker(BlockerKind::Lock, None)
        },
    ] {
        let s = status(Lane::Blocked, vec![bad]);
        assert_eq!(
            Payload::from_value(2, s.to_value()),
            Err(WireError::Shape("Blocker text or task_id"))
        );
    }
}

#[test]
fn readiness_is_a_permille_uint() {
    let req = |r| JoinRequest {
        task_id: "t".into(),
        note: "n".into(),
        readiness: r,
        agent_id: None,
        invite: None,
    };
    let half = Readiness::from_permille(500).unwrap();
    assert_eq!(
        req(half).to_value(),
        with(req(half).to_value(), "readiness", 500.into())
    );
    let over = with(req(half).to_value(), "readiness", 1001.into());
    assert_eq!(
        Payload::from_value(8, over),
        Err(WireError::Range("readiness"))
    );
    let text = with(req(half).to_value(), "readiness", "0.5".into());
    assert!(Payload::from_value(8, text).is_err());
}

fn control(
    op: ControlOp,
    tier: Option<TierName>,
    id: Option<[u8; 32]>,
    reason: Option<&str>,
) -> Result<ControlInner, WireError> {
    let c = ControlInner {
        aud: 1,
        op,
        tier,
        id,
        reason: reason.map(str::to_owned),
    };
    ControlInner::from_value(c.to_value())
}

#[test]
fn control_fields_follow_their_op() {
    use ControlOp::*;
    let id = Some([7; 32]);
    assert!(control(SetTier, Some(TierName::Aware), None, None).is_ok());
    assert!(control(ApproveJoin, None, id, None).is_ok());
    assert!(control(RefuseJoin, None, id, Some("full")).is_ok());
    assert!(control(Pause, None, None, None).is_ok());
    let bad = [
        control(SetTier, None, None, None),
        control(Pause, Some(TierName::Off), None, None),
        control(ApproveJoin, None, None, None),
        control(Stop, None, id, None),
        control(RefuseJoin, None, id, None),
        control(ApproveJoin, None, id, Some("why")),
        control(RefuseJoin, None, id, Some(&"x".repeat(281))),
    ];
    for got in bad {
        assert_eq!(got, Err(WireError::Shape("CONTROL fields for its op")));
    }
}

#[test]
fn ack_reason_iff_refused() {
    let ack = |result, reason: Option<&str>| {
        let a = Ack {
            signal_id: [1; 32],
            result,
            reason: reason.map(str::to_owned),
        };
        Payload::from_value(6, a.to_value())
    };
    assert!(ack(AckResult::Refused, Some("no")).is_ok());
    assert!(ack(AckResult::Delivered, None).is_ok());
    assert!(ack(AckResult::Refused, None).is_err());
    assert!(ack(AckResult::Executed, Some("x")).is_err());
}
