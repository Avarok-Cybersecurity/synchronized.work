#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! `SessionPort` over the native SDK: the Device account registers to a WebSocket URL, logs
//! in, and carries bytes both ways over the C2S channel.

mod support;

use std::time::Duration;

use citadel_sdk::prelude::{BackendType, WebSocketEndpoint};
use support::node;
use sw_client::{SessionError, SessionPort as _, SessionState};
use sw_mcp::session::{CitadelSession, SessionSettings};

#[tokio::test(flavor = "multi_thread")]
async fn the_device_registers_logs_in_and_carries_bytes_both_ways() {
    let mut node = node::start();
    let endpoint =
        WebSocketEndpoint::parse(&format!("ws://127.0.0.1:{}/citadel", node.ws_port)).unwrap();
    let settings = SessionSettings {
        endpoint,
        username: "3foriftckvpoy2rh.macbook-pro@synchronized.work".into(),
        password: zeroize::Zeroizing::new("generated-password-0123456789abcdefghijkl".into()),
        backend: BackendType::InMemory,
        reconnect_delay: Duration::from_secs(1),
        stun_servers: node::LOOPBACK_STUN.iter().map(|s| s.to_string()).collect(),
    };
    let (mut session, _done) = CitadelSession::start(settings).unwrap();
    assert_eq!(
        session.send(vec![1]),
        Err(SessionError::NotConnected),
        "nothing is sent before login"
    );
    let watch = session.watcher();
    let cid = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            if let SessionState::Connected { cid } = session.state() {
                return cid;
            }
            watch.changed().await;
        }
    })
    .await
    .expect("connected within a minute");
    assert_ne!(cid, 0);
    session.send(b"to the node".to_vec()).unwrap();
    assert_eq!(node.next(30).await, b"to the node");
    node.to_device.send(b"to the device".to_vec()).unwrap();
    let got = tokio::time::timeout(Duration::from_secs(30), session.recv())
        .await
        .unwrap();
    assert_eq!(got.as_deref(), Some(&b"to the device"[..]));
}
