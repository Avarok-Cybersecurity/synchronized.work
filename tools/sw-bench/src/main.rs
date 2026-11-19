//! `sw-bench issuer-pk <seed-hex>` prints the issuer key a node must be provisioned with.
//! `sw-bench run <ws-url> <issuer-seed-hex> <sessions> <messages> <out.json>` measures a node
//! running under `wrangler dev` on this machine and writes the figures as JSON.

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
mod run;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["issuer-pk", seed] => run::issuer(seed).map(|key| key.public_key().to_base64url()),
        ["run", url, seed, sessions, messages, out] => run::measure(url, seed, sessions, messages, out),
        _ => Err("usage: sw-bench issuer-pk <seed-hex> | run <ws-url> <issuer-seed-hex> <sessions> <messages> <out.json>".into()),
    };
    match outcome {
        Ok(line) => {
            println!("{line}");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("sw-bench: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
