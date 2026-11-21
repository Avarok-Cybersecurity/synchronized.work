//! The measurement: register, sign in and enrol N accounts, then send M messages on one
//! session; sample workerd between the phases. The last message (opening the task up) makes
//! the node push a `NOTIFY` back on the same session, so the sample is taken only after the
//! node has processed everything before it.

use std::time::Duration;

use citadel_identity::ServerKey;
use citadel_io::WebSocketEndpoint;
use citadel_sdk::prelude::*;
use sw_bench::client::{Account, Client, Dial, register};
use sw_bench::kernel::ClientKernel;
use sw_bench::payloads::{create, open_up, retitle};
use sw_bench::probe::{Sample, sample};
use sw_protocol::body::Audience;

pub fn issuer(seed_hex: &str) -> Result<ServerKey, String> {
    let bytes = data_encoding::HEXLOWER_PERMISSIVE
        .decode(seed_hex.as_bytes())
        .map_err(|e| e.to_string())?;
    let seed: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "the seed is not 32 bytes".to_owned())?;
    Ok(ServerKey::from_seed(&seed))
}

fn count(text: &str) -> Result<u32, String> {
    text.parse().map_err(|e| format!("{text}: {e}"))
}

pub fn measure(
    url: &str,
    seed: &str,
    sessions: &str,
    messages: &str,
    out: &str,
) -> Result<String, String> {
    let url = WebSocketEndpoint::parse(url).map_err(|e| format!("{e:?}"))?;
    let issuer = issuer(seed)?;
    let (sessions, messages) = (count(sessions)?, count(messages)?);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let samples = runtime.block_on(drive(Dial::Ws(url), issuer, sessions, messages))?;
    let [base, registered, signed_in, enrolled, created, sent] = samples;
    let per = |a: Sample, b: Sample, n: u32| (b.cpu_ms - a.cpu_ms) as f64 / f64::from(n.max(1));
    let report = serde_json::json!({
        "sessions": sessions,
        "messages": messages,
        "cpu_ms_per_register": per(base, registered, sessions),
        "cpu_ms_per_sign_in": per(registered, signed_in, sessions),
        "cpu_ms_per_enrol": per(signed_in, enrolled, sessions),
        "cpu_ms_per_message": per(created, sent, messages + 1),
        "rss_kib_baseline": base.rss_kib,
        "rss_kib_at_sessions": enrolled.rss_kib,
    });
    std::fs::write(out, report.to_string()).map_err(|e| format!("{out}: {e}"))?;
    Ok(report.to_string())
}

async fn drive(
    dial: Dial,
    issuer: ServerKey,
    sessions: u32,
    messages: u32,
) -> Result<[Sample; 6], String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let max_level = citadel_sdk::types::crypto::MaxSecurityLevel::new(SecurityLevel::Standard)
        .map_err(|e| e.to_string())?;
    let client = DefaultNodeBuilder::default()
        .with_link_manager(LinkManagerMode::Legacy)
        .with_max_security_level(max_level)
        .with_stun_servers([
            "stun.cloudflare.com:3478",
            "stun1.l.google.com:19302",
            "stun4.l.google.com:19302",
        ])
        .with_backend(BackendType::InMemory)
        .build(ClientKernel::new(
            move |remote: NodeRemote<StackedRatchet>| async move {
                let result = phases(&remote, &dial, &issuer, sessions, messages).await;
                let _sent = tx.send(result);
                remote.shutdown().await
            },
        ))
        .map_err(|e| e.to_string())?;
    client.await.map_err(|e| format!("{e:?}"))?;
    rx.await.map_err(|e| e.to_string())?
}

async fn phases(
    remote: &NodeRemote<StackedRatchet>,
    dial: &Dial,
    issuer: &ServerKey,
    sessions: u32,
    messages: u32,
) -> Result<[Sample; 6], String> {
    let e = |e: NetworkError| format!("{e:?}");
    let base = sample()?;
    for n in 0..sessions {
        register(remote, dial, &Account::numbered(n))
            .await
            .map_err(e)?;
    }
    let registered = sample()?;
    let mut clients = Vec::new();
    for n in 0..sessions {
        clients.push(
            Client::sign_in(remote, dial, Account::numbered(n))
                .await
                .map_err(e)?,
        );
    }
    let signed_in = sample()?;
    for c in &mut clients {
        c.enrol(issuer).await.map_err(e)?;
    }
    tokio::time::sleep(Duration::from_secs(2)).await;
    let enrolled = sample()?;
    let first = clients.first_mut().ok_or("no sessions")?;
    first
        .send(Audience::Node, create("bench", "bench task"))
        .await
        .map_err(e)?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    let created = sample()?;
    for m in 0..messages {
        first
            .send(Audience::Node, retitle("bench", &format!("bench task {m}")))
            .await
            .map_err(e)?;
    }
    first
        .send(Audience::Node, open_up("bench"))
        .await
        .map_err(e)?;
    tokio::time::timeout(Duration::from_secs(120), first.next_notify())
        .await
        .map_err(|_| "the node never answered the last message".to_owned())?
        .map_err(e)?;
    let sent = sample()?;
    Ok([base, registered, signed_in, enrolled, created, sent])
}
