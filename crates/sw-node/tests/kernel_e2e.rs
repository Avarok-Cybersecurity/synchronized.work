#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The node as a Citadel server, end to end (design §11.1 step 2: "a real Citadel client in the
//! test dials the kernel — the SDK's own test pattern"). Two accounts register and sign in
//! through the node's post-quantum sign-in, enrol by assertion over their C2S channels, and
//! the owner receives the `NOTIFY` for the other's join request on her own session.

mod support;

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use citadel_sdk::prelude::*;
use citadel_sdk::test_common::{LOCALHOST_STUN_SERVERS, server_test_node};
use support::vars::complete;
use sw_node::Provisioning;
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

#[tokio::test(flavor = "multi_thread")]
async fn two_people_sign_in_enrol_and_the_owner_is_told_of_a_join_request() {
    let mut vars = complete();
    vars.0
        .insert(VAR_STUN_SERVERS, LOCALHOST_STUN_SERVERS.join(","));
    let provisioning = Provisioning::from_source(&vars).unwrap();
    let store = MemStore::default();
    let kernel = sw_node::start::prepare(&provisioning, store.clone(), SystemClock)
        .await
        .unwrap();
    let core = kernel.core();
    let (server, addr) = server_test_node(kernel, |builder| {
        sw_node::start::configure(builder, &provisioning, BackendType::InMemory).unwrap();
    });

    let client = citadel_sdk::test_common::node_builder()
        .with_link_manager(LinkManagerMode::Legacy)
        .with_max_security_level(citadel_sdk::test_common::localhost_max_security_level())
        .with_stun_servers(LOCALHOST_STUN_SERVERS)
        .build(ClientKernel::new(move |remote| async move {
            let mut alice = Client::sign_in(&remote, addr, Account::alice()).await?;
            let mut bob = Client::sign_in(&remote, addr, Account::bob()).await?;
            alice.enrol().await?;
            bob.enrol().await?;
            alice.send(Audience::Node, e2e::create("p")).await?;
            alice.send(Audience::Node, e2e::open_up("p")).await?;
            bob.send(Audience::Node, e2e::join("p")).await?;
            let mut kinds = Vec::new();
            for _ in 0..2 {
                let notify = tokio::time::timeout(Duration::from_secs(30), alice.next_notify())
                    .await
                    .map_err(|_| NetworkError::msg("no NOTIFY reached the owner"))??;
                kinds.push(notify.kind);
            }
            // Opening the task up is told to the project owner; the request, to the task owner.
            assert_eq!(kinds, ["tier_changed", "join_requested"]);
            remote.shutdown().await
        }))
        .unwrap();
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
