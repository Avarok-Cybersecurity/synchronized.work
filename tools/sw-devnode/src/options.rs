//! `serve`'s options. Every listener's port can be fixed, so a container can publish it; the
//! server seed can be one of the published test seeds and nothing else.

use std::net::IpAddr;

/// The seed `tools/onboard-e2e.sh` provisions, and the one `serve` uses unless told otherwise.
pub const ONBOARD_SEED_HEX: &str =
    "1111111111111111111111111111111111111111111111111111111111111111";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Serve {
    pub bind: IpAddr,
    /// The WebSocket listener clients dial (`ws://<bind>:<port>/citadel`).
    pub port: u16,
    /// The SDK's TCP listener; 0 picks a free one.
    pub tcp_port: u16,
    /// The `status` listener; 0 picks a free one.
    pub status_port: u16,
    pub server_seed_hex: String,
}

const USAGE: &str = "serve <port> [--bind <ip>] [--tcp-port <port>] [--status-port <port>] \
                     [--server-seed <published test seed hex>]";

fn port(s: &str) -> Result<u16, String> {
    s.parse().map_err(|e| format!("port {s:?}: {e}"))
}

pub fn serve(args: &[&str]) -> Result<Serve, String> {
    let [p, rest @ ..] = args else {
        return Err(format!("usage: {USAGE}"));
    };
    let mut o = Serve {
        bind: IpAddr::from([127, 0, 0, 1]),
        port: port(p)?,
        tcp_port: 0,
        status_port: 0,
        server_seed_hex: ONBOARD_SEED_HEX.into(),
    };
    for pair in rest.chunks(2) {
        match pair {
            ["--bind", ip] => o.bind = ip.parse().map_err(|e| format!("--bind {ip:?}: {e}"))?,
            ["--tcp-port", p] => o.tcp_port = port(p)?,
            ["--status-port", p] => o.status_port = port(p)?,
            ["--server-seed", hex] => o.server_seed_hex = published_seed(hex)?,
            _ => return Err(format!("usage: {USAGE}")),
        }
    }
    Ok(o)
}

/// `hex` when it is a published test seed: the onboarding seed, or a server seed of
/// `fixtures/identity-kat.json`. A test node never runs under any other key.
fn published_seed(hex: &str) -> Result<String, String> {
    let hex = hex.to_ascii_lowercase();
    let kat: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/identity-kat.json"))
            .map_err(|e| e.to_string())?;
    let servers = kat["servers"]
        .as_object()
        .ok_or("the KAT names no servers")?;
    let published = servers
        .values()
        .filter_map(|s| s["key"]["seed_hex"].as_str())
        .chain([ONBOARD_SEED_HEX]);
    match published.into_iter().any(|seed| seed == hex) {
        true => Ok(hex),
        false => Err("--server-seed takes only a published test seed (identity-kat.json)".into()),
    }
}
