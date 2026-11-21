#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Joining by invitation on the wire (types 23, 24 and `JOIN_REQUEST.invite`): the code a
//! person pastes, the hash the node keeps, and which key signs each message.

use sw_model::IdFp;
use sw_protocol::WireError;
use sw_protocol::invite::{InviteCode, PREFIX, token_hash};
use sw_protocol::payload::invite::invite_limits;
use sw_protocol::payload::*;
use sw_protocol::signed::SwMessage;

fn owner() -> IdFp {
    IdFp::parse(&"a".repeat(IdFp::LEN)).unwrap()
}

fn code() -> InviteCode {
    InviteCode {
        project: "0123456789abcdef0123456789abcdef".into(),
        owner: owner(),
        name: "Acme refactor".into(),
        exp: 1_800_000_000_000,
        token: [7; 32],
    }
}

#[test]
fn an_invite_code_round_trips_and_ignores_surrounding_whitespace() {
    let text = code().encode().unwrap();
    assert!(text.starts_with(PREFIX));
    assert_eq!(InviteCode::decode(&format!("  {text}\n")).unwrap(), code());
}

#[test]
fn a_code_without_the_prefix_or_with_a_changed_byte_is_refused() {
    let text = code().encode().unwrap();
    let bare = text.strip_prefix(PREFIX).unwrap();
    assert!(InviteCode::decode(bare).is_err());
    // Control: the same body with its prefix decodes (above); a truncated one does not.
    assert!(InviteCode::decode(&text[..text.len() - 3]).is_err());
    let long = InviteCode {
        name: "x".repeat(201),
        ..code()
    };
    let encoded = long.encode().unwrap();
    assert_eq!(
        InviteCode::decode(&encoded),
        Err(WireError::Length("InviteCode project or name"))
    );
}

#[test]
fn the_node_keeps_a_domain_separated_hash_never_the_token() {
    let token = [7u8; 32];
    let hash = token_hash(&token);
    assert_ne!(hash, token);
    assert_ne!(hash, sw_crypto::sha3_256(&token), "domain separation");
    assert_eq!(hash, code().token_hash());
    assert_ne!(hash, token_hash(&[8u8; 32]));
}

#[test]
fn an_invite_and_a_join_answer_are_signed_by_any_endpoint_and_join_pending_by_none() {
    let invite = Payload::Invite(Invite {
        project: "p".into(),
        token_hash: [1; 32],
        exp: 1,
    });
    assert_eq!(invite.typ(), 23);
    assert_eq!(invite.signer(), Some(SignerRole::Endpoint));
    let answer = Payload::JoinAnswer(JoinAnswer {
        request_id: [2; 32],
        accept: true,
        reason: None,
    });
    assert_eq!(answer.signer(), Some(SignerRole::Endpoint));
    let ask = Payload::JoinPending(JoinPending {
        project: None,
        requests: None,
    });
    assert_eq!(ask.typ(), 24);
    assert_eq!(ask.signer(), None);
    let bytes = SwMessage::Unsigned(ask.clone()).to_bytes().unwrap();
    assert_eq!(
        SwMessage::from_bytes(&bytes).unwrap(),
        SwMessage::Unsigned(ask)
    );
    // Control: the gate refuses a signed type in the unsigned slot.
    let unsigned_invite = SwMessage::Unsigned(invite).to_bytes().unwrap();
    let decoded = SwMessage::from_bytes(&unsigned_invite).unwrap();
    assert!(sw_protocol::gate::accept_unsigned(decoded).is_err());
}

#[test]
fn a_join_pending_answer_is_bounded() {
    let one = PendingJoin {
        request_id: [3; 32],
        project: "p".into(),
        task_id: "p".into(),
        person: owner(),
        display: "bob@example".into(),
        at: 5,
    };
    let at_cap = JoinPending {
        project: Some("p".into()),
        requests: Some(vec![one.clone(); invite_limits::PENDING_LISTED]),
    };
    let ok = SwMessage::Unsigned(Payload::JoinPending(at_cap))
        .to_bytes()
        .unwrap();
    assert!(SwMessage::from_bytes(&ok).is_ok());
    let over = JoinPending {
        project: Some("p".into()),
        requests: Some(vec![one; invite_limits::PENDING_LISTED + 1]),
    };
    let bytes = SwMessage::Unsigned(Payload::JoinPending(over))
        .to_bytes()
        .unwrap();
    assert_eq!(
        SwMessage::from_bytes(&bytes),
        Err(WireError::Length("JOIN_PENDING requests"))
    );
}

#[test]
fn a_join_request_without_an_invite_encodes_as_before() {
    let request = JoinRequest {
        task_id: "t".into(),
        note: "n".into(),
        readiness: sw_model::readiness::Readiness::from_permille(1000).unwrap(),
        agent_id: None,
        invite: None,
    };
    let value = sw_protocol::wire::Wire::to_value(&request);
    let ciborium::value::Value::Map(entries) = value else {
        panic!("a map")
    };
    assert!(entries.iter().all(|(k, _)| k.as_text() != Some("invite")));
    let with = JoinRequest {
        invite: Some([9; 32]),
        ..request
    };
    let ciborium::value::Value::Map(entries) = sw_protocol::wire::Wire::to_value(&with) else {
        panic!("a map")
    };
    assert!(entries.iter().any(|(k, _)| k.as_text() == Some("invite")));
}
