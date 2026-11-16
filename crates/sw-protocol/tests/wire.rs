#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Canonical CBOR, schema strictness, sealing and the P2P envelope.

use sw_model::Id128;
use sw_protocol::WireError;
use sw_protocol::cbor::decode;
use sw_protocol::p2p::{decode_p2p, encode_p2p};
use sw_protocol::payload::*;
use sw_protocol::sealed::{SealError, Sealed};
use sw_protocol::signed::SwMessage;
use sw_testkit::{Cast, LabelledEntropy};

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
        assert_eq!(decode(bytes), Err(*expect), "{bytes:02x?}");
    }
    assert!(decode(&[0xa2, 0x01, 0x00, 0x02, 0x00]).is_ok());
}

fn hello_bytes() -> Vec<u8> {
    let hello = Hello {
        protocol_version: 1,
        endpoint_kind: sw_model::EndpointKind::Device,
        client_build: "b".into(),
    };
    SwMessage::Unsigned(Payload::Hello(hello))
        .to_bytes()
        .unwrap()
}

#[test]
fn unknown_and_missing_keys_are_refused() {
    // {0: 1, 1: 1, 2: {0: 1, 1: 0, 2: "b", 9: 0}}
    let unknown = [
        0xa3, 0x00, 0x01, 0x01, 0x01, 0x02, 0xa4, 0x00, 0x01, 0x01, 0x00, 0x02, 0x61, b'b', 0x09,
        0x00,
    ];
    assert_eq!(
        SwMessage::from_bytes(&unknown),
        Err(WireError::UnknownKey(9))
    );
    let missing = [
        0xa3, 0x00, 0x01, 0x01, 0x01, 0x02, 0xa2, 0x00, 0x01, 0x01, 0x00,
    ];
    assert_eq!(
        SwMessage::from_bytes(&missing),
        Err(WireError::MissingKey(2))
    );
    let ok = hello_bytes();
    assert_eq!(SwMessage::from_bytes(&ok).unwrap().to_bytes().unwrap(), ok);
}

#[test]
fn unknown_type_and_version_are_refused() {
    let mut bytes = hello_bytes();
    bytes[4] = 0x12; // typ 18
    assert_eq!(
        SwMessage::from_bytes(&bytes),
        Err(WireError::UnknownCode("typ"))
    );
    let mut bytes = hello_bytes();
    bytes[2] = 0x02; // version 2
    assert_eq!(
        SwMessage::from_bytes(&bytes),
        Err(WireError::Range("wire version"))
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
        agent_id: Id128([1; 16]),
        events: (0..65).map(|_| item()).collect(),
    };
    let v = sw_protocol::wire::Wire::to_value(&too_many);
    assert_eq!(
        Payload::from_value(3, v),
        Err(WireError::Length("payload limit"))
    );
    let long_note = JoinRequest {
        task_id: Id128([1; 16]),
        note: "x".repeat(281),
        readiness: sw_model::readiness::Readiness::READY,
        agent_id: None,
    };
    let v = sw_protocol::wire::Wire::to_value(&long_note);
    assert_eq!(
        Payload::from_value(8, v),
        Err(WireError::Length("payload limit"))
    );
}

fn sealed(cast: &Cast) -> Sealed {
    let inner = PromptInner {
        task_id: Id128([2; 16]),
        agent_id: None,
        text: "secret".into(),
        mode: PromptMode::Interrupt,
        reply_to: None,
    };
    let mut e = LabelledEntropy::new("seal-test");
    Sealed::seal(
        cast.alice_device_kem.encapsulation_key(),
        cast.alice_device_cid,
        &inner,
        &mut e,
    )
    .unwrap()
}

#[test]
fn sealed_opens_only_for_its_device_and_only_untampered() {
    let cast = Cast::new();
    let s = sealed(&cast);
    let (aud, inner) = s.open::<PromptInner>(&cast.alice_device_kem).unwrap();
    assert_eq!(
        (aud, inner.text.as_str()),
        (cast.alice_device_cid, "secret")
    );
    let other = sw_crypto::KemSecretKey::from_seed(&[0x77; 64]);
    assert_eq!(
        s.open::<PromptInner>(&other).unwrap_err(),
        SealError::WrongRecipient
    );
    for field in 0..3 {
        let mut t = s.clone();
        match field {
            0 => t.enc.0[0] ^= 1,
            1 => t.n[0] ^= 1,
            _ => t.ct.0[0] ^= 1,
        }
        assert_eq!(
            t.open::<PromptInner>(&cast.alice_device_kem).unwrap_err(),
            SealError::OpenFailed,
            "field {field}"
        );
    }
    let mut wrong_alg = s.clone();
    wrong_alg.kem = "X25519".into();
    assert_eq!(
        wrong_alg
            .open::<PromptInner>(&cast.alice_device_kem)
            .unwrap_err(),
        SealError::UnknownAlg
    );
}

#[test]
fn the_ciphertext_does_not_contain_the_prompt() {
    let cast = Cast::new();
    let s = sealed(&cast);
    assert!(!s.enc.0.windows(6).any(|w| w == b"secret"));
}

#[test]
fn p2p_envelope_is_exactly_type_and_payload() {
    let inner = hello_bytes();
    let env = encode_p2p(&inner).unwrap();
    assert_eq!(decode_p2p(&env).unwrap(), inner);
    let wrong_tag = sw_protocol::cbor::encode(&ciborium::value::Value::Map(vec![
        ("type".into(), "sw/2".into()),
        (
            "payload".into(),
            ciborium::value::Value::Bytes(inner.clone()),
        ),
    ]))
    .unwrap();
    assert_eq!(decode_p2p(&wrong_tag), Err(WireError::Range("p2p type")));
    let extra = sw_protocol::cbor::encode(&ciborium::value::Value::Map(vec![
        ("type".into(), "sw/1".into()),
        ("payload".into(), ciborium::value::Value::Bytes(inner)),
        ("x".into(), 1.into()),
    ]))
    .unwrap();
    assert!(decode_p2p(&extra).is_err());
}
