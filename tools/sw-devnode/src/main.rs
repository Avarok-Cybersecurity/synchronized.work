//! A test-only synchronized.work node for another repository's CI (never a deployment):
//!
//! `sw-devnode serve <port>` runs the production sw-node kernel natively on
//! `ws://127.0.0.1:<port>/citadel`, in memory, provisioned with wrangler.toml's `[vars]` and
//! these published test values: the server and OPRF seeds of `tools/onboard-e2e.sh`, the C0
//! KAT sign-in issuer (`fixtures/sign-in-kat.json`), loopback STUN. It prints one line,
//! `ready ws://127.0.0.1:<port>/citadel`, once it accepts sessions.
//!
//! `sw-devnode home <dir> <port> <person-seed-hex>` writes a signed-in sw-mcp home for the
//! Person whose identity seed is the 32 bytes given, pointed at that node; it prints the
//! Person's `id_fp`. Then the real `sw-mcp --home <dir>` runs against it.

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
mod home;
#[cfg(not(target_arch = "wasm32"))]
mod serve;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["serve", port] => port_of(port).and_then(serve::run),
        ["home", dir, port, seed] => port_of(port).and_then(|p| home::write(dir, p, seed)),
        _ => Err("usage: sw-devnode serve <port> | home <dir> <port> <person-seed-hex>".into()),
    };
    match outcome {
        Ok(line) => {
            println!("{line}");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("sw-devnode: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn port_of(s: &str) -> Result<u16, String> {
    s.parse().map_err(|e| format!("port {s:?}: {e}"))
}

/// The C0 KAT sign-in issuer: a published test key, never a production one.
#[cfg(not(target_arch = "wasm32"))]
fn kat_issuer() -> Result<citadel_identity::ServerKey, String> {
    let kat: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/sign-in-kat.json"))
            .map_err(|e| e.to_string())?;
    let seed = kat["issuer"]["key"]["seed_hex"]
        .as_str()
        .ok_or("the KAT names no issuer seed")?;
    Ok(citadel_identity::ServerKey::from_seed(&seed32(seed)?))
}

#[cfg(not(target_arch = "wasm32"))]
fn seed32(hex: &str) -> Result<[u8; 32], String> {
    let bytes = data_encoding::HEXLOWER_PERMISSIVE
        .decode(hex.as_bytes())
        .map_err(|e| format!("seed: {e}"))?;
    bytes
        .try_into()
        .map_err(|_| "a seed is 32 bytes (64 hex digits)".into())
}
