//! The Task tree (design §2.1): `TASK_WRITE` create / update / delete. Tier changes go
//! through `sw_model::tier::apply`, the one tier machine; this module only maps wire fields to
//! its events and keeps the tree's own invariants (parents exist, ids unique, caps).

use std::collections::BTreeMap;

use sw_model::tier::{Actor, Coop, Tier, TierEvent, TierName, apply};
use sw_model::{IdFp, Role, Status};
use sw_protocol::payload::{TaskFields, TaskOp, TaskWrite};

use super::NodeRefusal;
use super::significant::{Significant, from_effects};
use super::task::TaskState;

/// What a change did: the Task ids to store again (or delete) and the events it made.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Change {
    pub stored: Vec<String>,
    pub deleted: Vec<String>,
    pub events: Vec<Significant>,
}

#[derive(Debug, Default)]
pub struct Tree {
    pub(super) tasks: BTreeMap<String, TaskState>,
}

impl Tree {
    pub fn get(&self, id: &str) -> Option<&TaskState> {
        self.tasks.get(id)
    }

    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn insert_loaded(&mut self, task: TaskState) {
        self.tasks.insert(task.id.clone(), task);
    }

    /// The Project owner: the owner of the Task at the root of `task`'s tree.
    pub fn project_owner(&self, task: &TaskState) -> IdFp {
        self.tasks
            .get(&task.project)
            .map_or_else(|| task.coop.owner.clone(), |root| root.coop.owner.clone())
    }

    pub(super) fn actor(&self, task: &TaskState, iss: &IdFp) -> Actor {
        let role = if self.project_owner(task) == *iss {
            Role::Owner
        } else {
            Role::Member
        };
        Actor::Signed {
            id_fp: iss.clone(),
            role,
        }
    }

    pub fn write(
        &mut self,
        iss: &IdFp,
        write: &TaskWrite,
        now_ms: u64,
        max_tasks: usize,
        is_person: &dyn Fn(&IdFp) -> bool,
    ) -> Result<Change, NodeRefusal> {
        match write.op {
            TaskOp::Create => self.create(iss, &write.task_id, &write.fields, now_ms, max_tasks),
            TaskOp::Update => self.update(iss, &write.task_id, &write.fields, now_ms, is_person),
            TaskOp::Delete => self.delete(iss, &write.task_id),
        }
    }

    fn create(
        &mut self,
        iss: &IdFp,
        id: &str,
        f: &TaskFields,
        now_ms: u64,
        max_tasks: usize,
    ) -> Result<Change, NodeRefusal> {
        if self.tasks.contains_key(id) {
            return Err(NodeRefusal::AlreadyExists);
        }
        if self.tasks.len() >= max_tasks {
            return Err(NodeRefusal::CapReached("tasks"));
        }
        if f.embedding.is_some() {
            return Err(NodeRefusal::EmbeddingUnverifiable);
        }
        let well_formed = f.owner.as_ref().is_none_or(|o| o == iss)
            && f.tier.is_none_or(|t| t == TierName::Off)
            && f.status.is_none_or(|s| s == Status::Open)
            && f.join_policy.is_none();
        let (Some(title), Some(visibility), true) = (&f.title, f.visibility, well_formed) else {
            return Err(NodeRefusal::Malformed);
        };
        let project = match &f.parent {
            Some(parent) => self
                .tasks
                .get(parent)
                .ok_or(NodeRefusal::NoSuchTask)?
                .project
                .clone(),
            None => id.to_owned(),
        };
        let task = TaskState {
            id: id.to_owned(),
            project,
            parent: f.parent.clone(),
            title: title.clone(),
            description: f.description.clone().unwrap_or_default(),
            visibility,
            coop: Coop {
                owner: iss.clone(),
                status: Status::Open,
                tier: Tier::Off,
                pending: Vec::new(),
            },
            agents: BTreeMap::new(),
            requests: BTreeMap::new(),
            created_at: now_ms,
            updated_at: now_ms,
        };
        self.tasks.insert(id.to_owned(), task);
        Ok(Change {
            stored: vec![id.to_owned()],
            ..Change::default()
        })
    }

    fn update(
        &mut self,
        iss: &IdFp,
        id: &str,
        f: &TaskFields,
        now_ms: u64,
        is_person: &dyn Fn(&IdFp) -> bool,
    ) -> Result<Change, NodeRefusal> {
        let before = self.tasks.get(id).ok_or(NodeRefusal::NoSuchTask)?.clone();
        if f.parent.is_some() || f.embedding.is_some() {
            return Err(NodeRefusal::Malformed);
        }
        let actor = self.actor(&before, iss);
        let owner_only = f.title.is_some() || f.description.is_some() || f.visibility.is_some();
        if owner_only && before.coop.owner != *iss {
            return Err(NodeRefusal::Tier(sw_model::tier::Refusal::NotAuthorized));
        }
        let mut events = Vec::new();
        match (f.tier, f.join_policy) {
            (Some(TierName::Aware), Some(policy)) => events.push(TierEvent::OpenUp { policy }),
            (Some(TierName::Off), None) => events.push(TierEvent::Close),
            (None, None) => {}
            _ => return Err(NodeRefusal::Malformed),
        }
        if let Some(status) = f.status {
            events.push(TierEvent::Finish { status });
        }
        if let Some(new_owner) = &f.owner {
            events.push(TierEvent::ChangeOwner {
                new_owner: new_owner.clone(),
                new_owner_is_member: is_person(new_owner),
            });
        }
        let mut after = before.clone();
        let mut effects = Vec::new();
        for event in &events {
            let (coop, mut made) = apply(&after.coop, &actor, event).map_err(NodeRefusal::Tier)?;
            after.coop = coop;
            effects.append(&mut made);
        }
        if let Some(title) = &f.title {
            after.title = title.clone();
        }
        if let Some(description) = &f.description {
            after.description = description.clone();
        }
        if let Some(visibility) = f.visibility {
            after.visibility = visibility;
        }
        after.updated_at = now_ms;
        let owner = self.project_owner(&after);
        let significant = from_effects(&before, &after, &owner, &effects);
        self.tasks.insert(id.to_owned(), after);
        Ok(Change {
            stored: vec![id.to_owned()],
            deleted: Vec::new(),
            events: significant,
        })
    }

    /// Only the owner deletes, and only a leaf: a Task with children would orphan them.
    fn delete(&mut self, iss: &IdFp, id: &str) -> Result<Change, NodeRefusal> {
        let task = self.tasks.get(id).ok_or(NodeRefusal::NoSuchTask)?;
        if task.coop.owner != *iss {
            return Err(NodeRefusal::Tier(sw_model::tier::Refusal::NotAuthorized));
        }
        if self.tasks.values().any(|t| t.parent.as_deref() == Some(id)) {
            return Err(NodeRefusal::HasChildren);
        }
        self.tasks.remove(id);
        Ok(Change {
            deleted: vec![id.to_owned()],
            ..Change::default()
        })
    }
}
