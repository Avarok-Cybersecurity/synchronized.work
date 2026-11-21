#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The receiving daemon's checks before it wakes anyone, with the node played by a stand-in
//! that relays whatever the test hands it (as a compromised Worker could): a forged note, a
//! replay, a note whose proof the pinned issuer never signed, and a note for someone else are
//! all dropped with no event; the untouched note is the control and wakes exactly once.

mod support;

use serde_json::json;
use support::pad_sender::{Alice, alice, note};
use support::pad_world::{ask, home_for, issuer, person};
use sw_mcp::daemon::Daemon;
use sw_mcp::ipc::Request;

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
