#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The daemon's endpoint on this platform (a Unix socket, or on Windows a named pipe), through
//! the production `Transport`, `call` and `call_async`: a request crosses and its answer comes
//! back, from async code and from synchronous CLI code; a second daemon on the same home is
//! refused while the first holds the endpoint, and allowed once it has let go (the control).

use std::path::PathBuf;

use serde_json::{Value, json};
use sw_mcp::home::Home;
use sw_mcp::ipc::{Listener as _, Reply, Request, Transport as _, call, call_async};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

fn home(name: &str) -> Home {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let home = Home::locate(Some(dir)).unwrap();
    home.ensure_layout().unwrap();
    home
}

/// A stand-in daemon: answers every request with the request it read, `n` times.
async fn echo(home: &Home, n: usize) -> tokio::task::JoinHandle<()> {
    let mut listener = home.ipc().bind().unwrap();
    tokio::spawn(async move {
        for _ in 0..n {
            let conn = listener.accept().await.unwrap();
            let (read, mut write) = tokio::io::split(conn);
            let mut line = String::new();
            BufReader::new(read).read_line(&mut line).await.unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            let mut out = serde_json::to_string(&Reply::Ok(json!({ "echo": request }))).unwrap();
            out.push('\n');
            write.write_all(out.as_bytes()).await.unwrap();
        }
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_crosses_and_its_answer_comes_back() {
    let home = home("ipc-round-trip");
    let server = echo(&home, 3).await;
    for _ in 0..2 {
        let answer = call_async(home.ipc().clone(), Request::GateList, 0)
            .await
            .unwrap();
        assert_eq!(answer, json!({ "echo": { "op": "gate_list" } }));
    }
    // The CLI's synchronous path, from a plain thread as `main` runs it.
    let endpoint = home.ipc().clone();
    let answer = tokio::task::spawn_blocking(move || {
        std::thread::spawn(move || call(&endpoint, &Request::ProjectList, 0))
            .join()
            .unwrap()
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(answer, json!({ "echo": { "op": "project_list" } }));
    server.await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_daemon_on_the_same_home_is_refused() {
    let home = home("ipc-second-daemon");
    let first = home.ipc().bind().unwrap();
    let err = home.ipc().bind().unwrap_err();
    assert!(err.contains("already running"), "{err}");
    // Control: once the first daemon lets go, the endpoint can be taken again.
    drop(first);
    home.ipc().bind().unwrap();
}

#[tokio::test]
async fn no_daemon_is_reported_as_not_running() {
    let home = home("ipc-no-daemon");
    let err = call_async(home.ipc().clone(), Request::GateList, 0)
        .await
        .unwrap_err();
    assert!(err.contains("not running"), "{err}");
}

#[tokio::test]
async fn the_synchronous_call_refuses_to_run_inside_a_runtime() {
    let home = home("ipc-sync-in-async");
    let err = call(home.ipc(), &Request::GateList, 0).unwrap_err();
    assert!(err.contains("call_async"), "{err}");
}
