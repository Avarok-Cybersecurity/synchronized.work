//! What the daemon knows of the projects it subscribed to: the node's visibility-filtered
//! `EVENT` stream (and `NOTIFY`s, and `PROGRESS_TAIL`s when the P2P path delivers them) folded
//! into project, task and agent views. Pure; the status tools read it, `read_task_feed` frames
//! the parts other people wrote.

use std::collections::{BTreeMap, VecDeque};

use serde::Serialize;
use sw_model::Lane;
use sw_protocol::payload::{
    Event, Notify, ProgressEvent, ProgressTail, StatusUpdate, TaskOp, TaskWrite,
};
use sw_protocol::wire::Wire;

use crate::frame::FeedItem;

/// Progress lines kept per agent, and feed items kept per project.
pub const KEPT_LINES: usize = 64;
pub const KEPT_FEED: usize = 256;

#[derive(Debug, Clone, Default, Serialize)]
pub struct TaskView {
    pub title: Option<String>,
    pub tier: Option<String>,
    pub status: Option<String>,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgressLine {
    pub at: u64,
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentView {
    pub task_id: String,
    pub lane: String,
    pub blockers: Vec<String>,
    pub person: String,
    pub device: u64,
    pub progress: VecDeque<ProgressLine>,
    /// The live last line, P2P only (design §4.8).
    pub tail: Option<ProgressLine>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProjectView {
    pub cursor: u64,
    pub tasks: BTreeMap<String, TaskView>,
    pub agents: BTreeMap<String, AgentView>,
    #[serde(skip)]
    feed: VecDeque<(String, FeedItem)>,
}

#[derive(Debug, Clone, Default)]
pub struct Projection {
    projects: BTreeMap<String, ProjectView>,
}

fn lane_name(lane: Lane) -> String {
    format!("{lane:?}").to_ascii_lowercase()
}

fn push_bounded<T>(q: &mut VecDeque<T>, item: T, max: usize) {
    q.push_back(item);
    while q.len() > max {
        q.pop_front();
    }
}

impl Projection {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn project(&self, id: &str) -> Option<&ProjectView> {
        self.projects.get(id)
    }

    pub fn cursor(&self, id: &str) -> u64 {
        self.projects.get(id).map_or(0, |p| p.cursor)
    }

    /// Folds one `EVENT`. An event at or before the project's cursor was already applied (the
    /// node re-sends on catch-up) and is ignored.
    pub fn apply_event(&mut self, ev: Event) {
        let project = self.projects.entry(ev.project_id.clone()).or_default();
        if ev.cursor <= project.cursor && project.cursor != 0 {
            return;
        }
        project.cursor = ev.cursor;
        let b = ev.body;
        let person = b.iss.as_str().to_owned();
        let feed = |kind: &str, task: &str, text: String| {
            let item = FeedItem {
                author: person.clone(),
                device: b.from,
                verified: false,
                at: 0,
                kind: kind.into(),
                text,
            };
            (task.to_owned(), item)
        };
        match b.typ {
            2 => {
                let Ok(s) = StatusUpdate::from_value(b.payload.0) else {
                    return;
                };
                let agent = project
                    .agents
                    .entry(s.agent_id.clone())
                    .or_insert_with(|| AgentView {
                        task_id: s.task_id.clone(),
                        lane: String::new(),
                        blockers: Vec::new(),
                        person: person.clone(),
                        device: b.from,
                        progress: VecDeque::new(),
                        tail: None,
                    });
                agent.task_id = s.task_id.clone();
                agent.lane = lane_name(s.lane);
                agent.blockers = s.blockers.iter().map(|x| x.text.clone()).collect();
                let text = format!(
                    "lane {}; blockers: {}",
                    agent.lane,
                    agent.blockers.join("; ")
                );
                push_bounded(
                    &mut project.feed,
                    feed("status", &s.task_id, text),
                    KEPT_FEED,
                );
            }
            3 => {
                let Ok(p) = ProgressEvent::from_value(b.payload.0) else {
                    return;
                };
                let Some(agent) = project.agents.get_mut(&p.agent_id) else {
                    return;
                };
                let task = agent.task_id.clone();
                for e in p.events {
                    let line = ProgressLine {
                        at: e.at,
                        kind: e.kind.clone(),
                        text: e.body.clone(),
                    };
                    push_bounded(&mut agent.progress, line, KEPT_LINES);
                    let mut item = feed("progress", &task, e.body);
                    item.1.at = e.at;
                    push_bounded(&mut project.feed, item, KEPT_FEED);
                }
            }
            10 => {
                let Ok(w) = TaskWrite::from_value(b.payload.0) else {
                    return;
                };
                apply_task_write(project, w);
            }
            _ => {}
        }
    }

    /// A `NOTIFY` for the signed-in Person: shown in the feed of every task it names.
    pub fn apply_notify(&mut self, project_id: &str, task_id: &str, n: Notify) {
        let project = self.projects.entry(project_id.to_owned()).or_default();
        let text = format!("{}: {}", n.subject, n.text);
        let item = FeedItem {
            author: "node".into(),
            device: 0,
            verified: false,
            at: n.at,
            kind: format!("notify:{}", n.kind),
            text,
        };
        push_bounded(&mut project.feed, (task_id.to_owned(), item), KEPT_FEED);
    }

    /// A Device-signed `PROGRESS_TAIL` the P2P path verified.
    pub fn apply_tail(&mut self, project_id: &str, tail: ProgressTail) {
        let Some(project) = self.projects.get_mut(project_id) else {
            return;
        };
        if let Some(agent) = project.agents.get_mut(&tail.agent_id) {
            agent.tail = Some(ProgressLine {
                at: tail.at,
                kind: "tail".into(),
                text: tail.line,
            });
        }
    }

    /// The feed of `task_id` after `since` items, written by anyone but `own_person`.
    pub fn feed(
        &self,
        project_id: &str,
        task_id: &str,
        own_person: &str,
        since: usize,
        limit: usize,
    ) -> Vec<FeedItem> {
        let Some(project) = self.projects.get(project_id) else {
            return Vec::new();
        };
        project
            .feed
            .iter()
            .filter(|(task, item)| task == task_id && item.author != own_person)
            .skip(since)
            .take(limit)
            .map(|(_, item)| item.clone())
            .collect()
    }
}

fn apply_task_write(project: &mut ProjectView, w: TaskWrite) {
    if w.op == TaskOp::Delete {
        project.tasks.remove(&w.task_id);
        return;
    }
    let task = project.tasks.entry(w.task_id).or_default();
    let f = w.fields;
    if let Some(t) = f.title {
        task.title = Some(t);
    }
    if let Some(t) = f.tier {
        task.tier = Some(format!("{t:?}").to_ascii_lowercase());
    }
    if let Some(s) = f.status {
        task.status = Some(format!("{s:?}").to_ascii_lowercase());
    }
    if let Some(o) = f.owner {
        task.owner = Some(o.as_str().to_owned());
    }
}
