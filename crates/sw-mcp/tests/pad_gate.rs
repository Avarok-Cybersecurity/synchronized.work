#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The receiving daemon's checks before it wakes anyone, with the node played by a stand-in
//! that relays whatever the test hands it (as a compromised Worker could): a forged note, a
//! replay, a note whose proof the pinned issuer never signed, and a note for someone else are
//! all dropped with no event; the untouched note is the control and wakes exactly once.

mod support;

use citadel_identity::{MlDsa65Pk, SignInAssertion};
use data_encoding::BASE64URL_NOPAD;
use serde_json::json;
use support::pad_world::{ask, home_for, issuer, person};
use sw_crypto::{KemSecretKey, SigningKey, kid_of};
use sw_mcp::daemon::{Daemon, now_ms};
use sw_mcp::ipc::Request;
use sw_model::{EndpointKind, IdFp};
use sw_protocol::body::{Audience, Body};
use sw_protocol::pad::{Recipient, seal_text};
use sw_protocol::payload::*;
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Bytes;
use sw_testkit::LabelledEntropy;

const SENDER_CID: u64 = 5555;

struct Alice {
    id: IdFp,
    sign: SigningKey,
    proof: Vec<u8>,
}

fn alice(issuer_key: &citadel_identity::ServerKey) -> Alice {
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

fn signed(iss: &IdFp, key: &SigningKey, payload: Payload, nonce: u8) -> Vec<u8> {
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

fn note(a: &Alice, to: &IdFp, cid: u64, ek: &[u8], nonce: u8) -> Vec<u8> {
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

async fn quiet(home: &sw_mcp::home::Home, after: u64) {
    let v = ask(
        home,
        Request::PadWait {
            after,
            max_wait_s: 3,
        },
        3,
    )
    .await
    .unwrap();
    assert_eq!(v["timeout"], json!(true), "nobody may be woken: {v}");
}

#[tokio::test(flavor = "multi_thread")]
async fn only_a_proven_note_for_this_person_wakes_anyone_and_only_once() {
    let mut node = support::node::start();
    let (bob_identity, bob) = person(0xB2);
    let home = home_for("gate", &bob_identity).await;
    let keys = sw_mcp::store::load_keys(&home).unwrap();
    let daemon = Daemon::open(home.clone(), support::device::config(node.ws_port)).unwrap();
    tokio::spawn(daemon.run());
    let _hello = node.next(60).await;
    let enrol = support::device::signed(&node.next(30).await).1;
    let cid = enrol.from;
    let ek = keys.kem.encapsulation_key().to_vec();
    let a = alice(&issuer());

    let good = note(&a, &bob, cid, &ek, 1);
    node.to_device.send(good.clone()).unwrap();
    let woken = ask(
        &home,
        Request::PadWait {
            after: 0,
            max_wait_s: 20,
        },
        20,
    )
    .await
    .unwrap();
    assert_eq!(
        woken["kind"],
        json!("note"),
        "control: a proven note wakes: {woken}"
    );
    let cursor = woken["cursor"].as_u64().unwrap();

    let mut forged = note(&a, &bob, cid, &ek, 2);
    let at = forged.len() - 20;
    forged[at] ^= 0x01;
    node.to_device.send(forged).unwrap();
    quiet(&home, cursor).await;

    node.to_device.send(good).unwrap();
    quiet(&home, cursor).await;

    let other_issuer = citadel_identity::ServerKey::from_seed(&[0x33; 32]);
    let unproven = Alice {
        proof: alice(&other_issuer).proof,
        ..alice(&issuer())
    };
    node.to_device
        .send(note(&unproven, &bob, cid, &ek, 3))
        .unwrap();
    quiet(&home, cursor).await;

    let (_, carol) = person(0xC3);
    node.to_device.send(note(&a, &carol, cid, &ek, 4)).unwrap();
    quiet(&home, cursor).await;

    node.to_device.send(note(&a, &bob, cid, &ek, 5)).unwrap();
    let again = ask(
        &home,
        Request::PadWait {
            after: cursor,
            max_wait_s: 20,
        },
        20,
    )
    .await
    .unwrap();
    assert_eq!(
        again["kind"],
        json!("note"),
        "control: the daemon still wakes for a new proven note"
    );
}
