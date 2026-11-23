#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! `sw-mcp link-code` end to end: a production daemon creates a project and mints a code for
//! it against the production sw-node kernel in-process; the node stores the code's hash bound
//! to that project. A code for a project the Person is not a member of is refused, also when
//! the session follows another project the node would otherwise bind it to. The daemon's own
//! subscription is answered: its projection holds the project's tasks.

mod support;

use serde_json::json;
use support::pad_world::{MemStore, Running, ask, home_for, person, start_node, until};
use sw_mcp::ipc::Request;
use sw_protocol::link_code::{canonical, code_hash};

#[tokio::test(flavor = "multi_thread")]
async fn a_signed_in_device_mints_a_link_code_for_its_project() {
    let store = MemStore::default();
    let (port, _core) = start_node(store.clone());
    let (identity, _) = person(0xA1);
    let home = home_for("link-code", &identity).await;
    let _daemon = Running::start(&home, port);
    let create = Request::ProjectCreate {
        name: "Acme".into(),
    };
    let created = until(&home, 180, create, |_| true).await;
    let project = created["project"].as_str().unwrap().to_owned();

    let minted = ask(
        &home,
        Request::LinkCode {
            project: project.clone(),
        },
        30,
    )
    .await
    .unwrap();
    assert_eq!(minted["project"], json!(project));
    let code = minted["code"].as_str().unwrap();
    assert_eq!(
        canonical(code).as_deref(),
        Some(code),
        "eight symbols of the alphabet"
    );
    let hex: String = code_hash(code).iter().map(|b| format!("{b:02x}")).collect();
    assert!(
        store.0.lock().unwrap().contains_key(&format!("link/{hex}")),
        "the node keeps its hash"
    );

    let elsewhere = ask(
        &home,
        Request::LinkCode {
            project: "not-a-project".into(),
        },
        30,
    )
    .await;
    let refused = |e: &String| e.contains("none you are a member of");
    assert!(elsewhere.as_ref().is_err_and(refused), "{elsewhere:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_code_for_another_project_than_asked_is_refused_and_a_followed_project_is_fed() {
    let (port, _core) = start_node(MemStore::default());
    let (identity, _) = person(0xA2);
    let home = home_for("link-code-follow", &identity).await;
    let _daemon = Running::start(&home, port);
    let create = Request::ProjectCreate {
        name: "Acme".into(),
    };
    let project = until(&home, 180, create, |_| true).await["project"]
        .as_str()
        .unwrap()
        .to_owned();
    let attach = Request::Attach {
        agent_id: "agent-1".into(),
        project: project.clone(),
        task: project.clone(),
        cwd: "/tmp/acme".into(),
    };
    ask(&home, attach, 0).await.unwrap();
    let status = Request::ProjectStatus {
        project: project.clone(),
    };
    let fed = until(&home, 60, status, |v| {
        v["tasks"][&project]["title"] == json!("Acme")
    })
    .await;
    assert_eq!(fed["agents"]["agent-1"]["lane"], json!("running"));

    let other = ask(
        &home,
        Request::LinkCode {
            project: "not-a-project".into(),
        },
        30,
    )
    .await;
    assert!(
        other
            .as_ref()
            .is_err_and(|e| e.contains("not to this project")),
        "{other:?}"
    );
}
