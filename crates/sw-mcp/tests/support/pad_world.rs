//! The scratchpad end to end: the production sw-node kernel on the real Citadel SDK, serving
//! WebSocket on loopback, and production `Daemon`s for three people, each on its own runtime
//! so a test can take one offline and bring it back on the same home.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use citadel_identity::{
    Identity, MlDsa65Pk, ServerKey, SignInAssertion, WellKnownCitadel, canonical_domain,
};
use citadel_sdk::prelude::*;
use serde_json::Value as Json;
use sw_mcp::daemon::{Daemon, now_ms};
use sw_mcp::home::{Home, write_private};
use sw_mcp::ipc::{Request, call_async};
use sw_mcp::keys::DeviceKeys;
use sw_model::IdFp;
use sw_model::tier::{Coop, JoinPolicy, Tier};
use sw_node::Provisioning;
use sw_node::core::{NodeCore, TaskState};
use sw_node::store::{NodeStore, StoreError};
use sw_protocol::ports::{Clock, IdentityPort as _};
use sw_testkit::LabelledEntropy;

use super::device::{PinOnly, config};

#[path = "../../../sw-node/tests/support/vars.rs"]
mod vars;

#[derive(Default, Clone)]
pub struct MemStore(pub Arc<Mutex<BTreeMap<String, Vec<u8>>>>);

#[async_trait::async_trait]
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

struct WallClock;
impl Clock for WallClock {
    fn now_ms(&self) -> u64 {
        now_ms()
    }
}

/// The sign-in issuer every test home pins (the C0 KAT issuer seed).
pub fn issuer() -> ServerKey {
    let j: Json = serde_json::from_str(super::SIGN_IN_KAT).unwrap();
    let seed = data_encoding::HEXLOWER
        .decode(j["issuer"]["key"]["seed_hex"].as_str().unwrap().as_bytes())
        .unwrap();
    ServerKey::from_seed(&seed.try_into().unwrap())
}

pub fn person(seed: u8) -> (Identity, IdFp) {
    let identity = Identity::from_seed(&[seed; 32]);
    let id = sw_mcp::adapters::CitadelIdentity
        .id_fp(identity.public_key().as_bytes())
        .unwrap();
    (identity, id)
}

/// Project `p` owned by `owner`, with `coworkers` admitted: what the node would hold after
/// open-up and join answers, stored so the node loads it at start.
pub fn project(store: &MemStore, owner: &IdFp, coworkers: &[&IdFp]) {
    let task = TaskState {
        id: "p".into(),
        project: "p".into(),
        parent: None,
        title: "project".into(),
        description: String::new(),
        visibility: sw_model::Visibility::Members,
        coop: Coop {
            owner: owner.clone(),
            status: sw_model::Status::Open,
            tier: Tier::Synchronized {
                policy: JoinPolicy::Manual,
                coworkers: coworkers
                    .iter()
                    .map(|c| (*c).clone())
                    .collect::<BTreeSet<_>>(),
            },
            pending: Vec::new(),
        },
        agents: BTreeMap::new(),
        requests: BTreeMap::new(),
        created_at: now_ms(),
        updated_at: now_ms(),
    };
    let bytes = sw_node::core::task_to_bytes(&task).unwrap();
    store.0.lock().unwrap().insert("task/p".into(), bytes);
}

/// Starts the node on its own thread; returns its WebSocket port and its core.
pub fn start_node(store: MemStore) -> (u16, Arc<Mutex<NodeCore>>) {
    let mut v = vars::complete();
    v.0.insert(
        sw_node::provision::VAR_SIGN_IN_ISSUER_PK,
        issuer().public_key().to_base64url(),
    );
    v.0.insert(
        sw_node::provision::VAR_STUN_SERVERS,
        super::node::LOOPBACK_STUN.join(","),
    );
    let provisioning = Provisioning::from_source(&v).unwrap();
    let ws = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = ws.local_addr().unwrap().port();
    let tcp = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let (core_tx, core_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let kernel = sw_node::start::prepare(&provisioning, store, WallClock)
                .await
                .unwrap();
            core_tx.send(kernel.core()).unwrap();
            let mut builder = citadel_sdk::test_common::node_builder();
            sw_node::start::configure(&mut builder, &provisioning, BackendType::InMemory).unwrap();
            let node = builder
                .with_node_type(NodeType::Server(tcp))
                .with_path_policy(citadel_sdk::test_common::localhost_path_policy())
                .with_bound_websocket_listener(ws)
                .build(kernel)
                .unwrap();
            let _ = node.await;
        });
    });
    (port, core_rx.recv().unwrap())
}

/// A signed-in Device home for `identity` (pinned issuer, fresh keys, an assertion for them).
pub async fn home_for(name: &str, identity: &Identity) -> Home {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("pad-e2e-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    let home = Home::locate(Some(dir)).unwrap();
    home.ensure_layout().unwrap();
    let keys = DeviceKeys::generate(&mut LabelledEntropy::new(&format!("pad-{name}")));
    write_private(&home.device_keys(), &keys.to_bytes().unwrap()).unwrap();
    let issuer = issuer();
    let doc = WellKnownCitadel {
        v: 1,
        canonical_domain: canonical_domain("work.avarok.net").unwrap(),
        citadel_wss: None,
        server_key: issuer.public_key().clone(),
        former_domains: vec![],
        issued_at_s: 1_794_000_000,
    };
    let port = PinOnly::new(issuer.sign_well_known(&doc).unwrap());
    sw_mcp::signin::pin_issuer(&port, &home, "work.avarok.net")
        .await
        .unwrap();
    let assertion = SignInAssertion {
        identity: MlDsa65Pk::from_bytes(identity.public_key().as_bytes()).unwrap(),
        device: keys.device_label("e2e"),
        aud: "sw-mcp".into(),
        exp_s: now_ms() / 1000 + 86_400,
        nonce: [0x42; 32],
    };
    write_private(
        &home.assertion(),
        &issuer.sign_assertion(&assertion).unwrap(),
    )
    .unwrap();
    home
}

/// A daemon on its own runtime; dropping the handle stops it and closes its session.
pub struct Running(Option<tokio::runtime::Runtime>);

impl Running {
    pub fn start(home: &Home, ws_port: u16) -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let daemon = Daemon::open(home.clone(), config(ws_port)).unwrap();
        rt.spawn(async move {
            if let Err(e) = daemon.run().await {
                eprintln!("daemon stopped: {e}");
            }
        });
        Self(Some(rt))
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(rt) = self.0.take() {
            rt.shutdown_background();
        }
    }
}

pub async fn ask(home: &Home, r: Request, blocks_for_s: u64) -> Result<Json, String> {
    call_async(home.socket(), r, blocks_for_s).await
}

/// Retries `r` until it answers Ok and `done` accepts it, within `secs`.
pub async fn until(home: &Home, secs: u64, r: Request, done: impl Fn(&Json) -> bool) -> Json {
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    loop {
        if let Ok(v) = ask(home, r.clone(), 30).await
            && done(&v)
        {
            return v;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "no answer to {r:?} in {secs} s"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
