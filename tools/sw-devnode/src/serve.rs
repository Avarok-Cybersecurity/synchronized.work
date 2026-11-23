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

use crate::options::Serve;
use crate::status::{Counted, Status};

/// The STUN servers a loopback node names (the SDK requires three distinct ones).
pub const LOOPBACK_STUN: &str = "127.0.0.1:3478,127.0.0.1:3479,127.0.0.1:3480";

struct Vars(BTreeMap<String, String>);

impl Source for Vars {
    fn get(&self, name: &str) -> Option<String> {
        self.0.get(name).cloned()
    }
}

fn vars(o: &Serve) -> Result<Vars, String> {
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
    // Published test values: the seeds tools/onboard-e2e.sh provisions, or a KAT server's.
    m.insert(SECRET_SERVER_KEY_SEED.into(), o.server_seed_hex.clone());
    m.insert(SECRET_OPRF_SEED.into(), "22".repeat(32));
    let issuer = super::kat_issuer()?.public_key().to_base64url();
    m.insert(VAR_SIGN_IN_ISSUER_PK.into(), issuer);
    m.insert(
        VAR_CITADEL_WSS.into(),
        format!("ws://{}:{}/citadel", o.bind, o.port),
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
pub fn run(o: Serve) -> Result<String, String> {
    let provisioning = Provisioning::from_source(&vars(&o)?).map_err(|e| format!("{e:?}"))?;
    let bound = |port: u16| std::net::TcpListener::bind((o.bind, port)).map_err(|e| e.to_string());
    let ws = bound(o.port)?;
    let status_listener = bound(o.status_port)?;
    let status_addr = status_listener.local_addr().map_err(|e| e.to_string())?;
    // The SDK binds its TCP listener itself: a free port is found by a probe that closes first.
    let tcp = match o.tcp_port {
        0 => bound(0)?.local_addr().map_err(|e| e.to_string())?,
        port => std::net::SocketAddr::new(o.bind, port),
    };
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let status = std::sync::Arc::new(Status::default());
    crate::status::serve(status_listener, status.clone(), rt.handle().clone());
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
            .build(Counted {
                inner: kernel,
                status,
            })
            .map_err(|e| format!("{e:?}"))?;
        println!("status {status_addr}");
        println!("ready ws://{}:{}/citadel", o.bind, o.port);
        node.await.map_err(|e| format!("{e:?}"))?;
        Ok("stopped".into())
    })
}
