#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The developer scratchpad at the node: who a note reaches, how offline targets catch up,
//! how acks return to the sender, and every limit. Each refusal has its control.

mod pad_support;
mod support;

use pad_support::*;
use sw_node::core::{NodeCore, NodeRefusal, PadRefusal};
use sw_protocol::gate::Refusal;
use sw_protocol::payload::*;
use sw_protocol::signed::SwMessage;
use sw_testkit::pad::issuer;

fn to(sent: &[(u64, Vec<u8>)], bytes: &[u8]) -> Vec<u64> {
    sent.iter()
        .filter(|(_, b)| b == bytes)
        .map(|(c, _)| *c)
        .collect()
}

#[test]
fn a_note_reaches_its_target_only_and_a_broadcast_reaches_everyone() {
    let mut w = Pad::standard();
    let note = w.note(1, &[2], PadScope::Members);
    let sent = w.raw(101, &note).unwrap();
    assert_eq!(
        to(&sent, &note),
        vec![102],
        "B only; C is not sent a note for B"
    );
    assert_eq!(receipts(&sent), vec![(101, true, None)]);

    let all = w.note(1, &[2, 3], PadScope::All);
    let sent = w.raw(101, &all).unwrap();
    assert_eq!(
        to(&sent, &all),
        vec![102, 103],
        "control: a broadcast reaches C"
    );
}

#[test]
fn an_offline_target_gets_the_note_on_connect_until_it_acks() {
    let mut w = Pad::standard();
    w.node.disconnected(102);
    let note = w.note(1, &[2], PadScope::Members);
    assert!(to(&w.raw(101, &note).unwrap(), &note).is_empty());
    assert_eq!(to(&w.online(102), &note), vec![102]);
    assert_eq!(to(&w.online(102), &note), vec![102], "kept until acked");

    let ack = w.ack(2, id_of(&note));
    let sent = w.raw(102, &ack).unwrap();
    assert_eq!(
        to(&sent, &ack),
        vec![101],
        "the ack reaches the live sender"
    );
    assert!(
        to(&w.online(102), &note).is_empty(),
        "acked: not sent again"
    );
}

#[test]
fn an_ack_waits_for_an_offline_sender_and_is_taken_once_per_person() {
    let mut w = Pad::standard();
    let note = w.note(1, &[2], PadScope::Members);
    w.raw(101, &note).unwrap();
    w.node.disconnected(101);
    let ack = w.ack(2, id_of(&note));
    assert!(to(&w.raw(102, &ack).unwrap(), &ack).is_empty());
    assert_eq!(
        to(&w.online(101), &ack),
        vec![101],
        "delivered on reconnect"
    );

    assert_eq!(
        w.raw(102, &ack),
        Err(NodeRefusal::Gate(Refusal::Replay)),
        "the same ack replayed"
    );
    let again = w.ack(2, id_of(&note));
    let sent = w.raw(102, &again).unwrap();
    assert_eq!(
        receipts(&sent),
        vec![(102, false, Some("AlreadyAcked".into()))]
    );
    let by_c = w.ack(3, id_of(&note));
    let sent = w.raw(103, &by_c).unwrap();
    assert_eq!(
        receipts(&sent),
        vec![(103, false, Some("NotATarget".into()))]
    );
}

#[test]
fn the_directory_answers_members_with_their_proofs() {
    let mut w = Pad::standard();
    let ask = SwMessage::Unsigned(Payload::PadDirectory(PadDirectory {
        project: "p".into(),
        members: None,
    }));
    let sent = w.raw(102, &ask.to_bytes().unwrap()).unwrap();
    let SwMessage::Unsigned(Payload::PadDirectory(d)) = SwMessage::from_bytes(&sent[0].1).unwrap()
    else {
        panic!("a directory")
    };
    let members = d.members.unwrap();
    assert_eq!(members.len(), 3);
    assert!(members.iter().all(|m| m.proofs.len() == 1));

    let refused = w.raw(104, &ask.to_bytes().unwrap());
    assert_eq!(refused, Err(NodeRefusal::Pad(PadRefusal::NotAMember)));
}

#[test]
fn a_rebuilt_node_still_delivers_pending_notes_and_acks() {
    let mut w = Pad::standard();
    let n1 = w.note(1, &[2], PadScope::Members);
    let n2 = w.note(1, &[2, 3], PadScope::Members);
    w.raw(101, &n1).unwrap();
    w.raw(101, &n2).unwrap();
    let ack = w.ack(3, id_of(&n2));
    w.raw(103, &ack).unwrap();

    let mut rebuilt = NodeCore::new(support::rules(&issuer()));
    for (key, value) in &w.stored {
        rebuilt.load(key, value).unwrap();
    }
    let to_b: Vec<_> = rebuilt.connected(102, NOW).into_iter().collect();
    let sends = |effects: &[sw_node::core::Effect], bytes: &[u8]| {
        effects
            .iter()
            .any(|e| matches!(e, sw_node::core::Effect::Send { bytes: b, .. } if b == bytes))
    };
    assert!(sends(&to_b, &n1) && sends(&to_b, &n2));
    let to_c = rebuilt.connected(103, NOW);
    assert!(!sends(&to_c, &n2), "C acked n2 before the restart");
    assert!(
        sends(&rebuilt.connected(101, NOW), &ack),
        "the sender still gets C's ack"
    );
}

#[test]
fn expired_notes_are_deleted_and_not_delivered() {
    let mut w = Pad::standard();
    w.node.disconnected(102);
    let note = w.note(1, &[2], PadScope::Members);
    w.raw(101, &note).unwrap();
    let later = w.node.connected(102, NOW + 2 * HOUR);
    let deleted = later
        .iter()
        .any(|e| matches!(e, sw_node::core::Effect::Delete { key } if key.starts_with("pad/")));
    assert!(deleted, "the expired note is deleted");
    assert!(
        !later
            .iter()
            .any(|e| matches!(e, sw_node::core::Effect::Send { .. }))
    );
}
