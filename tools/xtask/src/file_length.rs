//! Every source file at most 250 lines (design §10).

use std::path::Path;

use crate::workspace::walk;

pub const MAX_LINES: usize = 250;
const SOURCE_EXTENSIONS: &[&str] = &["rs", "toml", "yml", "yaml", "sh", "js", "mjs", "ts", "tsx"];

pub fn run(root: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    walk(root, &mut files)?;
    let mut checked = 0;
    let mut over = Vec::new();
    for path in files {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        if !SOURCE_EXTENSIONS.contains(&ext) {
            continue;
        }
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        checked += 1;
        let lines = text.lines().count();
        if lines > MAX_LINES {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            over.push(format!(
                "{} has {lines} lines (max {MAX_LINES})",
                rel.display()
            ));
        }
    }
    if over.is_empty() {
        Ok(format!(
            "file-length: PASS ({checked} source files, all <= {MAX_LINES} lines)"
        ))
    } else {
        Err(format!("file-length:\n  {}", over.join("\n  ")))
    }
}
