//! What a viewer knows of one Project, folded from the node's messages (sw-wire §3: `EVENT`
//! carries the visibility-filtered projection of types 2, 3, 7, 8, 9, 10; `PRESENCE` says which
//! Devices are online). Every field the node may filter out is an `Option`: absent means "not
//! shown to this viewer", never a default.

use std::collections::BTreeMap;

use sw_model::readiness::Blocker;
use sw_model::tier::TierName;
use sw_model::{Cid, IdFp, Lane, Status, Visibility};
use sw_protocol::WireError;
use sw_protocol::payload::{
    Event, Payload, Presence, ProgressEvent, StatusUpdate, TaskFields, TaskOp,
};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::{MapIn, Wire};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskView {
    pub id: String,
    pub title: Option<String>,
    pub parent: Option<String>,
    pub owner: Option<IdFp>,
    pub tier: Option<TierName>,
    pub status: Option<Status>,
    pub visibility: Option<Visibility>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentView {
    pub id: String,
    pub task_id: String,
    pub person: IdFp,
    pub device_cid: Cid,
    pub harness: String,
    pub lane: Lane,
    pub blockers: Vec<Blocker>,
    pub last_line: Option<String>,
    /// From `PRESENCE`; `None` until the node has said.
    pub online: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonView {
    pub id_fp: IdFp,
    /// A Device's Citadel username, from `PRESENCE`.
    pub username: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectState {
    pub project_id: String,
    /// The last applied `EVENT` cursor: `SUBSCRIBE` resumes after it.
    pub cursor: u64,
    pub tasks: BTreeMap<String, TaskView>,
    pub agents: BTreeMap<String, AgentView>,
    pub persons: BTreeMap<IdFp, PersonView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Folded {
    Applied,
    /// Not a message a viewer folds (another type, another project).
    Ignored,
    /// An `EVENT` at or before the cursor: a catch-up replay already applied.
    Stale,
}

/// An absent field leaves what the viewer already knows; a present one replaces it.
fn keep<T>(new: Option<T>, old: &mut Option<T>) {
    if new.is_some() {
        *old = new;
    }
}

impl ProjectState {
    pub fn new(project_id: &str) -> Self {
        Self {
            project_id: project_id.to_owned(),
            cursor: 0,
            tasks: BTreeMap::new(),
            agents: BTreeMap::new(),
            persons: BTreeMap::new(),
        }
    }

    pub fn fold(&mut self, message: &SwMessage) -> Result<Folded, WireError> {
        match message {
            SwMessage::Unsigned(Payload::Event(e)) => self.event(e),
            SwMessage::Unsigned(Payload::Presence(p)) => {
                self.presence(p);
                Ok(Folded::Applied)
            }
            _ => Ok(Folded::Ignored),
        }
    }

    fn event(&mut self, e: &Event) -> Result<Folded, WireError> {
        if e.project_id != self.project_id {
            return Ok(Folded::Ignored);
        }
        if e.cursor <= self.cursor {
            return Ok(Folded::Stale);
        }
        let body = &e.body;
        let payload = body.payload.0.clone();
        let folded = match body.typ {
            2 => {
                let s = StatusUpdate::from_value(payload)?;
                self.status(s, body.iss.clone(), body.from);
                Folded::Applied
            }
            3 => self.progress(ProgressEvent::from_value(payload)?),
            10 => self.task_write(payload)?,
            _ => Folded::Ignored,
        };
        self.cursor = e.cursor;
        Ok(folded)
    }

    fn status(&mut self, s: StatusUpdate, person: IdFp, device_cid: Cid) {
        let previous = self.agents.remove(&s.agent_id);
        self.persons
            .entry(person.clone())
            .or_insert_with(|| PersonView {
                id_fp: person.clone(),
                username: None,
            });
        self.agents.insert(
            s.agent_id.clone(),
            AgentView {
                id: s.agent_id,
                task_id: s.task_id,
                person,
                device_cid,
                harness: s.harness,
                lane: s.lane,
                blockers: s.blockers,
                last_line: previous.as_ref().and_then(|a| a.last_line.clone()),
                online: previous.and_then(|a| a.online),
            },
        );
    }

    fn progress(&mut self, p: ProgressEvent) -> Folded {
        let (Some(agent), Some(last)) = (self.agents.get_mut(&p.agent_id), p.events.last()) else {
            return Folded::Ignored;
        };
        agent.last_line = Some(last.body.clone());
        Folded::Applied
    }

    /// A `TASK_WRITE` after the visibility filter: a `create` may have lost its `title`, so the
    /// map is read field by field rather than through `TaskWrite`'s own shape check.
    fn task_write(&mut self, payload: ciborium::value::Value) -> Result<Folded, WireError> {
        let mut map = MapIn::new(payload)?;
        let op: TaskOp = map.field("op")?;
        let id: String = map.field("task_id")?;
        let f: TaskFields = map.field("fields")?;
        map.finish()?;
        if op == TaskOp::Delete {
            self.tasks.remove(&id);
            return Ok(Folded::Applied);
        }
        let task = self.tasks.entry(id.clone()).or_insert_with(|| TaskView {
            id,
            title: None,
            parent: None,
            owner: None,
            tier: None,
            status: None,
            visibility: None,
        });
        keep(f.title, &mut task.title);
        keep(f.parent, &mut task.parent);
        keep(f.owner.clone(), &mut task.owner);
        keep(f.tier, &mut task.tier);
        keep(f.status, &mut task.status);
        keep(f.visibility, &mut task.visibility);
        if let Some(owner) = f.owner {
            self.persons.entry(owner.clone()).or_insert(PersonView {
                id_fp: owner,
                username: None,
            });
        }
        Ok(Folded::Applied)
    }

    fn presence(&mut self, p: &Presence) {
        for agent in self
            .agents
            .values_mut()
            .filter(|a| a.device_cid == p.device_cid)
        {
            agent.online = Some(p.online && p.agents.contains(&agent.id));
            if let Some(person) = self.persons.get_mut(&agent.person) {
                person.username = Some(p.username.clone());
            }
        }
    }
}
