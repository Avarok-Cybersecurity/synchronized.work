//! A viewer's [`ProjectState`] as the tree the view draws (design §2.2):
//! `Project → Task (→ Task) → Person → Agent → Progress`. The order is fixed by ids alone,
//! never by arrival, so every viewer — and the App canvas — sees the same tree.

use std::collections::{BTreeMap, BTreeSet};

use sw_client::live::{AgentView, ProjectState, TaskView};
use sw_model::IdFp;
use sw_model::tier::TierName;
use sw_model::{Lane, Status};

use crate::view::{Kind, Point};

pub fn points(state: &ProjectState, project_name: &str) -> Vec<Point> {
    let mut out = vec![Point {
        id: "project".into(),
        parent: None,
        kind: Kind::Project,
        label: project_name.to_owned(),
        detail: format!("{} tasks", state.tasks.len()),
        state: None,
    }];
    let mut children: BTreeMap<Option<&str>, Vec<&TaskView>> = BTreeMap::new();
    for task in state.tasks.values() {
        // A parent the viewer cannot see puts the Task directly under the Project.
        let parent = task
            .parent
            .as_deref()
            .filter(|p| state.tasks.contains_key(*p) && *p != task.id);
        children.entry(parent).or_default().push(task);
    }
    let mut placed = BTreeSet::new();
    let roots = children.get(&None).into_iter().flatten().copied();
    // Tasks whose parents form a cycle are reachable from no root: they go under the Project
    // too, in id order, rather than disappearing.
    let rest: Vec<&TaskView> = state.tasks.values().collect();
    for top in roots.chain(rest) {
        let mut stack = vec![(top, 0)];
        while let Some((task, parent)) = stack.pop() {
            if !placed.insert(task.id.as_str()) {
                continue;
            }
            let at = out.len();
            out.push(task_point(task, parent));
            people(state, task, at, &mut out);
            for child in children
                .get(&Some(task.id.as_str()))
                .into_iter()
                .flatten()
                .rev()
            {
                stack.push((child, at));
            }
        }
    }
    out
}

fn task_point(task: &TaskView, parent: usize) -> Point {
    let tier = task.tier.map(tier_name);
    let status = match task.status {
        Some(Status::Open) => "open",
        Some(Status::Done) => "done",
        Some(Status::Cancelled) => "cancelled",
        None => "status not shown",
    };
    Point {
        id: format!("task:{}", task.id),
        parent: Some(parent),
        kind: Kind::Task,
        label: task
            .title
            .clone()
            .unwrap_or_else(|| "Title hidden by the owner".into()),
        detail: status.into(),
        state: tier.map(str::to_owned),
    }
}

fn people(state: &ProjectState, task: &TaskView, at: usize, out: &mut Vec<Point>) {
    let mut by_person: BTreeMap<&IdFp, Vec<&AgentView>> = BTreeMap::new();
    if let Some(owner) = &task.owner {
        by_person.entry(owner).or_default();
    }
    for agent in state.agents.values().filter(|a| a.task_id == task.id) {
        by_person.entry(&agent.person).or_default().push(agent);
    }
    for (person, agents) in by_person {
        let person_at = out.len();
        let person_id = format!("task:{}/person:{person}", task.id);
        let username = state
            .persons
            .get(person)
            .and_then(|p| p.username.as_deref());
        out.push(Point {
            id: person_id.clone(),
            parent: Some(at),
            kind: Kind::Person,
            label: username.map_or_else(|| person.to_string(), handle),
            detail: if task.owner.as_ref() == Some(person) {
                "owner".into()
            } else {
                "co-worker".into()
            },
            state: None,
        });
        for agent in agents {
            let agent_at = out.len();
            let agent_id = format!("{person_id}/agent:{}", agent.id);
            out.push(Point {
                id: agent_id.clone(),
                parent: Some(person_at),
                kind: Kind::Agent,
                label: agent.id.clone(),
                detail: match agent.online {
                    Some(true) => format!("{}, online", agent.harness),
                    Some(false) => format!("{}, offline", agent.harness),
                    None => agent.harness.clone(),
                },
                state: None,
            });
            out.push(Point {
                id: format!("{agent_id}/progress"),
                parent: Some(agent_at),
                kind: Kind::Progress,
                label: agent
                    .last_line
                    .clone()
                    .unwrap_or_else(|| "No progress yet".into()),
                detail: format!("{} blockers", agent.blockers.len()),
                state: Some(lane_name(agent.lane).into()),
            });
        }
    }
}

/// `<id_fp>.<slug>@<domain>` shows as its slug (the Device's name).
fn handle(username: &str) -> String {
    let local = username.split('@').next().unwrap_or(username);
    local
        .split_once('.')
        .map_or(local, |(_, slug)| slug)
        .to_owned()
}

pub fn tier_name(t: TierName) -> &'static str {
    match t {
        TierName::Off => "off",
        TierName::Aware => "aware",
        TierName::Synchronized => "synchronized",
    }
}

pub fn lane_name(l: Lane) -> &'static str {
    match l {
        Lane::Running => "running",
        Lane::Waiting => "waiting",
        Lane::Blocked => "blocked",
        Lane::Done => "done",
        Lane::Stalled => "stalled",
    }
}
