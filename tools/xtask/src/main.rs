//! `cargo run -p xtask -- <gate>`. Each gate prints its verdict and exits non-zero on failure.

mod conformance;
mod file_length;
mod sbio;
mod workspace;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = match workspace::root() {
        Ok(root) => root,
        Err(e) => {
            eprintln!("xtask: {e}");
            return ExitCode::FAILURE;
        }
    };
    let result = match args.first().map(String::as_str) {
        Some("file-length") => file_length::run(&root),
        Some("sbio") => sbio::run(&root),
        Some("conformance") => conformance::run(&root, &args[1..]),
        _ => Err("usage: xtask <file-length | sbio | conformance [--mirror <dir>]>".to_owned()),
    };
    match result {
        Ok(verdict) => {
            println!("{verdict}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("FAIL: {e}");
            ExitCode::FAILURE
        }
    }
}
