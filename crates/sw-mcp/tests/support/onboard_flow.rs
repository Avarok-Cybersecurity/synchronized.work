//! The onboarding path end to end against a node on `port`: Alice creates a project and
//! invites Bob, Bob spends the invite, Alice is woken and admits him (Bob cannot admit himself),
//! both see each other as members, and a note and its ack cross between them. Carol shows a
//! spent invite is refused and a refused person gets nothing. Production daemons throughout;
//! the node is either the in-process sw-node kernel or `wrangler dev`.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value as Json, json};
use sw_mcp::home::Home;
use sw_mcp::ipc::Request;

use super::pad_world::*;

fn wait(after: u64, secs: u64) -> Request {
    Request::PadWait {
        after,
        max_wait_s: secs,
    }
}

/// The next event of `kind` after `after` on `home`, within a minute.
async fn woken(home: &Home, after: u64, kind: &str) -> Json {
    let mut after = after;
    loop {
        let v = until(home, 60, wait(after, 25), |v| v["timeout"] != json!(true)).await;
        if v["kind"] == json!(kind) {
            return v;
        }
        after = v["cursor"].as_u64().unwrap();
    }
}

fn members(v: &Json) -> Vec<String> {
    let mut m: Vec<String> = v["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id_fp"].as_str().unwrap().to_owned())
        .collect();
    m.sort();
    m
}

async fn invite(owner: &Home, project: &str) -> String {
    let r = Request::InviteCreate {
        project: project.into(),
        valid_hours: 24,
    };
    let v = ask(owner, r, 30).await.unwrap();
    v["invite"].as_str().unwrap().to_owned()
}

fn redeem(code: &str) -> Request {
    Request::InviteRedeem {
        code: code.into(),
        note: "hi, it's me".into(),
    }
}

async fn request_of(owner: &Home, project: &str, person: &str) -> String {
    let r = Request::JoinRequests {
        project: Some(project.into()),
    };
    let v = until(owner, 60, r, |v| {
        v["requests"]
            .as_array()
            .is_some_and(|r| r.iter().any(|x| x["person"] == json!(person)))
    })
    .await;
    let r = v["requests"].as_array().unwrap();
    let mine = r.iter().find(|x| x["person"] == json!(person)).unwrap();
    mine["request_id"].as_str().unwrap().to_owned()
}

fn answer(request_id: &str, accept: bool) -> Request {
    Request::JoinAnswer {
        request_id: request_id.into(),
        accept,
        reason: None,
    }
}

pub async fn run(tag: &str, port: u16) {
    let (ia, alice) = person(0xA1);
    let (ib, bob) = person(0xB2);
    let (ic, carol) = person(0xC3);
    let ha = home_for(&format!("{tag}-a"), &ia).await;
    let hb = home_for(&format!("{tag}-b"), &ib).await;
    let hc = home_for(&format!("{tag}-c"), &ic).await;
    let _a = Running::start(&ha, port);
    let _b = Running::start(&hb, port);
    let _c = Running::start(&hc, port);

    let created = until(
        &ha,
        180,
        Request::ProjectCreate {
            name: "Acme".into(),
        },
        |_| true,
    )
    .await;
    let project = created["project"].as_str().unwrap().to_owned();
    assert_eq!(created["scratchpad_retention_hours"], json!(168));
    let code = invite(&ha, &project).await;
    let asked = until(&hb, 180, redeem(&code), |_| true).await;
    assert_eq!(asked["owner"], json!(alice.as_str()));

    // The owner's agent is woken by the request; Bob cannot answer it, Alice can.
    woken(&ha, 0, "join_requested").await;
    let request = request_of(&ha, &project, bob.as_str()).await;
    let by_bob = ask(&hb, answer(&request, true), 30).await;
    assert!(
        by_bob.as_ref().is_err_and(|e| e.contains("NotAuthorized")),
        "{by_bob:?}"
    );
    let ms = Request::PadMembers {
        project: project.clone(),
    };
    assert_eq!(
        ask(&ha, ms.clone(), 30).await.map(|v| members(&v).len()),
        Ok(1)
    );
    ask(&ha, answer(&request, true), 30).await.unwrap();
    woken(&hb, 0, "joined").await;
    let mut both = vec![alice.as_str().to_owned(), bob.as_str().to_owned()];
    both.sort();
    for h in [&ha, &hb] {
        let v = until(h, 60, ms.clone(), |v| {
            v["members"].as_array().is_some_and(|m| {
                m.len() == 2 && m.iter().all(|x| x["verified_devices"] == json!(1))
            })
        })
        .await;
        assert_eq!(members(&v), both);
    }

    // A note and its ack, both ways of waking.
    let post = Request::PadPost {
        project: project.clone(),
        to: None,
        body: "welcome aboard".into(),
        refs: vec![],
    };
    let sent = ask(&ha, post, 30).await.unwrap();
    let note = woken(&hb, 0, "note").await;
    assert_eq!(note["note_id"], sent["note_id"]);
    assert_eq!(note["from"]["id_fp"], json!(alice.as_str()));
    let ack = Request::PadAck {
        note_id: note["note_id"].as_str().unwrap().into(),
        reply: Some("thanks".into()),
    };
    ask(&hb, ack, 0).await.unwrap();
    let acked = woken(&ha, 0, "ack").await;
    assert_eq!(acked["reply"], json!("thanks"));

    // Carol: the spent invite is refused; a fresh one is accepted (control), then refused by
    // the owner, and Carol is no member.
    let spent = online(&hc, redeem(&code)).await;
    assert!(
        spent.as_ref().is_err_and(|e| e.contains("InviteInvalid")),
        "{spent:?}"
    );
    let fresh = invite(&ha, &project).await;
    online(&hc, redeem(&fresh)).await.unwrap();
    let request = request_of(&ha, &project, carol.as_str()).await;
    ask(&ha, answer(&request, false), 30).await.unwrap();
    let v = ask(&ha, ms.clone(), 30).await.unwrap();
    assert_eq!(members(&v), both, "a refused person is no member");
    let again = online(&hc, redeem(&fresh)).await;
    assert!(
        again.as_ref().is_err_and(|e| e.contains("InviteInvalid")),
        "{again:?}"
    );
}

/// Asks once the daemon is connected: retries only "not connected", returns anything else.
async fn online(home: &Home, r: Request) -> Result<Json, String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
    loop {
        match ask(home, r.clone(), 30).await {
            Err(e) if e.contains("not connected") || e.contains("not running") => {
                assert!(std::time::Instant::now() < deadline, "never connected: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            other => return other,
        }
    }
}
