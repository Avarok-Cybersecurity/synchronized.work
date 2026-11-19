//! The ServerDO (design §3.1): exactly one, `idFromName("node")`, running the Citadel node.
//!
//! The node starts in the object's constructor, under `block_concurrency_while`, so no request
//! reaches the object before it either runs or has refused to start. Unprovisioned, it never
//! starts (PCND): every upgrade is answered 503 and the reason logged. Sockets use the plain,
//! non-hibernating `accept()`: the SDK's sessions and ratchets live in this wasm instance's
//! memory, which hibernation would discard (§3.2 Q1). The object is the only Durable Object
//! class, so the isolate's one wasm instance is never shared with another object's node.

use std::cell::{Cell, RefCell};
use std::net::{Ipv4Addr, SocketAddr};
use std::rc::Rc;

use citadel_sdk::prelude::*;
use worker::{DurableObject, Env, Request, Response, Result, State, WebSocketPair, durable_object};

use crate::clock::IsolateClock;
use crate::env_source::EnvSource;
use crate::node_store::DoNodeStore;
use crate::sql_host::DurableObjectSql;

struct Running {
    injector: WasmConnectionInjector,
    accepted: Cell<u32>,
}

/// What the constructor's start came to.
enum Node {
    Starting,
    Running(Running),
    /// Unprovisioned or misprovisioned; the reason was logged when the start refused.
    Refused,
}

#[durable_object]
pub struct ServerDo {
    node: Rc<RefCell<Node>>,
}

/// `state.storage()` twice: the node's records and the SDK's accounts are two views of the
/// same object storage (prefix `n/` keys and SQL tables), each owned by one adapter.
async fn start(
    records: worker::Storage,
    accounts: worker::Storage,
    env: &Env,
) -> std::result::Result<Running, String> {
    let provisioning =
        sw_node::Provisioning::from_source(&EnvSource(env)).map_err(|e| e.to_string())?;
    let store = DoNodeStore::new(records);
    let kernel = sw_node::start::prepare(&provisioning, store, IsolateClock)
        .await
        .map_err(|e| e.to_string())?;
    let backend = BackendType::HostSql(HostSqlHandle::new(DurableObjectSql::new(accounts)));
    let (injector, listener) = WasmListener::injected();
    let mut builder = NodeBuilder::<StackedRatchet, WasmIO>::default();
    sw_node::start::configure(&mut builder, &provisioning, backend).map_err(|e| e.to_string())?;
    let node = builder
        .with_node_type(NodeType::Server(SocketAddr::from(([0, 0, 0, 0], 0))))
        .with_injected_listener(listener)
        .build(kernel)
        .map_err(|e| e.to_string())?;
    wasm_bindgen_futures::spawn_local(async move {
        match node.await {
            Ok(_) => log::error!(target: "sw_worker", "the node stopped"),
            Err(e) => log::error!(target: "sw_worker", "the node failed: {e:?}"),
        }
    });
    Ok(Running {
        injector,
        accepted: Cell::new(0),
    })
}

impl std::fmt::Debug for ServerDo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerDo").finish_non_exhaustive()
    }
}

impl Running {
    /// A Worker sees no client socket address, but the node keys sessions by one: each
    /// connection gets its own from the shared-address block (100.64.0.0/10), which nothing
    /// routes — the tenant Worker's scheme.
    fn next_peer_addr(&self) -> SocketAddr {
        let n = self.accepted.get().wrapping_add(1);
        self.accepted.set(n);
        let [_, b, c, d] = n.to_be_bytes();
        SocketAddr::from((Ipv4Addr::new(100, 64 | (b & 0x3f), c, d), 1))
    }
}

impl DurableObject for ServerDo {
    fn new(state: State, env: Env) -> Self {
        crate::console::init(&env);
        let node = Rc::new(RefCell::new(Node::Starting));
        let slot = node.clone();
        let (records, accounts) = (state.storage(), state.storage());
        drop(state.block_concurrency_while(async move {
            let outcome = match start(records, accounts, &env).await {
                Ok(running) => Node::Running(running),
                Err(why) => {
                    log::error!(target: "sw_worker", "the node refused to start: {why}");
                    Node::Refused
                }
            };
            *slot.borrow_mut() = outcome;
            Ok::<(), worker::Error>(())
        }));
        Self { node }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        if req.headers().get("upgrade")?.as_deref() != Some("websocket") {
            return Response::error("expected a WebSocket upgrade", 426);
        }
        let node = self.node.borrow();
        let running = match &*node {
            Node::Running(running) => running,
            Node::Refused => return Response::error("the node is not provisioned", 503),
            Node::Starting => return Response::error("the node is starting", 503),
        };
        let pair = WebSocketPair::new()?;
        pair.server.accept()?;
        let ws: web_sys::WebSocket = pair.server.as_ref().clone();
        let stream = WasmWebSocketStream::from_accepted(ws)
            .map_err(|e| worker::Error::RustError(format!("cannot wrap socket: {e}")))?;
        running
            .injector
            .inject(WasmStream::WebSocket(stream), running.next_peer_addr())
            .map_err(|e| worker::Error::RustError(e.to_string()))?;
        // The node's memory as the isolate meters it, for the node-budget measurement.
        let wasm_memory_bytes = core::arch::wasm32::memory_size(0) * 65536;
        log::info!(target: "sw_worker", "accepted={} wasm_memory_bytes={wasm_memory_bytes}", running.accepted.get());
        Response::from_websocket(pair.client)
    }
}
