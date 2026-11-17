//! `xtask node-budget <measure.json>`: a tools/measure.sh run is within budget.toml's
//! `[node]` ceilings, at the session count the budget names.

use std::path::Path;

use crate::budget;

pub fn run(root: &Path, args: &[String]) -> Result<String, String> {
    let [file] = args else {
        return Err("usage: xtask node-budget <measure.json>".into());
    };
    let node = budget::table(&budget::load(root)?, "node")?.clone();
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let measured: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{file}: {e}"))?;
    let figure = |key: &str| {
        measured
            .get(key)
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(|| format!("{file}: no `{key}`"))
    };
    let mut problems = Vec::new();
    let sessions = budget::number(&node, "sessions")?;
    if figure("sessions")? < sessions {
        problems.push(format!(
            "measured at {} sessions, the budget is at {sessions}",
            figure("sessions")?
        ));
    }
    let mut ceilings: Vec<(&str, &str)> = budget::CPU_CEILINGS.to_vec();
    ceilings.push(("max_wasm_memory_bytes", "wasm_memory_bytes_at_sessions"));
    for (ceiling, key) in ceilings {
        let (max, got) = (budget::number(&node, ceiling)?, figure(key)?);
        if got > max {
            problems.push(format!("{key} = {got} is over {ceiling} = {max}"));
        }
    }
    if problems.is_empty() {
        Ok(format!(
            "node-budget: PASS ({sessions} sessions, every figure within budget)"
        ))
    } else {
        Err(format!("node-budget:\n  {}", problems.join("\n  ")))
    }
}
