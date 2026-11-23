#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The node as a Citadel server, end to end (design §11.1 step 2: "a real Citadel client in the
//! test dials the kernel — the SDK's own test pattern"). Two accounts register and sign in
//! through the node's post-quantum sign-in, enrol by assertion over their C2S channels, and
//! the owner receives the `NOTIFY` for the other's join request on her own session. An app
//! session's account (a 47-byte name) asks the server identity challenge right after it
//! connects, as the Citadel agent does, and verifies the answer.

mod support;

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use citadel_sdk::prelude::*;
use citadel_sdk::test_common::{LOCALHOST_STUN_SERVERS, server_test_node};
use support::challenge::{domain, frames, kat_server, parse_answer, verifies};
use support::vars::complete;
use sw_node::Provisioning;
use sw_node::core::Effect;
use sw_node::provision::VAR_STUN_SERVERS;
use sw_node::store::{NodeStore, StoreError};
use sw_protocol::body::Audience;
use sw_protocol::ports::Clock;

mod e2e;
use e2e::{Account, Client, ClientKernel, now_ms};

#[derive(Default, Clone)]
struct MemStore(std::sync::Arc<Mutex<BTreeMap<String, Vec<u8>>>>);

#[async_trait]
impl NodeStore for MemStore {
    async fn load_all(&self) -> Result<Vec<(String, Vec<u8>)>, StoreError> {
        Ok(self.0.lock().unwrap().clone().into_iter().collect())
    }
    async fn put(&self, key: &str, value: &[u8]) -> Result<(), StoreError> {
        self.0
            .lock()
            .unwrap()
            .insert(key.to_owned(), value.to_vec());
        Ok(())
    }
    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
}

struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        now_ms()
    }
}

fn provisioning() -> Provisioning {
    let mut vars = complete();
    vars.0
        .insert(VAR_STUN_SERVERS, LOCALHOST_STUN_SERVERS.join(","));
    Provisioning::from_source(&vars).unwrap()
}

/// A client node that runs `f`, as the SDK's own tests build one.
fn client<F, Fut>(f: F) -> NodeFuture<'static, ClientKernel<F>>
where
    F: FnOnce(NodeRemote<StackedRatchet>) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), NetworkError>> + Send + 'static,
{
    citadel_sdk::test_common::node_builder()
        .with_link_manager(LinkManagerMode::Legacy)
        .with_max_security_level(citadel_sdk::test_common::localhost_max_security_level())
        .with_stun_servers(LOCALHOST_STUN_SERVERS)
        .build(ClientKernel::new(f))
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn two_people_sign_in_enrol_and_the_owner_is_told_of_a_join_request() {
    let provisioning = provisioning();
    let store = MemStore::default();
    let kernel = sw_node::start::prepare(&provisioning, store.clone(), SystemClock)
        .await
        .unwrap();
    let core = kernel.core();
    let (server, addr) = server_test_node(kernel, |builder| {
        sw_node::start::configure(builder, &provisioning, BackendType::InMemory).unwrap();
    });

    let client = client(move |remote| async move {
        let mut alice = Client::sign_in(&remote, addr, Account::alice()).await?;
        let mut bob = Client::sign_in(&remote, addr, Account::bob()).await?;
        alice.enrol().await?;
        bob.enrol().await?;
        alice.send(Audience::Node, e2e::create("p")).await?;
        // The first invite opens the project to requests; Bob spends it.
        alice
            .send(Audience::Node, e2e::invite("p", &[9; 32]))
            .await?;
        bob.send(Audience::Node, e2e::join("p", Some([9; 32])))
            .await?;
        let mut kinds = Vec::new();
        for _ in 0..2 {
            let notify = tokio::time::timeout(Duration::from_secs(30), alice.next_notify())
                .await
                .map_err(|_| NetworkError::msg("no NOTIFY reached the owner"))??;
            kinds.push(notify.kind);
        }
        // Opening the project up is told to its owner; the request, to the task owner.
        assert_eq!(kinds, ["tier_changed", "join_requested"]);
        remote.shutdown().await
    });
    tokio::select! {
        _ = server => panic!("the server ended first"),
        res = client => { res.unwrap(); }
    }
    let core = core.lock().unwrap();
    assert_eq!(core.directory().persons(), 2);
    assert!(core.task("p").is_some());
    let stored = store.0.lock().unwrap();
    assert!(stored.keys().any(|k| k.starts_with("person/")));
    assert!(stored.contains_key("task/p"));
}

/// Control: provision the core with no server identity in `start::prepare` → red.
#[tokio::test(flavor = "multi_thread")]
async fn the_started_node_answers_with_its_provisioned_identity() {
    let provisioning = provisioning();
    let kernel = sw_node::start::prepare(&provisioning, MemStore::default(), SystemClock)
        .await
        .unwrap();
    let kat = kat_server();
    let effects = kernel
        .core()
        .lock()
        .unwrap()
        .received(kat.cid, &frames().request, now_ms())
        .unwrap();
    let [Effect::Send { to, bytes }] = effects.as_slice() else {
        panic!("not one answer: {effects:?}");
    };
    assert_eq!(*to, kat.cid);
    let answer = parse_answer(bytes);
    assert!(verifies(&answer, "synchronized.work", kat.cid, &kat.nonce));
    let json = sw_node::well_known::well_known_json(&provisioning).unwrap();
    let doc = citadel_identity::parse_well_known(&json, &domain("synchronized.work")).unwrap();
    assert_eq!(answer.pk, doc.server_key.as_bytes().to_vec());
}

/// Control: let a challenge fall through to the `SwMessage` decode in `received` → no answer.
#[tokio::test(flavor = "multi_thread")]
async fn an_agent_held_account_proves_the_node_right_after_connecting() {
    let provisioning = provisioning();
    let kernel = sw_node::start::prepare(&provisioning, MemStore::default(), SystemClock)
        .await
        .unwrap();
    let (server, addr) = server_test_node(kernel, |builder| {
        sw_node::start::configure(builder, &provisioning, BackendType::InMemory).unwrap();
    });
    let pinned = provisioning.server_key().public_key().as_bytes().to_vec();
    let client = client(move |remote| async move {
        let account = Account::app_session();
        assert_eq!(account.username.len(), 47);
        let mut session = Client::open(&remote, addr, account).await?;
        let nonce = kat_server().nonce;
        for round in 0..2 {
            session.send_raw(frames().request).await?;
            let frame = tokio::time::timeout(Duration::from_secs(30), session.next_frame())
                .await
                .map_err(|_| NetworkError::msg("the node never answered the challenge"))??;
            let answer = parse_answer(&frame);
            assert_eq!(answer.pk, pinned, "round {round}");
            assert!(verifies(&answer, "synchronized.work", session.cid, &nonce));
            // The agent asks again after its next connect; a second later is the soonest.
            tokio::time::sleep(Duration::from_millis(1100)).await;
        }
        remote.shutdown().await
    });
    tokio::select! {
        _ = server => panic!("the server ended first"),
        res = client => { res.unwrap(); }
    }
}
