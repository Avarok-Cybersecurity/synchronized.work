//! `xtask size <dir>`: the gzip of what wrangler would upload (`wrangler deploy --dry-run
//! --outdir <dir>`) is within budget.toml's `[bundle] max_gzip_bytes`. Measured with the
//! system `gzip -9`; wrangler's README and source maps are not uploaded and are skipped.

use std::path::Path;
use std::process::Command;

use crate::budget;

pub fn run(root: &Path, args: &[String]) -> Result<String, String> {
    let [dir] = args else {
        return Err("usage: xtask size <wrangler --outdir dir>".into());
    };
    let max = budget::number(
        budget::table(&budget::load(root)?, "bundle")?,
        "max_gzip_bytes",
    )? as u64;
    let mut total = 0u64;
    let mut files = 0;
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{dir}: {e}"))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name == "README.md" || name.ends_with(".map") || path.is_dir() {
            continue;
        }
        total += gzip_len(&path)?;
        files += 1;
    }
    if files == 0 {
        return Err(format!("size: {dir} holds no bundle"));
    }
    if total <= max {
        Ok(format!(
            "size: PASS ({files} files, {total} bytes gzip <= {max})"
        ))
    } else {
        Err(format!(
            "size: the bundle is {total} bytes gzip, over the budget of {max}"
        ))
    }
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
