//! `xtask plan`: wrangler.toml declares the Workers Paid features the design relies on
//! (D11: `limits.cpu_ms`, SQLite-backed Durable Objects for every binding) and none it rules
//! out (queues, logpush, KV-backed object classes); every per-event CPU ceiling in the
//! budget sits below `limits.cpu_ms`; and deploy.yml deploys on a push to main, never a PR.

use std::path::Path;

use toml::{Table, Value};

use crate::budget;

pub const WRANGLER: &str = "wrangler.toml";
pub const DEPLOY: &str = ".github/workflows/deploy.yml";
/// The Paid plan's ceiling for `limits.cpu_ms`.
const PAID_MAX_CPU_MS: i64 = 300_000;

pub fn run(root: &Path) -> Result<String, String> {
    let read =
        |rel: &str| std::fs::read_to_string(root.join(rel)).map_err(|e| format!("{rel}: {e}"));
    let wrangler = read(WRANGLER)?
        .parse::<Table>()
        .map_err(|e| format!("{WRANGLER}: {e}"))?;
    let mut problems = check_wrangler(&wrangler);
    let ceilings = budget::table(&budget::load(root)?, "node")?.clone();
    problems.extend(check_ceilings(&wrangler, &ceilings));
    problems.extend(check_deploy(&read(DEPLOY)?));
    if problems.is_empty() {
        Ok(
            "plan: PASS (Paid features declared, budget under limits.cpu_ms, deploy on main only)"
                .into(),
        )
    } else {
        Err(format!("plan:\n  {}", problems.join("\n  ")))
    }
}

fn cpu_ms(w: &Table) -> Option<i64> {
    w.get("limits")?.get("cpu_ms")?.as_integer()
}

fn strings(v: Option<&Value>) -> impl Iterator<Item = &str> {
    v.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
}

pub fn check_wrangler(w: &Table) -> Vec<String> {
    let mut problems = Vec::new();
    match cpu_ms(w) {
        Some(ms) if (1..=PAID_MAX_CPU_MS).contains(&ms) => {}
        Some(ms) => problems.push(format!(
            "limits.cpu_ms = {ms} is outside 1..={PAID_MAX_CPU_MS}"
        )),
        None => problems
            .push("limits.cpu_ms is not declared (a Paid-plan feature the node relies on)".into()),
    }
    let migrations = w
        .get("migrations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let sqlite: Vec<&str> = migrations
        .iter()
        .flat_map(|m| strings(m.get("new_sqlite_classes")))
        .collect();
    for m in &migrations {
        for class in strings(m.get("new_classes")) {
            problems.push(format!(
                "migration declares {class} KV-backed (new_classes); it must be SQLite"
            ));
        }
    }
    let bindings = w
        .get("durable_objects")
        .and_then(|d| d.get("bindings"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if bindings.is_empty() {
        problems.push("no durable_objects.bindings: the node has no ServerDO".into());
    }
    for b in &bindings {
        let class = b
            .get("class_name")
            .and_then(Value::as_str)
            .unwrap_or("<unnamed>");
        if !sqlite.contains(&class) {
            problems.push(format!(
                "Durable Object class {class} has no new_sqlite_classes migration"
            ));
        }
    }
    for forbidden in ["queues", "logpush", "tail_consumers"] {
        if w.contains_key(forbidden) {
            problems.push(format!(
                "`{forbidden}` is declared; the design rules it out"
            ));
        }
    }
    problems
}

pub fn check_ceilings(w: &Table, node: &Table) -> Vec<String> {
    let Some(limit) = cpu_ms(w) else {
        return Vec::new();
    };
    budget::CPU_CEILINGS
        .iter()
        .filter_map(|(ceiling, _)| match budget::number(node, ceiling) {
            Ok(ms) if ms < limit as f64 => None,
            Ok(ms) => Some(format!(
                "budget {ceiling} = {ms} is not below limits.cpu_ms = {limit}"
            )),
            Err(e) => Some(e),
        })
        .collect()
}

/// deploy.yml's triggers, read as lines: a `pull_request` trigger anywhere, or no push to
/// main, fails.
pub fn check_deploy(yml: &str) -> Vec<String> {
    let code: Vec<&str> = yml
        .lines()
        .map(|l| l.split('#').next().unwrap_or_default())
        .collect();
    let mut problems = Vec::new();
    if code.iter().any(|l| l.contains("pull_request")) {
        problems.push(format!(
            "{DEPLOY} has a pull_request trigger: a PR must never deploy"
        ));
    }
    if !code.iter().any(|l| l.trim() == "branches: [main]") {
        problems.push(format!(
            "{DEPLOY} does not deploy on a push to main (`branches: [main]`)"
        ));
    }
    problems
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
