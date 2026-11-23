use std::path::{Path, PathBuf};

/// The repository root: two levels above this crate's manifest.
pub fn root() -> Result<PathBuf, String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "cannot locate the workspace root".to_owned())
}

/// The crates the sbio gate holds to "no I/O, no ambient randomness" (design §10).
pub const PURE_CRATES: &[&str] = &[
    "sw-model",
    "sw-protocol",
    "sw-crypto",
    "sw-client",
    "sw-graph",
    "sw-view",
    "sw-app-core",
];

/// Every file under `dir`, skipping build output and VCS metadata.
pub fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name();
        if matches!(
            name.to_str(),
            Some(
                "target"
                    | ".git"
                    | "node_modules"
                    | ".wrangler"
                    | "build"
                    | "dist"
                    | "pkg"
                    | "test-results"
                    | "playwright-report"
            )
        ) {
            continue;
        }
        if path.is_dir() {
            walk(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}
