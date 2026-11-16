#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! `TaskFields` and `EventBody` (sw-wire §3 named types).

use ciborium::value::Value;
use sw_model::IdFp;
use sw_model::tier::{JoinPolicy, TierName};
use sw_protocol::WireError;
use sw_protocol::payload::*;
use sw_protocol::wire::{Raw, Wire};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn write(op: TaskOp, fields: TaskFields) -> Result<Payload, WireError> {
    Payload::from_value(
        10,
        TaskWrite {
            op,
            task_id: "t".into(),
            fields,
        }
        .to_value(),
    )
}

#[test]
fn task_fields_are_typed_and_bounded() {
    let full = TaskFields {
        title: Some("Ship".into()),
        description: Some("d".into()),
        parent: Some("p".into()),
        tier: Some(TierName::Synchronized),
        visibility: Some(sw_model::Visibility::Summary),
        status: Some(sw_model::Status::Open),
        owner: Some(IdFp::parse("3foriftckvpoy2rh").unwrap()),
        join_policy: Some(JoinPolicy::Auto { max_coworkers: 64 }),
        embedding: Some(vec![65535; 256]),
    };
    assert!(write(TaskOp::Create, full.clone()).is_ok());
    assert!(write(TaskOp::Delete, TaskFields::EMPTY).is_ok());
    assert!(
        write(
            TaskOp::Update,
            TaskFields {
                join_policy: Some(JoinPolicy::Manual),
                ..TaskFields::EMPTY
            }
        )
        .is_ok()
    );
    let shape = Err(WireError::Shape("TASK_WRITE fields for its op"));
    assert_eq!(
        write(
            TaskOp::Create,
            TaskFields {
                title: None,
                ..full.clone()
            }
        ),
        shape
    );
    assert_eq!(write(TaskOp::Delete, full.clone()), shape);
    let length = Err(WireError::Length("TaskFields limit"));
    assert_eq!(
        write(
            TaskOp::Create,
            TaskFields {
                title: Some("x".repeat(201)),
                ..full.clone()
            }
        ),
        length
    );
    assert_eq!(
        write(
            TaskOp::Create,
            TaskFields {
                description: Some("x".repeat(4097)),
                ..full.clone()
            }
        ),
        length
    );
    assert_eq!(
        write(
            TaskOp::Create,
            TaskFields {
                embedding: Some(vec![0; 255]),
                ..full.clone()
            }
        ),
        length
    );
    for max in [0, 65] {
        let p = TaskFields {
            join_policy: Some(JoinPolicy::Auto { max_coworkers: max }),
            ..full.clone()
        };
        assert_eq!(
            write(TaskOp::Create, p),
            Err(WireError::Range("max_coworkers")),
            "{max}"
        );
    }
    let manual_with_max = Value::Map(vec![
        ("mode".into(), "manual".into()),
        ("max_coworkers".into(), 2.into()),
    ]);
    assert!(JoinPolicy::from_value(manual_with_max).is_err());
}

#[test]
fn event_body_names_a_projected_type_and_carries_a_map() {
    let body = |typ, payload| EventBody {
        typ,
        from: 1,
        iss: IdFp::parse("3foriftckvpoy2rh").unwrap(),
        payload: Raw(payload),
    };
    let event = |b| Event {
        project_id: "p".into(),
        cursor: 1,
        kind: "k".into(),
        body: b,
    };
    for typ in EventBody::TYPES {
        assert!(Payload::from_value(13, event(body(typ, Value::Map(vec![]))).to_value()).is_ok());
    }
    assert_eq!(
        Payload::from_value(13, event(body(4, Value::Map(vec![]))).to_value()),
        Err(WireError::UnknownCode("EventBody.typ"))
    );
    assert_eq!(
        Payload::from_value(13, event(body(2, 1.into())).to_value()),
        Err(WireError::Type("map"))
    );
}
