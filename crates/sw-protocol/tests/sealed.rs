#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Sealing to one Device, and the P2P envelope.

use ciborium::value::Value;
use sw_protocol::WireError;
use sw_protocol::cbor::encode;
use sw_protocol::p2p::{decode_p2p, encode_p2p};
use sw_protocol::payload::*;
use sw_protocol::sealed::{SealError, Sealed};
use sw_protocol::signed::SwMessage;
use sw_testkit::{Cast, LabelledEntropy};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn hello() -> Hello {
    Hello {
        protocol_version: 1,
        endpoint_kind: HelloKind::Viewer,
        client_build: "b".into(),
    }
}

fn sealed(cast: &Cast) -> Sealed {
    let inner = PromptInner {
        aud: cast.alice_device_cid,
        task_id: "t".into(),
        text: "secret".into(),
        mode: PromptMode::Interrupt,
        agent_id: None,
        reply_to: None,
    };
    let mut e = LabelledEntropy::new("seal-test");
    Sealed::seal(cast.alice_device_kem.encapsulation_key(), &inner, &mut e).unwrap()
}

#[test]
fn sealed_opens_only_for_its_device_and_only_untampered() {
    let cast = Cast::new();
    let s = sealed(&cast);
    let inner: PromptInner = s.open(&cast.alice_device_kem).unwrap();
    assert_eq!(
        (inner.aud, inner.text.as_str()),
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
    assert!(
        !s.enc.0.windows(6).any(|w| w == b"secret"),
        "the ciphertext hides the prompt"
    );
}

#[test]
fn p2p_envelope_is_exactly_type_and_payload() {
    let inner = SwMessage::Unsigned(Payload::Hello(hello()))
        .to_bytes()
        .unwrap();
    let env = encode_p2p(&inner).unwrap();
    assert_eq!(decode_p2p(&env).unwrap(), inner);
    let with = |tag: &str, extra: bool| {
        let mut m = vec![
            ("type".into(), tag.into()),
            ("payload".into(), Value::Bytes(inner.clone())),
        ];
        if extra {
            m.push(("x".into(), 1.into()));
        }
        encode(&Value::Map(m)).unwrap()
    };
    assert_eq!(
        decode_p2p(&with("sw/2", false)),
        Err(WireError::Range("p2p type"))
    );
    assert!(decode_p2p(&with("sw/1", true)).is_err());
}

#[test]
fn version_one_envelopes_are_refused_by_name() {
    let cast = Cast::new();
    let s = sealed(&cast);
    assert_eq!(s.kem, "ML-KEM-1024");
    assert_eq!(s.ct.0.len(), 1568);
    assert!(s.check_shape().is_ok(), "control: version 2 has its shape");
    let mut v1 = s.clone();
    v1.kem = "ML-KEM-768".into();
    assert_eq!(v1.check_shape(), Err(SealError::Retired));
    assert_eq!(
        v1.open::<PromptInner>(&cast.alice_device_kem).unwrap_err(),
        SealError::Retired
    );
    v1.ct.0.truncate(1088);
    assert_eq!(
        v1.check_shape(),
        Err(SealError::Retired),
        "named before its size"
    );

    let mut short = s.clone();
    short.ct.0.truncate(1088);
    assert_eq!(short.check_shape(), Err(SealError::Malformed));
}
