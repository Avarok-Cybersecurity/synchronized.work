//! A stand-in for the synchronized.work node: a real Citadel SDK server, in-process, listening
//! for WebSocket clients on loopback, whose kernel hands every message a client sends to the
//! test and sends the client whatever the test queues.
//!
//! Stand-in, justified: sw-node (lane S2) is not built. Everything on the Device's side — the
//! SDK client, registration, login, the C2S channel, the wire, the gate — is the production
//! code; the test plays the node's relaying role with the bytes the contract fixes.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use citadel_sdk::prefabs::server::client_connect_listener::ClientConnectListenerKernel;
use citadel_sdk::prelude::*;
use futures::StreamExt as _;
use tokio::sync::{Mutex, mpsc};

/// Three loopback STUN endpoints nothing answers on (the SDK requires three).
pub const LOOPBACK_STUN: [&str; 3] = ["127.0.0.1:3478", "127.0.0.1:3479", "127.0.0.1:3480"];

pub struct StandInNode {
    pub ws_port: u16,
    pub from_device: mpsc::UnboundedReceiver<Vec<u8>>,
    pub to_device: mpsc::UnboundedSender<Vec<u8>>,
}

impl StandInNode {
    /// The next message a client sent, within `secs`.
    pub async fn next(&mut self, secs: u64) -> Vec<u8> {
        tokio::time::timeout(
            std::time::Duration::from_secs(secs),
            self.from_device.recv(),
        )
        .await
        .expect("the device sent nothing in time")
        .expect("the stand-in node stopped")
    }
}

fn free_port() -> std::net::SocketAddr {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap()
}

pub fn start() -> StandInNode {
    let (from_tx, from_device) = mpsc::unbounded_channel::<Vec<u8>>();
    let (to_device, to_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let to_rx = Arc::new(Mutex::new(to_rx));
    let ws = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let ws_port = ws.local_addr().unwrap().port();
    let tcp = free_port();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let kernel = ClientConnectListenerKernel::<_, _, StackedRatchet>::new(
                move |mut conn: CitadelClientServerConnection<StackedRatchet>| {
                    let from_tx = from_tx.clone();
                    let to_rx = to_rx.clone();
                    async move {
                        let (mut tx, mut rx) = conn.take_channel().expect("a channel").split();
                        let mut to_rx = to_rx.lock().await;
                        loop {
                            tokio::select! {
                                m = rx.next() => match m {
                                    Some(m) => { let _ = from_tx.send(m.as_ref().to_vec()); }
                                    None => return Ok(()),
                                },
                                out = to_rx.recv() => match out {
                                    Some(b) => tx.send(SecBuffer::from(b)).await?,
                                    None => return Ok(()),
                                },
                            }
                        }
                    }
                },
            );
            let node = DefaultNodeBuilder::default()
                .with_node_type(NodeType::Server(tcp))
                .with_backend(BackendType::InMemory)
                .with_stun_servers(LOOPBACK_STUN)
                .with_path_policy(citadel_sdk::test_common::localhost_path_policy())
                .with_server_misc_settings(sw_mcp::session::v2_account_rules().unwrap())
                .with_bound_websocket_listener(ws)
                .build(kernel)
                .unwrap();
            let _ = node.await;
        });
    });
    StandInNode {
        ws_port,
        from_device,
        to_device,
    }
}
