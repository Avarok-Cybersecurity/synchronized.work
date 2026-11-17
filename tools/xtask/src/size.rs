//! `xtask size <dir>`: the gzip of what wrangler would upload (`wrangler deploy --dry-run
//! --outdir <dir>`) is within budget.toml's `[bundle] max_gzip_bytes`.
//! `xtask site-size <dir>`: the gzip of every file of the built site (`site/dist`, recursively)
//! is within `[site] max_gzip_bytes`. Both measure with the system `gzip -9`; wrangler's
//! README and source maps are not uploaded and are skipped.

use std::path::Path;
use std::process::Command;

use crate::budget;

/// One gate: which budget table bounds it and whether to descend into directories.
struct Gate {
    name: &'static str,
    table: &'static str,
    recursive: bool,
}

const BUNDLE: Gate = Gate {
    name: "size",
    table: "bundle",
    recursive: false,
};
const SITE: Gate = Gate {
    name: "site-size",
    table: "site",
    recursive: true,
};

pub fn run(root: &Path, args: &[String]) -> Result<String, String> {
    check(root, args, &BUNDLE)
}

pub fn site(root: &Path, args: &[String]) -> Result<String, String> {
    check(root, args, &SITE)
}

fn check(root: &Path, args: &[String], gate: &Gate) -> Result<String, String> {
    let [dir] = args else {
        return Err(format!("usage: xtask {} <dir>", gate.name));
    };
    let max = budget::number(
        budget::table(&budget::load(root)?, gate.table)?,
        "max_gzip_bytes",
    )? as u64;
    let (files, total) = measure(Path::new(dir), gate.recursive)?;
    if files == 0 {
        return Err(format!("{}: {dir} holds nothing to measure", gate.name));
    }
    if total <= max {
        Ok(format!(
            "{}: PASS ({files} files, {total} bytes gzip <= {max}, headroom {})",
            gate.name,
            max - total
        ))
    } else {
        Err(format!(
            "{}: {dir} is {total} bytes gzip, over the budget of {max}",
            gate.name
        ))
    }
}

fn measure(dir: &Path, recursive: bool) -> Result<(u64, u64), String> {
    let (mut files, mut total) = (0, 0);
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if path.is_dir() {
            if recursive {
                let (f, t) = measure(&path, true)?;
                files += f;
                total += t;
            }
            continue;
        }
        if name == "README.md" || name.ends_with(".map") {
            continue;
        }
        total += gzip_len(&path)?;
        files += 1;
    }
    Ok((files, total))
}

fn gzip_len(path: &Path) -> Result<u64, String> {
    let out = Command::new("gzip")
        .args(["-9", "-c"])
        .arg(path)
        .output()
        .map_err(|e| format!("gzip: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "gzip {}: {}",
            path.display(),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(out.stdout.len() as u64)
}
