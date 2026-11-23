//! The node: wrangler.toml's `[vars]` (the one provisioning), with only the values a test node
//! must not share with the deployment replaced by published test ones.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use citadel_sdk::prelude::*;
use sw_node::provision::{
    SECRET_OPRF_SEED, SECRET_SERVER_KEY_SEED, VAR_CITADEL_WSS, VAR_SIGN_IN_ISSUER_PK,
    VAR_STUN_SERVERS,
};
use sw_node::store::{NodeStore, StoreError};
use sw_node::{Provisioning, Source};
use sw_protocol::ports::Clock;

/// The STUN servers a loopback node names (the SDK requires three distinct ones).
pub const LOOPBACK_STUN: &str = "127.0.0.1:3478,127.0.0.1:3479,127.0.0.1:3480";

struct Vars(BTreeMap<String, String>);

impl Source for Vars {
    fn get(&self, name: &str) -> Option<String> {
        self.0.get(name).cloned()
    }
}

fn vars(port: u16) -> Result<Vars, String> {
    let wrangler: toml::Table = include_str!("../../../wrangler.toml")
        .parse()
        .map_err(|e| format!("wrangler.toml: {e}"))?;
    let table = wrangler.get("vars").and_then(toml::Value::as_table);
    let mut m = BTreeMap::new();
    for (k, v) in table.ok_or("wrangler.toml has no [vars]")? {
        let v = v
            .as_str()
            .ok_or_else(|| format!("wrangler.toml var {k} is not a string"))?;
        m.insert(k.clone(), v.to_owned());
    }
    // The seeds tools/onboard-e2e.sh provisions: published test values.
    m.insert(SECRET_SERVER_KEY_SEED.into(), "11".repeat(32));
    m.insert(SECRET_OPRF_SEED.into(), "22".repeat(32));
    let issuer = super::kat_issuer()?.public_key().to_base64url();
    m.insert(VAR_SIGN_IN_ISSUER_PK.into(), issuer);
    m.insert(
        VAR_CITADEL_WSS.into(),
        format!("ws://127.0.0.1:{port}/citadel"),
    );
    m.insert(VAR_STUN_SERVERS.into(), LOOPBACK_STUN.into());
    Ok(Vars(m))
}

#[derive(Default, Clone)]
struct Memory(Arc<Mutex<BTreeMap<String, Vec<u8>>>>);

#[async_trait::async_trait]
impl NodeStore for Memory {
    async fn load_all(&self) -> Result<Vec<(String, Vec<u8>)>, StoreError> {
        let map = self
            .0
            .lock()
            .map_err(|_| StoreError("the store lock was poisoned".into()))?;
        Ok(map.clone().into_iter().collect())
    }
    async fn put(&self, key: &str, value: &[u8]) -> Result<(), StoreError> {
        let mut map = self
            .0
            .lock()
            .map_err(|_| StoreError("the store lock was poisoned".into()))?;
        map.insert(key.to_owned(), value.to_vec());
        Ok(())
    }
    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        let mut map = self
            .0
            .lock()
            .map_err(|_| StoreError("the store lock was poisoned".into()))?;
        map.remove(key);
        Ok(())
    }
}

struct WallClock;

impl Clock for WallClock {
    fn now_ms(&self) -> u64 {
        sw_mcp::daemon::now_ms()
    }
}

/// Runs the node until the process ends; returns only on a failure to start or a stop.
pub fn run(port: u16) -> Result<String, String> {
    let provisioning = Provisioning::from_source(&vars(port)?).map_err(|e| format!("{e:?}"))?;
    let ws = std::net::TcpListener::bind(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    // A free port for the SDK's TCP listener, which binds it itself: this probe closes first.
    let tcp = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|probe| probe.local_addr())
        .map_err(|e| e.to_string())?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(async move {
        let kernel = sw_node::start::prepare(&provisioning, Memory::default(), WallClock)
            .await
            .map_err(|e| format!("{e:?}"))?;
        let mut builder = citadel_sdk::test_common::node_builder();
        sw_node::start::configure(&mut builder, &provisioning, BackendType::InMemory)
            .map_err(|e| format!("{e:?}"))?;
        let node = builder
            .with_node_type(NodeType::Server(tcp))
            .with_path_policy(citadel_sdk::test_common::localhost_path_policy())
            .with_bound_websocket_listener(ws)
            .build(kernel)
            .map_err(|e| format!("{e:?}"))?;
        println!("ready ws://127.0.0.1:{port}/citadel");
        node.await.map_err(|e| format!("{e:?}"))?;
        Ok("stopped".into())
    })
}
