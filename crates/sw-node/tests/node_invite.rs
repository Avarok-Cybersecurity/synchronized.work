#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Invitations at the node: the owner's invite opens the project, a join request spends it
//! once, an expired or spent one is refused, and a restart keeps the live ones. Every refusal
//! has its control beside it.

mod invite_support;
mod support;

use invite_support::*;
use support::*;
use sw_model::EndpointKind::Device;
use sw_model::tier::{Refusal, Tier};
use sw_node::core::{NodeCore, NodeRefusal};
use sw_protocol::body::Audience;
#[test]
fn the_first_invite_opens_the_project_and_is_receipted_to_its_writer() {
    let mut w = World::enrolled();
    let sent = w
        .send_as(Who::Alice, Device, Audience::Node, create("p", None))
        .unwrap();
    assert_eq!(receipts(&sent), [(w.alice.device.cid, 10, true)]);
    assert_eq!(w.node.task("p").unwrap().coop.tier, Tier::Off);
    let sent = w
        .send_as(Who::Alice, Device, Audience::Node, invite(1, NOW + HOUR))
        .unwrap();
    assert!(receipts(&sent).contains(&(w.alice.device.cid, 23, true)));
    assert!(matches!(
        w.node.task("p").unwrap().coop.tier,
        Tier::Aware { .. }
    ));
    assert!(w.stored.keys().any(|k| k.starts_with("invite/")));
    // A second invite leaves the tier as it is.
    w.send_as(Who::Alice, Device, Audience::Node, invite(2, NOW + HOUR))
        .unwrap();
}

#[test]
fn only_the_owner_invites_and_never_for_longer_than_seven_days() {
    let mut w = invited();
    let by_bob = w.send_as(Who::Bob, Device, Audience::Node, invite(3, NOW + HOUR));
    assert_eq!(by_bob, Err(NodeRefusal::Tier(Refusal::NotAuthorized)));
    let week = 7 * 24 * HOUR;
    let long = w.send_as(
        Who::Alice,
        Device,
        Audience::Node,
        invite(3, NOW + week + 1),
    );
    assert_eq!(long, Err(NodeRefusal::InviteLifetime));
    let past = w.send_as(Who::Alice, Device, Audience::Node, invite(3, NOW));
    assert_eq!(past, Err(NodeRefusal::InviteLifetime));
    // Control: exactly seven days is accepted.
    w.send_as(Who::Alice, Device, Audience::Node, invite(3, NOW + week))
        .unwrap();
}

#[test]
fn joining_a_project_needs_its_invite_and_spends_it() {
    let mut w = invited();
    assert_eq!(
        bob_joins(&mut w, None, NOW).1,
        Err(NodeRefusal::NeedsInvite)
    );
    assert_eq!(
        bob_joins(&mut w, Some(9), NOW).1,
        Err(NodeRefusal::InviteInvalid)
    );
    // Control: the right token is accepted, and the invite is gone from memory and storage.
    let (id, ok) = bob_joins(&mut w, Some(1), NOW);
    ok.unwrap();
    assert!(!w.stored.keys().any(|k| k.starts_with("invite/")));
    // Refused, Bob tries the spent invite again: refused. A fresh invite works.
    w.send_as(Who::Alice, Device, Audience::Node, answer(id, false))
        .unwrap();
    assert_eq!(
        bob_joins(&mut w, Some(1), NOW).1,
        Err(NodeRefusal::InviteInvalid)
    );
    w.send_as(Who::Alice, Device, Audience::Node, invite(2, NOW + HOUR))
        .unwrap();
    bob_joins(&mut w, Some(2), NOW).1.unwrap();
}

#[test]
fn an_expired_invite_is_refused() {
    let mut w = World::enrolled();
    w.send_as(Who::Alice, Device, Audience::Node, create("p", None))
        .unwrap();
    w.send_as(Who::Alice, Device, Audience::Node, invite(1, NOW + 1_000))
        .unwrap();
    let mut control = World::enrolled();
    control
        .send_as(Who::Alice, Device, Audience::Node, create("p", None))
        .unwrap();
    control
        .send_as(Who::Alice, Device, Audience::Node, invite(1, NOW + 1_000))
        .unwrap();
    assert_eq!(
        bob_joins(&mut w, Some(1), NOW + 1_500).1,
        Err(NodeRefusal::InviteInvalid)
    );
    bob_joins(&mut control, Some(1), NOW + 500).1.unwrap();
}

#[test]
fn a_restarted_node_keeps_its_live_invites() {
    let w = invited();
    let mut again = NodeCore::new(rules(&w.issuer));
    for (k, v) in &w.stored {
        again.load(k, v).unwrap();
    }
    let mut w = World { node: again, ..w };
    let bob = w.bob.device.cid;
    w.connect(bob);
    bob_joins(&mut w, Some(1), NOW).1.unwrap();
}
