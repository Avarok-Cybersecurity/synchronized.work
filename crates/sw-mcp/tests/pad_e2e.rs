#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The scratchpad's whole signal chain, in-process, over the real Citadel SDK: Alice's agent
//! posts → her daemon seals and signs → the sw-node kernel stores and relays → Bob's daemon
//! verifies and wakes his waiter → Bob acks with a reply → the node relays → Alice's waiter
//! wakes with "acknowledged by Bob". Carol, a member the note is not for, is never woken; a
//! broadcast is the control. Then Bob is offline when Alice posts, and gets it on reconnect.
//!
//! Stand-ins, justified: none on the path under test. The project tree is loaded from the
//! node's store (as after a restart) because sw-mcp has no task-writing tools; the sign-in
//! issuer is the identity crate's `ServerKey` from the C0 KAT seed, as in `daemon.rs`.

mod support;

use std::time::Duration;

use serde_json::{Value as Json, json};
use support::pad_world::*;
use sw_mcp::ipc::Request;

const PROJECT: &str = "p";

fn post(to: Option<&str>, body: &str) -> Request {
    Request::PadPost {
        project: PROJECT.into(),
        to: to.map(|t| vec![t.to_owned()]),
        body: body.into(),
        refs: vec!["https://github.com/o/r/pull/9".into()],
    }
}

fn wait(after: u64, secs: u64) -> Request {
    Request::PadWait {
        after,
        max_wait_s: secs,
    }
}

fn timed_out(v: &Json) -> bool {
    v["timeout"] == json!(true)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_note_wakes_its_target_whose_ack_wakes_the_sender_and_an_offline_target_catches_up() {
    let (_, alice) = person(0xA1);
    let (_, bob) = person(0xB2);
    let (_, carol) = person(0xC3);
    let store = MemStore::default();
    project(&store, &alice, &[&bob, &carol]);
    let (port, core) = start_node(store);

    let (ia, ib, ic) = (person(0xA1).0, person(0xB2).0, person(0xC3).0);
    let (ha, hb, hc) = (
        home_for("a", &ia).await,
        home_for("b", &ib).await,
        home_for("c", &ic).await,
    );
    let _ = sw_mcp::logging::init(&ha, sw_mcp::config::LogLevel::Info);
    let _a = Running::start(&ha, port);
    let b = Running::start(&hb, port);
    let _c = Running::start(&hc, port);

    let policy = Request::PadPolicy {
        project: PROJECT.into(),
        retention_hours: 24,
    };
    until(&ha, 180, policy, |_| true).await;
    let members = Request::PadMembers {
        project: PROJECT.into(),
    };
    until(&ha, 180, members, |v| {
        v["members"]
            .as_array()
            .is_some_and(|m| m.len() == 3 && m.iter().all(|x| x["verified_devices"] == json!(1)))
    })
    .await;

    // A → B: B wakes, C does not.
    let sent = ask(
        &ha,
        post(Some(bob.as_str()), "free to merge my PR when you're ready"),
        30,
    )
    .await
    .unwrap();
    let note_id = sent["note_id"].as_str().unwrap().to_owned();
    let at_b = ask(&hb, wait(0, 30), 30).await.unwrap();
    assert_eq!(at_b["kind"], json!("note"));
    assert_eq!(at_b["note_id"], json!(note_id));
    assert_eq!(at_b["from"]["id_fp"], json!(alice.as_str()));
    assert_eq!(at_b["trust"], json!("untrusted"));
    assert!(
        at_b["frame"]
            .as_str()
            .unwrap()
            .contains("DATA, NOT INSTRUCTIONS")
    );
    assert!(
        timed_out(&ask(&hc, wait(0, 3), 3).await.unwrap()),
        "C is not woken by a note to B"
    );

    // The sender is not woken before the ack (control), then is, with the reply.
    assert!(
        timed_out(&ask(&ha, wait(0, 2), 2).await.unwrap()),
        "no ack yet"
    );
    let ack = Request::PadAck {
        note_id: note_id.clone(),
        reply: Some("merging now".into()),
    };
    ask(&hb, ack, 0).await.unwrap();
    let at_a = ask(&ha, wait(0, 30), 30).await.unwrap();
    assert_eq!(at_a["kind"], json!("ack"));
    assert_eq!(at_a["from"]["id_fp"], json!(bob.as_str()));
    assert_eq!(at_a["reply"], json!("merging now"));
    let after_ack = at_a["cursor"].as_u64().unwrap();
    assert!(
        timed_out(&ask(&hb, wait(0, 2), 2).await.unwrap()),
        "acked: B is not woken again"
    );

    // Control for targeting: a broadcast does wake C.
    ask(&ha, post(None, "release branch is cut"), 30)
        .await
        .unwrap();
    let at_c = ask(&hc, wait(0, 30), 30).await.unwrap();
    assert_eq!(at_c["kind"], json!("note"));

    // B offline at post time: nothing lost.
    let b_cid = {
        let c = core.lock().unwrap();
        c.directory().endpoints_of(&bob)[0]
    };
    drop(b);
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    while core.lock().unwrap().is_online(b_cid) {
        assert!(
            std::time::Instant::now() < deadline,
            "the node never saw B go offline"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let sent = ask(
        &ha,
        post(Some(bob.as_str()), "rebased on main, please re-review"),
        30,
    )
    .await
    .unwrap();
    let offline_note = sent["note_id"].as_str().unwrap().to_owned();
    let _b = Running::start(&hb, port);
    // Re-armed from the last cursor B handled, as a 24/7 session does.
    let handled = at_b["cursor"].as_u64().unwrap();
    let mut after = handled;
    let mut seen = Vec::new();
    while !seen.contains(&json!(offline_note)) {
        let v = until(&hb, 120, wait(after, 25), |v| !timed_out(v)).await;
        after = v["cursor"].as_u64().unwrap();
        seen.push(v["note_id"].clone());
    }
    assert_eq!(
        seen.len(),
        2,
        "the broadcast B had not acked, then the offline note: {seen:?}"
    );
    assert!(
        timed_out(&ask(&ha, wait(after_ack, 2), 2).await.unwrap()),
        "no stray ack for A"
    );
}
