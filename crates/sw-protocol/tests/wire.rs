#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Canonical CBOR, schema strictness and payload limits.

use ciborium::value::Value;
use sw_protocol::WireError;
use sw_protocol::cbor::{decode, encode};
use sw_protocol::payload::*;
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::{Key, Wire};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn non_canonical_cbor_is_refused() {
    let cases: &[(&[u8], WireError)] = &[
        (&[0x18, 0x01], WireError::NonCanonical), // 1 in long form
        (&[0xa2, 0x02, 0x00, 0x01, 0x00], WireError::NonCanonical), // keys 2, 1 unsorted
        (&[0xa2, 0x01, 0x00, 0x01, 0x00], WireError::DuplicateKey),
        (&[0x9f, 0x01, 0xff], WireError::NonCanonical), // indefinite array
        (&[0x01, 0x01], WireError::NonCanonical),       // trailing byte
        (&[0xf9, 0x3c, 0x00], WireError::ForbiddenType), // half float 1.0
        (&[0xf6], WireError::ForbiddenType),            // null
        (&[0xc1, 0x01], WireError::ForbiddenType),      // tag 1
    ];
    for (bytes, expect) in cases {
        assert_eq!(decode(bytes).as_ref(), Err(expect), "{bytes:02x?}");
    }
    assert!(decode(&[0xa2, 0x01, 0x00, 0x02, 0x00]).is_ok());
}

fn hello() -> Hello {
    Hello {
        protocol_version: 1,
        endpoint_kind: HelloKind::Viewer,
        client_build: "b".into(),
    }
}

fn frame(payload: Value) -> Vec<u8> {
    encode(&Value::Map(vec![(1.into(), 1.into()), (2.into(), payload)])).unwrap()
}

#[test]
fn unknown_and_missing_keys_are_refused() {
    let Value::Map(mut entries) = hello().to_value() else {
        panic!()
    };
    entries.push(("extra".into(), 0.into()));
    let got = SwMessage::from_bytes(&frame(Value::Map(entries)));
    assert_eq!(got, Err(WireError::UnknownKey(Key::Text("extra".into()))));
    let Value::Map(mut entries) = hello().to_value() else {
        panic!()
    };
    entries.retain(|(k, _)| k != &Value::Text("client_build".into()));
    let got = SwMessage::from_bytes(&frame(Value::Map(entries)));
    assert_eq!(
        got,
        Err(WireError::MissingKey(Key::Text("client_build".into())))
    );
    let ok = SwMessage::Unsigned(Payload::Hello(hello()))
        .to_bytes()
        .unwrap();
    assert_eq!(SwMessage::from_bytes(&ok).unwrap().to_bytes().unwrap(), ok);
}

#[test]
fn unknown_type_and_both_slots_are_refused() {
    let bytes = encode(&Value::Map(vec![
        (1.into(), 25.into()),
        (2.into(), hello().to_value()),
    ]))
    .unwrap();
    assert_eq!(
        SwMessage::from_bytes(&bytes),
        Err(WireError::UnknownCode("typ"))
    );
    let both = encode(&Value::Map(vec![
        (1.into(), 1.into()),
        (2.into(), hello().to_value()),
        (3.into(), hello().to_value()),
    ]))
    .unwrap();
    assert!(SwMessage::from_bytes(&both).is_err());
    let bad_enum = Hello {
        endpoint_kind: HelloKind::Device,
        ..hello()
    }
    .to_value();
    let Value::Map(mut e) = bad_enum else {
        panic!()
    };
    for (k, v) in &mut e {
        if k == &Value::Text("endpoint_kind".into()) {
            *v = "robot".into();
        }
    }
    assert_eq!(
        SwMessage::from_bytes(&frame(Value::Map(e))),
        Err(WireError::UnknownCode("HelloKind"))
    );
}

#[test]
fn payload_limits_are_refused_at_decode() {
    let item = || ProgressItem {
        at: 1,
        kind: "k".into(),
        body: "b".into(),
    };
    let too_many = ProgressEvent {
        agent_id: "a".into(),
        events: (0..65).map(|_| item()).collect(),
    };
    assert_eq!(
        Payload::from_value(3, too_many.to_value()),
        Err(WireError::Length("payload limit"))
    );
    let long_note = JoinRequest {
        task_id: "t".into(),
        note: "x".repeat(281),
        readiness: sw_model::readiness::Readiness::READY,
        agent_id: None,
        invite: None,
    };
    assert_eq!(
        Payload::from_value(8, long_note.to_value()),
        Err(WireError::Length("payload limit"))
    );
    let short_code = Link {
        op: LinkOp::Code,
        code: "ABC".into(),
        acting_username: None,
        install_id: None,
        node_id: None,
    };
    assert!(Payload::from_value(16, short_code.to_value()).is_err());
    let not_a_map = Value::Map(vec![
        ("op".into(), "create".into()),
        ("task_id".into(), "t".into()),
        ("fields".into(), 1.into()),
    ]);
    assert_eq!(
        Payload::from_value(10, not_a_map),
        Err(WireError::Type("map"))
    );
}

#[test]
fn enrol_and_revoke_carry_exactly_their_op_fields() {
    let revoke_with_keys = Enrol {
        op: EnrolOp::Revoke,
        epoch: 2,
        endpoint_cid: Some(1),
        kind: None,
        pubkeys: None,
        kid: Some([0; 32]),
        assertion: None,
    };
    assert!(Payload::from_value(11, revoke_with_keys.to_value()).is_err());
    let device_without_seal = Enrol {
        op: EnrolOp::Enrol,
        epoch: 1,
        endpoint_cid: Some(1),
        kind: Some(sw_model::EndpointKind::Device),
        pubkeys: Some(PubKeys {
            sign: sw_protocol::wire::Bytes(vec![0; 1952]),
            seal: None,
        }),
        kid: None,
        assertion: None,
    };
    assert!(Payload::from_value(11, device_without_seal.to_value()).is_err());
    let with_seal = |len: usize| Enrol {
        pubkeys: Some(PubKeys {
            sign: sw_protocol::wire::Bytes(vec![0; 1952]),
            seal: Some(sw_protocol::wire::Bytes(vec![0; len])),
        }),
        ..device_without_seal.clone()
    };
    assert!(
        Payload::from_value(11, with_seal(1568).to_value()).is_ok(),
        "control: an ML-KEM-1024 key"
    );
    assert!(
        Payload::from_value(11, with_seal(1184).to_value()).is_err(),
        "an ML-KEM-768 key is refused"
    );
}
