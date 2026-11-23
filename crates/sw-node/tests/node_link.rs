#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Link codes (design §5.3 "ships first"): a signed-in Endpoint mints, a Citadel App's account
//! redeems once within ten minutes, and is then a viewer of the Person for that Project —
//! `EVENT`s and `NOTIFY`s reach it. Wrong, spent and expired codes are refused with a reason.

mod feed_support;
mod support;

use feed_support::*;
use support::rules::notified;
use support::*;
use sw_model::EndpointKind::Controller;
use sw_node::core::{LinkRefusal, NodeRefusal};
use sw_protocol::body::Audience;
use sw_protocol::link_code::link_limits::LIFETIME_MS;
use sw_protocol::payload::*;
use sw_protocol::signed::SwMessage;

const APP: u64 = 7000;
const CODE: &str = "ABCD2345";

/// A Citadel App's agent-held account: a session, then `HELLO` as a viewer.
fn app_session(w: &mut World, cid: u64) {
    w.node.connected(cid, NOW);
    let hello = SwMessage::Unsigned(sw_client::live::hello("sw-app@test"));
    w.node
        .received(cid, &hello.to_bytes().unwrap(), NOW)
        .unwrap();
}

fn redeem_at(w: &mut World, cid: u64, code: &str, now: u64) -> (Sent, [u8; 32]) {
    let bytes = SwMessage::Unsigned(Payload::Link(Link {
        op: LinkOp::Redeem,
        code: code.into(),
        acting_username: Some("alice".into()),
        install_id: Some("inst-0123456789".into()),
        node_id: Some("node-7".into()),
    }))
    .to_bytes()
    .unwrap();
    let effects = w.node.received(cid, &bytes, now).unwrap();
    (w.apply(effects), sw_crypto::sha3_256(&bytes))
}

fn redeem(w: &mut World, cid: u64, code: &str) -> (Sent, [u8; 32]) {
    redeem_at(w, cid, code, NOW)
}

/// The one receipt `to` got: `(stored, project, reason)`.
fn answer(sent: &Sent, to: u64) -> (bool, String, Option<String>) {
    let r = receipts(sent, to);
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(r[0].typ, 16);
    (r[0].stored, r[0].project.clone(), r[0].reason.clone())
}

fn refused(r: LinkRefusal) -> (bool, String, Option<String>) {
    (false, String::new(), Some(r.reason().to_owned()))
}

#[test]
fn a_redeemed_code_links_the_app_which_then_receives_events_and_notifies() {
    let mut w = project();
    let minted = mint(&mut w, CODE);
    assert_eq!(
        answer(&minted, w.alice.controller.cid),
        (true, "p".into(), None)
    );
    app_session(&mut w, APP);
    let (sent, id) = redeem(&mut w, APP, "abcd2345");
    assert_eq!(
        answer(&sent, APP),
        (true, "p".into(), None),
        "typed in lower case"
    );
    assert_eq!(
        receipts(&sent, APP)[0].id,
        id,
        "the receipt answers this redeem"
    );

    let caught_up = subscribe(&mut w, APP, "p", 0).unwrap();
    assert_eq!(task_ids(&events(&caught_up, APP)).len(), 2, "p and t");
    let request = bob_requests(&mut w, "t");
    let sent = alice(&mut w, accept(request));
    assert!(!events(&sent, APP).is_empty(), "the tier change, live");
    assert_eq!(
        notified(&sent)
            .iter()
            .filter(|(to, k)| *to == APP && k == "person_joined_task")
            .count(),
        1
    );
}

#[test]
fn a_wrong_code_is_refused_with_its_reason() {
    let mut w = project();
    mint(&mut w, CODE);
    app_session(&mut w, APP);
    assert_eq!(
        answer(&redeem(&mut w, APP, "ABCD2346").0, APP),
        refused(LinkRefusal::Unknown)
    );
    assert_eq!(
        answer(&redeem(&mut w, APP, "ABCD-234").0, APP),
        refused(LinkRefusal::NotACode)
    );
    assert_eq!(
        subscribe(&mut w, APP, "p", 0),
        Err(NodeRefusal::UnknownPerson),
        "not linked"
    );
}

#[test]
fn a_code_is_spent_by_its_first_redeem() {
    let mut w = project();
    mint(&mut w, CODE);
    app_session(&mut w, APP);
    app_session(&mut w, APP + 1);
    assert!(answer(&redeem(&mut w, APP, CODE).0, APP).0);
    assert_eq!(
        answer(&redeem(&mut w, APP + 1, CODE).0, APP + 1),
        refused(LinkRefusal::Spent)
    );
}

#[test]
fn a_code_expires_after_ten_minutes() {
    let mut w = project();
    mint(&mut w, CODE);
    app_session(&mut w, APP);
    let late = redeem_at(&mut w, APP, CODE, NOW + LIFETIME_MS).0;
    assert_eq!(answer(&late, APP), refused(LinkRefusal::Expired));
    let in_time = redeem_at(&mut w, APP, CODE, NOW + LIFETIME_MS - 1).0;
    assert!(
        answer(&in_time, APP).0,
        "the control: a minute earlier it links"
    );
}

#[test]
fn redeem_attempts_are_limited_per_account_per_minute() {
    let mut w = project();
    mint(&mut w, CODE);
    app_session(&mut w, APP);
    for _ in 0..3 {
        assert_eq!(
            answer(&redeem(&mut w, APP, "ZZZZZZZZ").0, APP),
            refused(LinkRefusal::Unknown)
        );
    }
    let blocked = redeem(&mut w, APP, CODE).0;
    assert_eq!(
        answer(&blocked, APP),
        refused(LinkRefusal::TooManyAttempts),
        "even the right code"
    );
    let later = redeem_at(&mut w, APP, CODE, NOW + 60_000).0;
    assert!(answer(&later, APP).0, "a minute later it links");
}

#[test]
fn a_mint_names_the_project_its_session_follows_and_one_code_is_one_hash() {
    let mut w = project();
    let link = Payload::Link(Link {
        op: LinkOp::Code,
        code: CODE.into(),
        acting_username: None,
        install_id: None,
        node_id: None,
    });
    let me = w.alice.controller.cid;
    let sent = w
        .send_as(Who::Alice, Controller, Audience::Node, link.clone())
        .unwrap();
    assert_eq!(
        answer(&sent, me),
        refused(LinkRefusal::NoProject),
        "it follows no project"
    );
    assert!(!w.stored.keys().any(|k| k.starts_with("link/")));
    assert!(answer(&mint(&mut w, CODE), me).0);
    let again = w
        .send_as(Who::Alice, Controller, Audience::Node, link)
        .unwrap();
    assert_eq!(
        answer(&again, me),
        (false, "p".into(), Some(LinkRefusal::Taken.reason().into()))
    );
}

#[test]
fn a_linked_app_views_only_the_linked_project() {
    let mut w = project();
    alice(&mut w, create("q", None, sw_model::Visibility::Members));
    mint(&mut w, CODE);
    app_session(&mut w, APP);
    redeem(&mut w, APP, CODE);
    assert!(subscribe(&mut w, APP, "p", 0).is_ok());
    assert_eq!(
        subscribe(&mut w, APP, "q", 0),
        Err(NodeRefusal::UnknownPerson)
    );
}

#[test]
fn links_and_spent_codes_survive_a_restart() {
    let mut w = project();
    mint(&mut w, CODE);
    app_session(&mut w, APP);
    redeem(&mut w, APP, CODE);
    w.node = restarted(&w);
    app_session(&mut w, APP);
    let caught_up = subscribe(&mut w, APP, "p", 0).unwrap();
    assert_eq!(task_ids(&events(&caught_up, APP)).len(), 2);
    app_session(&mut w, APP + 1);
    assert_eq!(
        answer(&redeem(&mut w, APP + 1, CODE).0, APP + 1),
        refused(LinkRefusal::Spent)
    );
}
