#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The node core answers the server identity challenge (`server-identity-challenge.md` §2):
//! the agent's frame, before or after `HELLO`, from any session; the signed cid is the
//! session's; at most one answer per interval per session; nothing stored.

mod support;

use support::challenge::*;
use support::{NOW, Who, World, rules};
use sw_model::EndpointKind;
use sw_node::core::{Effect, NodeCore, NodeRefusal};
use sw_protocol::WireError;
use sw_protocol::body::Audience;

const INTERVAL: u64 = 1000;

fn node(name: &str) -> NodeCore {
    let mut r = rules(&World::new().issuer);
    r.server_identity = Some(kat_identity(name));
    NodeCore::new(r)
}

/// The one frame sent back to `cid`.
fn answered(effects: Vec<Effect>, cid: u64) -> Vec<u8> {
    match effects.as_slice() {
        [Effect::Send { to, bytes }] if *to == cid => bytes.clone(),
        other => panic!("not one answer to {cid}: {other:?}"),
    }
}

/// Control: sign under another domain in `answer_challenge` → red.
#[test]
fn the_kat_challenge_is_answered_with_the_kat_signature() {
    let kat = kat_server();
    let mut node = node(KAT_SERVER);
    let effects = node.received(kat.cid, &frames().request, NOW).unwrap();
    let frame = answered(effects, kat.cid);
    assert_eq!(frame, frames().answer(KAT_SERVER));
    let answer = parse_answer(&frame);
    assert_eq!(answer.sig, kat.sig, "ML-DSA-65 signing is deterministic");
    assert!(verifies(&answer, KAT_SERVER, kat.cid, &kat.nonce));
    assert!(!verifies(
        &answer,
        "main-old.work.avarok.net",
        kat.cid,
        &kat.nonce
    ));
}

/// Control: pass a constant in place of `cid` to `sign_challenge` → red.
#[test]
fn the_signed_cid_is_the_sessions() {
    let kat = kat_server();
    let mut node = node("synchronized.work");
    for (cid, other) in [(7, 8), (8, 7)] {
        let frame = answered(node.received(cid, &frames().request, NOW).unwrap(), cid);
        let answer = parse_answer(&frame);
        assert!(
            verifies(&answer, "synchronized.work", cid, &kat.nonce),
            "{cid}"
        );
        assert!(!verifies(&answer, "synchronized.work", other, &kat.nonce));
    }
}

/// What the node did with each of these bytes before it knew the challenge.
/// Control: drop `deny_unknown_fields` from the mirrored `Challenge` → red.
#[test]
fn a_near_miss_is_refused_as_it_was_before() {
    let mut node = node("synchronized.work");
    for (why, frame) in frames().near_misses {
        let refused = node.received(1000, &frame, NOW);
        assert_eq!(
            refused,
            Err(NodeRefusal::Wire(WireError::Malformed)),
            "{why}"
        );
        let receipt = node.refusal_receipt(1000, &frame, &refused.unwrap_err());
        assert_eq!(receipt, Vec::new(), "{why}");
    }
}

/// Control: delete the `ChallengeTooSoon` return → red.
#[test]
fn a_session_is_answered_once_a_second() {
    let mut node = node("synchronized.work");
    let request = frames().request;
    answered(node.received(5, &request, NOW).unwrap(), 5);
    let again = node.received(5, &request, NOW + INTERVAL - 1);
    assert_eq!(again, Err(NodeRefusal::ChallengeTooSoon));
    assert!(
        node.refusal_receipt(5, &request, &NodeRefusal::ChallengeTooSoon)
            .is_empty()
    );
    answered(node.received(6, &request, NOW + 1).unwrap(), 6);
    answered(node.received(5, &request, NOW + INTERVAL).unwrap(), 5);
}

/// Control: delete `self.answered.forget(cid)` in `disconnected` → red.
#[test]
fn a_session_that_ends_is_forgotten() {
    let mut node = node("synchronized.work");
    let request = frames().request;
    answered(node.received(5, &request, NOW).unwrap(), 5);
    node.disconnected(5);
    let _ = node.connected(5, NOW + 1);
    answered(node.received(5, &request, NOW + 1).unwrap(), 5);
}

/// Control: answer `NodeRefusal::Malformed` where the key is missing → red.
#[test]
fn a_node_with_no_server_key_refuses() {
    let mut w = World::new();
    let refused = w.node.received(5, &frames().request, NOW);
    assert_eq!(refused, Err(NodeRefusal::NoServerIdentity));
}

/// Control: let a challenge fall through to the `SwMessage` decode in `received` → red.
#[test]
fn the_challenge_needs_no_hello_and_is_not_one() {
    let mut w = World::new();
    w.node = node("synchronized.work");
    let cid = w.alice.controller.cid;
    let _ = w.node.connected(cid, NOW);
    answered(w.node.received(cid, &frames().request, NOW).unwrap(), cid);
    let enrol = w.enrol_payload(
        w.person(Who::Alice),
        w.ep(Who::Alice, EndpointKind::Controller),
    );
    let before_hello = w.send_as(Who::Alice, EndpointKind::Controller, Audience::Node, enrol);
    assert_eq!(before_hello, Err(NodeRefusal::NoHello));
    let _ = support::connect(&mut w.node, cid);
    let later = NOW + INTERVAL;
    answered(w.node.received(cid, &frames().request, later).unwrap(), cid);
}
