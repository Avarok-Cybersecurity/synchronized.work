//! `status`: what the running node holds, as one JSON line `{"accounts":N,"sessions":M}`,
//! served on its own listener at the bind address and read by `sw-devnode status <addr>`.
//!
//! The node's kernel is wrapped, not changed: the wrapper keeps the SDK's remote (whose account
//! manager counts the registered accounts) and counts the C2S sessions in progress, each of
//! which the kernel serves inside one `ConnectSuccess` event.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use citadel_sdk::prelude::*;

#[derive(Default)]
pub struct Status {
    remote: Mutex<Option<NodeRemote<StackedRatchet>>>,
    sessions: AtomicUsize,
}

pub struct Counted<K> {
    pub inner: K,
    pub status: Arc<Status>,
}

#[async_trait]
impl<K: NetKernel<StackedRatchet>> NetKernel<StackedRatchet> for Counted<K> {
    fn load_remote(&mut self, remote: NodeRemote<StackedRatchet>) -> Result<(), NetworkError> {
        let mut held = self
            .status
            .remote
            .lock()
            .map_err(|_| NetworkError::msg("the status lock was poisoned"))?;
        *held = Some(remote.clone());
        self.inner.load_remote(remote)
    }
    async fn on_start(&self) -> Result<(), NetworkError> {
        self.inner.on_start().await
    }
    async fn on_node_event_received(
        &self,
        message: NodeResult<StackedRatchet>,
    ) -> Result<(), NetworkError> {
        let session = matches!(message, NodeResult::ConnectSuccess(_));
        if session {
            self.status.sessions.fetch_add(1, Ordering::SeqCst);
        }
        let outcome = self.inner.on_node_event_received(message).await;
        if session {
            self.status.sessions.fetch_sub(1, Ordering::SeqCst);
        }
        outcome
    }
    async fn on_stop(&mut self) -> Result<(), NetworkError> {
        self.inner.on_stop().await
    }
}

impl Status {
    async fn json(&self) -> Result<String, String> {
        let remote = self.remote.lock().map_err(|_| "poisoned")?.clone();
        let remote = remote.ok_or("the node has not started")?;
        let cids = remote
            .account_manager()
            .get_registered_impersonal_cids(None)
            .await
            .map_err(|e| format!("{e:?}"))?;
        let accounts = cids.map_or(0, |c| c.len());
        let sessions = self.sessions.load(Ordering::SeqCst);
        Ok(serde_json::json!({ "accounts": accounts, "sessions": sessions }).to_string())
    }
}

/// Answers every connection to `listener` with the JSON line, on its own thread.
pub fn serve(listener: TcpListener, status: Arc<Status>, rt: tokio::runtime::Handle) {
    drop(std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let line = rt
                .block_on(status.json())
                .unwrap_or_else(|e| serde_json::json!({ "error": e }).to_string());
            let mut stream = stream;
            if let Err(e) = writeln!(stream, "{line}") {
                eprintln!("sw-devnode: status: {e}");
            }
        }
    }));
}

/// `sw-devnode status <host:port>`.
pub fn read(addr: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect(addr).map_err(|e| format!("{addr}: {e}"))?;
    let mut line = String::new();
    stream
        .read_to_string(&mut line)
        .map_err(|e| format!("{addr}: {e}"))?;
    Ok(line.trim_end().to_owned())
}
