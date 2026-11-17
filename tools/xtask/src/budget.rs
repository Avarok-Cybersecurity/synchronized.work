//! `tools/xtask/budget.toml`: the ceilings the size and node-budget gates hold the Worker to.

use std::path::Path;

use toml::Table;

pub const BUDGET_FILE: &str = "tools/xtask/budget.toml";

pub fn load(root: &Path) -> Result<Table, String> {
    let path = root.join(BUDGET_FILE);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    text.parse::<Table>()
        .map_err(|e| format!("{BUDGET_FILE}: {e}"))
}

pub fn table<'a>(t: &'a Table, key: &str) -> Result<&'a Table, String> {
    t.get(key)
        .and_then(|v| v.as_table())
        .ok_or_else(|| format!("{BUDGET_FILE}: no [{key}] table"))
}

/// A required number (integer or float), never defaulted.
pub fn number(t: &Table, key: &str) -> Result<f64, String> {
    match t.get(key) {
        Some(toml::Value::Integer(i)) => Ok(*i as f64),
        Some(toml::Value::Float(f)) => Ok(*f),
        _ => Err(format!("{BUDGET_FILE}: `{key}` is missing or not a number")),
    }
}

/// The per-event CPU ceilings, by the measurement key each one bounds.
pub const CPU_CEILINGS: &[(&str, &str)] = &[
    ("max_cpu_ms_per_register", "cpu_ms_per_register"),
    ("max_cpu_ms_per_sign_in", "cpu_ms_per_sign_in"),
    ("max_cpu_ms_per_enrol", "cpu_ms_per_enrol"),
    ("max_cpu_ms_per_message", "cpu_ms_per_message"),
];
