//! SBIO (design §10): the pure crates depend on no I/O or ambient-randomness crate, directly
//! or transitively, on either target, and their sources name no I/O module.

use std::path::Path;
use std::process::Command;

use crate::workspace::{PURE_CRATES, walk};

/// Crates that perform I/O, reach a platform, or draw OS randomness. `sw-testkit` is here
/// because its values are deterministic stand-ins, never fit for a shipped path.
const FORBIDDEN_CRATES: &[&str] = &[
    "web-sys",
    "js-sys",
    "wasm-bindgen",
    "worker",
    "tokio",
    "mio",
    "async-std",
    "reqwest",
    "hyper",
    "citadel_sdk",
    "citadel_proto",
    "citadel_crypt",
    "getrandom",
    "rand",
    "sw-testkit",
];

/// Source patterns that reach the OS from a pure crate.
const FORBIDDEN_PATHS: &[&str] = &[
    "std::fs",
    "std::net",
    "std::process",
    "std::env",
    "std::time",
    "std::thread",
    "std::io",
    "println!",
    "eprintln!",
];

const TARGETS: &[&str] = &["wasm32-unknown-unknown", "all"];

pub fn run(root: &Path) -> Result<String, String> {
    let mut problems = Vec::new();
    for krate in PURE_CRATES {
        for target in TARGETS {
            for dep in normal_deps(root, krate, target)? {
                if FORBIDDEN_CRATES.contains(&dep.as_str()) {
                    problems.push(format!("{krate} depends on {dep} (target {target})"));
                }
            }
        }
        let mut files = Vec::new();
        walk(&root.join("crates").join(krate).join("src"), &mut files)?;
        for file in files
            .iter()
            .filter(|f| f.extension().is_some_and(|e| e == "rs"))
        {
            let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
            for (n, line) in text.lines().enumerate() {
                let code = line.split("//").next().unwrap_or_default();
                for pattern in FORBIDDEN_PATHS.iter().filter(|p| code.contains(**p)) {
                    let rel = file.strip_prefix(root).unwrap_or(file);
                    problems.push(format!("{}:{}: uses {pattern}", rel.display(), n + 1));
                }
            }
        }
    }
    if problems.is_empty() {
        Ok(format!(
            "sbio: PASS ({} pure crates, both targets)",
            PURE_CRATES.len()
        ))
    } else {
        Err(format!("sbio:\n  {}", problems.join("\n  ")))
    }
}

/// The package names in `krate`'s normal (non-dev, non-build) dependency tree.
fn normal_deps(root: &Path, krate: &str, target: &str) -> Result<Vec<String>, String> {
    let cargo =
        std::env::var("CARGO").map_err(|_| "CARGO is unset: run through `cargo run -p xtask`")?;
    let output = Command::new(cargo)
        .current_dir(root)
        .args([
            "tree", "-p", krate, "-e", "normal", "--prefix", "none", "--format", "{p}",
        ])
        .args(["--target", target])
        .output()
        .map_err(|e| format!("cargo tree: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo tree -p {krate}: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .map(str::to_owned)
        .collect())
}
