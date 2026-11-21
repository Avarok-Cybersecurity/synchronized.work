//! The daemon's side of [`crate::ipc`]: accept, read one request line, hand it to the daemon's
//! task with a reply channel, write the one answer line.

use std::time::Duration;

use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::sync::{mpsc, oneshot};

use crate::ipc::transport::Conn;
use crate::ipc::{Listener, Reply, Request, Transport};

/// The longest request line accepted (a progress line or a feed request is far shorter).
const MAX_REQUEST_BYTES: usize = 64 * 1024;

pub(super) type Inbound = mpsc::UnboundedSender<(Request, oneshot::Sender<Reply>)>;

/// Binds the endpoint (owner-only, [`Transport::bind`]) and serves it on a task. Refuses to
/// start if another daemon is already answering on it.
pub(super) fn bind<T: Transport>(endpoint: &T, to_daemon: Inbound) -> Result<(), String> {
    let mut listener = endpoint.bind()?;
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok(stream) => {
                    let to_daemon = to_daemon.clone();
                    tokio::spawn(async move {
                        if let Err(e) = serve_one(stream, to_daemon).await {
                            log::warn!("ipc client: {e}");
                        }
                    });
                }
                Err(e) => {
                    log::error!("ipc accept: {e}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    });
    Ok(())
}

async fn serve_one(stream: impl Conn, to_daemon: Inbound) -> Result<(), String> {
    let (read, mut write) = tokio::io::split(stream);
    let mut line = String::new();
    let mut reader = BufReader::new(read).take(MAX_REQUEST_BYTES as u64);
    reader
        .read_line(&mut line)
        .await
        .map_err(|e| e.to_string())?;
    let reply = match serde_json::from_str::<Request>(&line) {
        Ok(request) => {
            let (tx, rx) = oneshot::channel();
            to_daemon
                .send((request, tx))
                .map_err(|_| "the daemon stopped".to_owned())?;
            // A client that hangs up while it waits drops `rx`, so the daemon sees the reply
            // channel closed and never hands that client an event it cannot receive.
            let mut probe = [0u8; 1];
            tokio::select! {
                answer = rx => answer.map_err(|_| "the daemon dropped the request".to_owned())?,
                _ = reader.read(&mut probe) => return Ok(()),
            }
        }
        Err(e) => Reply::Error(format!("not a request: {e}")),
    };
    let mut out = serde_json::to_string(&reply).map_err(|e| e.to_string())?;
    out.push('\n');
    write
        .write_all(out.as_bytes())
        .await
        .map_err(|e| e.to_string())
}
