//! One entry as one viewer may see it: an `EVENT` whose body is the projection of a
//! `TASK_WRITE` (typ 10) or a `STATUS_UPDATE` (typ 2), filtered by `sw_model::visibility` —
//! the same rule the `NOTIFY` text follows. A Task entry carries the Task's current fields as
//! an `update` (both folds upsert; a filtered `create` could not carry a missing title), a
//! deleted Task a `delete`.

use std::collections::BTreeSet;

use sw_model::IdFp;
use sw_model::visibility::{Level, Viewer, bodies_shown, level, title_shown};
use sw_protocol::payload::{
    Event, EventBody, Payload, StatusUpdate, TaskFields, TaskOp, TaskWrite,
};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::{Raw, Wire};
use sw_protocol::{WireError, cbor};

use super::super::task::TaskState;
use super::super::tree::Tree;
use super::{AgentMark, Author, Entry, TaskMark};

/// One viewer of one Project: its Person and the Project's members as they are now.
pub struct Lens<'a> {
    pub person: &'a IdFp,
    pub members: BTreeSet<IdFp>,
}

impl Lens<'_> {
    fn viewer(&self, task: &TaskState) -> Viewer {
        if task.coop.owner == *self.person {
            Viewer::TaskOwner
        } else if self.members.contains(self.person) {
            Viewer::ProjectMember
        } else {
            Viewer::Outsider
        }
    }

    fn level(&self, task: &TaskState) -> Level {
        level(task.visibility, self.viewer(task))
    }
}

/// The Task's fields a viewer at `level` may see (`Nothing` is never rendered).
pub fn task_write(task: &TaskState, level: Level) -> TaskWrite {
    TaskWrite {
        op: TaskOp::Update,
        task_id: task.id.clone(),
        fields: TaskFields {
            title: title_shown(level).then(|| task.title.clone()),
            description: bodies_shown(level).then(|| task.description.clone()),
            parent: task.parent.clone(),
            tier: Some(task.coop.tier.name()),
            visibility: Some(task.visibility),
            status: Some(task.coop.status),
            owner: Some(task.coop.owner.clone()),
            ..TaskFields::EMPTY
        },
    }
}

/// What a Task mark records: the hash of everything any viewer could be shown of it.
pub fn digest(task: &TaskState) -> Result<[u8; 32], WireError> {
    let full = cbor::encode(&task_write(task, Level::Full).to_value())?;
    Ok(sw_crypto::sha3_256(&full))
}

fn event(
    project: &str,
    cursor: u64,
    typ: u8,
    by: &Author,
    payload: Payload,
) -> Result<Vec<u8>, WireError> {
    let kind = match typ {
        2 => "status_update",
        _ => "task_write",
    };
    let event = Event {
        project_id: project.to_owned(),
        cursor,
        kind: kind.to_owned(),
        body: EventBody {
            typ,
            from: by.from,
            iss: by.iss.clone(),
            payload: Raw(payload.to_value()),
        },
    };
    SwMessage::Unsigned(Payload::Event(event)).to_bytes()
}

fn task_event(tree: &Tree, lens: &Lens<'_>, m: &TaskMark) -> Result<Option<Vec<u8>>, WireError> {
    let write = if m.deleted {
        if !lens.members.contains(lens.person) {
            return Ok(None);
        }
        TaskWrite {
            op: TaskOp::Delete,
            task_id: m.task.clone(),
            fields: TaskFields::EMPTY,
        }
    } else {
        let Some(task) = tree.get(&m.task) else {
            return Ok(None);
        };
        match lens.level(task) {
            Level::Nothing => return Ok(None),
            l => task_write(task, l),
        }
    };
    event(&m.project, m.cursor, 10, &m.by, Payload::TaskWrite(write)).map(Some)
}

/// An Agent's lane and harness reach every viewer of its Task; its blockers only those shown
/// bodies (the frozen `STATUS_UPDATE` has no absent form for them, so they are sent empty).
fn agent_event(tree: &Tree, lens: &Lens<'_>, m: &AgentMark) -> Result<Option<Vec<u8>>, WireError> {
    let Some(task) = tree.get(&m.status.task_id) else {
        return Ok(None);
    };
    let level = lens.level(task);
    if level == Level::Nothing {
        return Ok(None);
    }
    let status = StatusUpdate {
        blockers: if bodies_shown(level) {
            m.status.blockers.clone()
        } else {
            Vec::new()
        },
        ..m.status.clone()
    };
    event(
        &m.project,
        m.cursor,
        2,
        &m.by,
        Payload::StatusUpdate(status),
    )
    .map(Some)
}

/// The `EVENT` bytes `entry` makes for this viewer, if it may see anything of it.
pub fn render(
    tree: &Tree,
    lens: &Lens<'_>,
    entry: Entry<'_>,
) -> Result<Option<Vec<u8>>, WireError> {
    match entry {
        Entry::Task(m) => task_event(tree, lens, m),
        Entry::Agent(m) => agent_event(tree, lens, m),
    }
}
