//! A scratchpad sender outside any daemon: Alice's Device, her ENROL-by-assertion proof and
//! notes she signs, for tests that hand relayed bytes straight to a Device.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use citadel_identity::{MlDsa65Pk, SignInAssertion};
use data_encoding::BASE64URL_NOPAD;
use sw_crypto::{KemSecretKey, SigningKey, kid_of};
use sw_mcp::daemon::now_ms;
use sw_model::{EndpointKind, IdFp};
use sw_protocol::body::{Audience, Body};
use sw_protocol::pad::{Recipient, seal_text};
use sw_protocol::payload::*;
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Bytes;
use sw_testkit::LabelledEntropy;

use super::pad_world::person;

pub const SENDER_CID: u64 = 5555;

pub struct Alice {
    pub id: IdFp,
    pub sign: SigningKey,
    pub proof: Vec<u8>,
}

pub fn alice(issuer_key: &citadel_identity::ServerKey) -> Alice {
    let (identity, id) = person(0xA1);
    let sign = SigningKey::from_seed(&[0x61; 32]);
    let kem = KemSecretKey::from_seed(&[0x62; 64]);
    let assertion = SignInAssertion {
        identity: MlDsa65Pk::from_bytes(identity.public_key().as_bytes()).unwrap(),
        device: format!(
            "laptop#{}",
            BASE64URL_NOPAD.encode(&kid_of(sign.public_key()))
        ),
        aud: "sw-mcp".into(),
        exp_s: now_ms() / 1000 + 3600,
        nonce: [9; 32],
    };
    let enrol = Payload::Enrol(Enrol {
        op: EnrolOp::Enrol,
        epoch: 1,
        endpoint_cid: Some(SENDER_CID),
        kind: Some(EndpointKind::Device),
        pubkeys: Some(PubKeys {
            sign: Bytes(sign.public_key().to_vec()),
            seal: Some(Bytes(kem.encapsulation_key().to_vec())),
        }),
        kid: None,
        assertion: Some(Bytes(issuer_key.sign_assertion(&assertion).unwrap())),
    });
    let proof = signed(&id, &sign, enrol, 0);
    Alice { id, sign, proof }
}

pub fn signed(iss: &IdFp, key: &SigningKey, payload: Payload, nonce: u8) -> Vec<u8> {
    let now = now_ms();
    let body = Body {
        iss: iss.clone(),
        from: SENDER_CID,
        aud: Audience::Node,
        iat: now,
        exp: now + 3_600_000,
        nonce: [nonce; 16],
        seq: now + u64::from(nonce),
        payload,
    };
    SwMessage::signed(&body, key).unwrap().to_bytes().unwrap()
}

pub fn note(a: &Alice, to: &IdFp, cid: u64, ek: &[u8], nonce: u8) -> Vec<u8> {
    let text = PadText {
        body: "free to merge".into(),
        refs: vec![],
    };
    let body = seal_text(
        &text,
        &[Recipient { cid, seal_pk: ek }],
        &mut LabelledEntropy::new("g"),
    )
    .unwrap();
    let n = PadNote {
        project: "p".into(),
        scope: PadScope::Members,
        to: vec![to.clone()],
        body,
        proof: Bytes(a.proof.clone()),
    };
    signed(&a.id, &a.sign, Payload::PadNote(n), nonce)
}
