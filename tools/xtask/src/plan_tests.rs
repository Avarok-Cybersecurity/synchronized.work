#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

fn repo(rel: &str) -> String {
    let root = crate::workspace::root().unwrap();
    std::fs::read_to_string(root.join(rel)).unwrap()
}

fn wrangler() -> Table {
    repo(WRANGLER).parse().unwrap()
}

fn edited(from: &str, to: &str) -> Table {
    let text = repo(WRANGLER);
    assert!(text.contains(from), "the fixture edit must apply");
    text.replacen(from, to, 1).parse().unwrap()
}

#[test]
fn the_shipped_wrangler_toml_and_deploy_yml_pass() {
    assert_eq!(check_wrangler(&wrangler()), Vec::<String>::new());
    assert_eq!(check_deploy(&repo(DEPLOY)), Vec::<String>::new());
}

#[test]
fn a_wrangler_toml_without_limits_cpu_ms_fails() {
    let problems = check_wrangler(&edited("cpu_ms = ", "not_cpu_ms = "));
    assert!(
        problems
            .iter()
            .any(|p| p.contains("limits.cpu_ms is not declared")),
        "{problems:?}"
    );
}

#[test]
fn a_queue_or_logpush_fails() {
    let queued = edited(
        "[limits]",
        "[[queues.producers]]\nqueue = \"q\"\nbinding = \"Q\"\n\n[limits]",
    );
    assert!(
        check_wrangler(&queued)
            .iter()
            .any(|p| p.contains("`queues`"))
    );
    let logpush = edited("workers_dev = false", "workers_dev = false\nlogpush = true");
    assert!(
        check_wrangler(&logpush)
            .iter()
            .any(|p| p.contains("`logpush`"))
    );
}

#[test]
fn a_kv_backed_object_class_fails() {
    let problems = check_wrangler(&edited("new_sqlite_classes", "new_classes"));
    assert!(
        problems.iter().any(|p| p.contains("KV-backed")),
        "{problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("ServerDo has no new_sqlite_classes")),
        "{problems:?}"
    );
}

#[test]
fn a_cpu_ceiling_at_or_over_the_limit_fails() {
    let mut node = Table::new();
    for (ceiling, _) in budget::CPU_CEILINGS {
        node.insert((*ceiling).into(), Value::Float(1.0));
    }
    assert!(check_ceilings(&wrangler(), &node).is_empty());
    node.insert(
        "max_cpu_ms_per_message".into(),
        Value::Integer(cpu_ms(&wrangler()).unwrap()),
    );
    let problems = check_ceilings(&wrangler(), &node);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("max_cpu_ms_per_message")),
        "{problems:?}"
    );
}

#[test]
fn a_deploy_on_pull_requests_or_off_main_fails() {
    let deploy = repo(DEPLOY);
    let on_prs = deploy.replacen("on:\n", "on:\n  pull_request:\n", 1);
    assert!(
        check_deploy(&on_prs)
            .iter()
            .any(|p| p.contains("pull_request"))
    );
    let off_main = deploy.replacen("branches: [main]", "branches: [release]", 1);
    assert!(
        check_deploy(&off_main)
            .iter()
            .any(|p| p.contains("push to main"))
    );
}
