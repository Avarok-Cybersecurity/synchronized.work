//! What the host sees while the tree moves: every draw it took, when, and which nodes the fold
//! held at that moment; and the reload path, a remount with the link the App saved.

use super::fixture::Fixture;
use super::paced::{Paced, ScriptedMeasure};
use crate::app_link::Step;
use sw_app_core::*;

pub struct Accepted {
    pub at: f64,
    pub commands: Vec<DrawCommand>,
    /// The labels of the tree the fold held when the host took the draw, sorted.
    pub nodes: Vec<String>,
}

/// The labels of the tree the fold holds now (none before a Task is visible), sorted.
pub fn fold_nodes(app: &App<ScriptedMeasure>) -> Vec<String> {
    let Some(live) = app.live() else {
        return Vec::new();
    };
    let state = live.state();
    if state.tasks.is_empty() {
        return Vec::new();
    }
    let name = sw_app_core::graph::project_name(state);
    let mut labels: Vec<String> = sw_view::points::points(state, &name)
        .into_iter()
        .map(|p| p.label)
        .collect();
    labels.sort_unstable();
    labels
}

impl Paced {
    /// One message, one frame interval after the last thing that happened.
    pub fn deliver(&mut self, dt_ms: f32, body: &[u8]) {
        self.tick(dt_ms);
        let at = self.host.borrow().now_ms;
        self.delivered.push(at);
        self.feed(Input::SessionMessage {
            body: body.to_vec(),
        });
        let nodes = fold_nodes(&self.app);
        self.folded.push(nodes);
    }
}

/// The labels of the regions a draw carries, sorted.
pub fn region_labels(commands: &[DrawCommand]) -> Vec<String> {
    let mut labels: Vec<String> = commands
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Region { label, .. } => Some(label.clone()),
            _ => None,
        })
        .collect();
    labels.sort_unstable();
    labels
}

/// The draws taken from `from_ms` until the first one that shows the tree as it ends at rest
/// (the last draw taken before `until_ms`).
pub fn moving(accepted: &[Accepted], from_ms: f64, until_ms: f64) -> &[Accepted] {
    let start = accepted.partition_point(|a| a.at < from_ms);
    let end = accepted.partition_point(|a| a.at < until_ms);
    if start >= end {
        return &[];
    }
    let rest = &accepted[end - 1].commands;
    let first = (start..end)
        .find(|i| accepted[*i].commands == *rest)
        .unwrap_or(end - 1);
    &accepted[start..=first]
}

/// The longest time without a draw the host took, from `from_ms` through `draws`.
pub fn longest_gap(from_ms: f64, draws: &[Accepted]) -> f64 {
    let mut last = from_ms;
    let mut longest = 0.0f64;
    for d in draws {
        longest = longest.max(d.at - last);
        last = d.at;
    }
    longest
}

/// Whether `regions` names every one of `nodes` (both sorted, labels may repeat).
fn names_all(regions: &[String], nodes: &[String]) -> bool {
    let mut left = regions.to_vec();
    nodes.iter().all(|n| {
        let i = left.iter().position(|r| r == n);
        i.map(|i| left.remove(i)).is_some()
    })
}

/// For each message after which the fold held a tree: how long until the host showed a draw
/// naming every node of it (`None`: never), with when it was delivered. A message that left
/// the tree as the host already shows it takes no time.
pub fn shown_after(p: &Paced) -> Vec<(f64, Option<f64>)> {
    p.delivered
        .iter()
        .zip(&p.folded)
        .filter(|(_, nodes)| !nodes.is_empty())
        .map(|(at, nodes)| {
            let i = p.accepted.partition_point(|d| d.at < *at);
            let whole = |d: &&Accepted| names_all(&region_labels(&d.commands), nodes);
            let shown = p.accepted[i.saturating_sub(1)..].iter().find(whole);
            (*at, shown.map(|d| (d.at - at).max(0.0)))
        })
        .collect()
}

/// The fixture's catch-up: what the node delivers after the App's `SUBSCRIBE`.
pub fn catch_up(f: &Fixture) -> Vec<Vec<u8>> {
    let after = f.steps.iter().position(|(_, w)| w == "subscribe").unwrap();
    f.steps[after..]
        .iter()
        .filter_map(|(s, _)| match s {
            Step::Deliver(b) => Some(b.clone()),
            Step::Expect(_) => None,
        })
        .collect()
}

/// A reload: the App mounted again with the link it saved says HELLO and SUBSCRIBE, and the
/// catch-up is folded message by message, one frame interval apart.
pub fn remount(
    f: &Fixture,
    ctx: AppCtx,
    (config, rate): (AppConfig, f64),
    stored: Vec<u8>,
    (dt_ms, settle_ms): (f32, f32),
) -> Paced {
    let mut p = Paced::new(ctx, config, rate);
    let effects = p.app.start();
    p.feed_effects(effects);
    p.feed(Input::LinkLoaded {
        record: Some(stored),
    });
    p.feed(Input::SessionOpened {
        cid: f.cid,
        status: HostSessionStatus::Connected,
    });
    for body in catch_up(f) {
        p.deliver(dt_ms, &body);
    }
    p.settle(dt_ms, settle_ms);
    p
}
