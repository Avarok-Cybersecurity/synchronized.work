//! What viewers are sent (design §4.6): every Project keeps a monotonic `cursor`, and each
//! Task and each Agent carries the cursor of its last visible change (a *mark*). The log is
//! kept compacted to one entry per Task and per Agent, so `SUBSCRIBE{since_cursor}` answers
//! with every entry whose mark is later, oldest first — the snapshot and the log in one read —
//! and every later change reaches each subscribed session as one live `EVENT`, rendered for
//! that session's Person through `sw_model::visibility`.
//!
//! Subscriptions belong to a session and end with it; the marks are stored.

mod codec;
pub(super) mod render;
pub(super) mod serve;

use codec::{agent_from_bytes, agent_to_bytes, task_from_bytes, task_to_bytes};

use std::collections::BTreeMap;

use sw_model::{Cid, IdFp};
use sw_protocol::payload::StatusUpdate;

use super::NodeRefusal;
use super::pad::ledger::hex;
use crate::limits::Limits;

/// Who wrote the change an entry projects: an `EventBody`'s `iss` and `from`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author {
    pub iss: IdFp,
    pub from: Cid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskMark {
    pub task: String,
    pub project: String,
    pub cursor: u64,
    pub deleted: bool,
    /// SHA3-256 of the Task's full projection: a write that changes nothing a viewer could
    /// see (an Agent's lane, a pending request) makes no Task event.
    pub digest: [u8; 32],
    pub by: Author,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMark {
    pub project: String,
    pub cursor: u64,
    pub by: Author,
    /// The Agent's last `STATUS_UPDATE`, as its Device signed it.
    pub status: StatusUpdate,
}

/// One entry of a Project's compacted log.
#[derive(Debug, Clone, Copy)]
pub enum Entry<'a> {
    Task(&'a TaskMark),
    Agent(&'a AgentMark),
}

impl Entry<'_> {
    pub fn cursor(&self) -> u64 {
        match self {
            Self::Task(m) => m.cursor,
            Self::Agent(m) => m.cursor,
        }
    }
}

pub fn task_mark_key(task: &str) -> String {
    format!("feed/t/{}", hex(task.as_bytes()))
}

pub fn agent_mark_key(project: &str, agent: &str) -> String {
    format!(
        "feed/a/{}/{}",
        hex(project.as_bytes()),
        hex(agent.as_bytes())
    )
}

#[derive(Debug, Default)]
pub struct Feed {
    tasks: BTreeMap<String, TaskMark>,
    agents: BTreeMap<(String, String), AgentMark>,
    /// Index: each Project's last cursor, derived from the marks as they are put.
    heads: BTreeMap<String, u64>,
    /// Each session's Projects, in the order it subscribed (the latest last).
    subs: BTreeMap<Cid, Vec<String>>,
}

impl Feed {
    pub fn task(&self, id: &str) -> Option<&TaskMark> {
        self.tasks.get(id)
    }

    /// The cursor the next entry of `project` gets.
    pub fn next_cursor(&self, project: &str) -> u64 {
        self.heads.get(project).map_or(1, |h| h + 1)
    }

    fn advance(&mut self, project: &str, cursor: u64) {
        let head = self.heads.entry(project.to_owned()).or_insert(cursor);
        *head = (*head).max(cursor);
    }

    pub fn put_task(&mut self, mark: TaskMark) {
        self.advance(&mark.project, mark.cursor);
        self.tasks.insert(mark.task.clone(), mark);
    }

    pub fn put_agent(&mut self, mark: AgentMark) {
        self.advance(&mark.project, mark.cursor);
        let key = (mark.project.clone(), mark.status.agent_id.clone());
        self.agents.insert(key, mark);
    }

    /// `project`'s entries after `since`, oldest first.
    pub fn after(&self, project: &str, since: u64) -> Vec<Entry<'_>> {
        let tasks = self.tasks.values().map(Entry::Task);
        let agents = self.agents.values().map(Entry::Agent);
        let mut out: Vec<Entry<'_>> = tasks
            .chain(agents)
            .filter(|e| e.cursor() > since && Self::of(e) == project)
            .collect();
        out.sort_by_key(Entry::cursor);
        out
    }

    fn of<'a>(e: &Entry<'a>) -> &'a str {
        match e {
            Entry::Task(m) => &m.project,
            Entry::Agent(m) => &m.project,
        }
    }

    /// Subscribes `cid` (again, which moves `project` to the latest), within the limits.
    pub fn subscribe(&mut self, cid: Cid, project: &str, l: &Limits) -> Result<(), NodeRefusal> {
        let mine = self.subs.get(&cid).map_or(&[][..], Vec::as_slice);
        let already = mine.iter().any(|p| p == project);
        if !already && mine.len() >= l.subscriptions_per_session {
            return Err(NodeRefusal::CapReached("subscriptions per session"));
        }
        if !already && self.subscribers(project).len() >= l.subscribers_per_project {
            return Err(NodeRefusal::CapReached("subscribers per project"));
        }
        let mine = self.subs.entry(cid).or_default();
        mine.retain(|p| p != project);
        mine.push(project.to_owned());
        Ok(())
    }

    pub fn unsubscribe(&mut self, cid: Cid, project: &str) {
        if let Some(mine) = self.subs.get_mut(&cid) {
            mine.retain(|p| p != project);
            if mine.is_empty() {
                self.subs.remove(&cid);
            }
        }
    }

    pub fn drop_session(&mut self, cid: Cid) {
        self.subs.remove(&cid);
    }

    /// The Project `cid` subscribed to last.
    pub fn latest(&self, cid: Cid) -> Option<&str> {
        self.subs.get(&cid)?.last().map(String::as_str)
    }

    pub fn subscribers(&self, project: &str) -> Vec<Cid> {
        self.subs
            .iter()
            .filter(|(_, ps)| ps.iter().any(|p| p == project))
            .map(|(cid, _)| *cid)
            .collect()
    }
}

impl super::NodeCore {
    /// Loads one mark; `Ok(false)` if `key` is not one.
    pub(crate) fn feed_load(&mut self, key: &str, value: &[u8]) -> Result<bool, String> {
        let wire = |e: sw_protocol::WireError| format!("{e:?}");
        if key.starts_with("feed/t/") {
            let mark = task_from_bytes(value).map_err(wire)?;
            if task_mark_key(&mark.task) != key {
                return Err("task mark stored under another key".into());
            }
            self.feed.put_task(mark);
            return Ok(true);
        }
        if key.starts_with("feed/a/") {
            let mark = agent_from_bytes(value).map_err(wire)?;
            if agent_mark_key(&mark.project, &mark.status.agent_id) != key {
                return Err("agent mark stored under another key".into());
            }
            self.feed.put_agent(mark);
            return Ok(true);
        }
        Ok(false)
    }
}
