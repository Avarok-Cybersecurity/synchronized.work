//! `SUBSCRIBE` / `UNSUBSCRIBE`, and the marks and live `EVENT`s each change makes.

use sw_model::{Cid, IdFp};
use sw_protocol::payload::{StatusUpdate, Subscribe, SubscribeOp};

use super::render::{Lens, digest, render};
use super::task_to_bytes;
use super::{AgentMark, Author, Entry, TaskMark, agent_mark_key, agent_to_bytes, task_mark_key};
use crate::core::tree::Change;
use crate::core::{Effect, NodeCore, NodeRefusal};

/// The `from` of an entry adopted from a Task written before the feed existed: its writer's
/// session was never recorded. Citadel assigns no account CID 0.
pub const UNRECORDED_SENDER: Cid = 0;

impl NodeCore {
    /// `SUBSCRIBE` answers with every entry after `since_cursor` the session's Person may see,
    /// oldest first; then live `EVENT`s follow until `UNSUBSCRIBE` or the session's end.
    pub(in crate::core) fn subscription(
        &mut self,
        cid: Cid,
        s: &Subscribe,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        if !self.greeted.contains(&cid) {
            return Err(NodeRefusal::NoHello);
        }
        let project = &s.project_id;
        if s.op == SubscribeOp::Unsubscribe {
            self.feed.unsubscribe(cid, project);
            return Ok(Vec::new());
        }
        let person = self.viewer_of(cid, project)?;
        let limits = self.rules.limits;
        self.feed.subscribe(cid, project, &limits)?;
        let mut effects = self.adopt_unmarked(project, &[])?;
        let lens = self.lens(&person, project);
        for entry in self.feed.after(project, s.since_cursor) {
            if let Some(bytes) = render(&self.tree, &lens, entry)? {
                effects.push(Effect::Send { to: cid, bytes });
            }
        }
        Ok(effects)
    }

    /// The Person session `cid` views `project` as — an enrolled Endpoint's, or a linked
    /// viewer's of that project — refused unless the Person is one of its members.
    pub(in crate::core) fn viewer_of(&self, cid: Cid, project: &str) -> Result<IdFp, NodeRefusal> {
        let person = self
            .session_person(cid, project)
            .ok_or(NodeRefusal::UnknownPerson)?;
        let members = self.tree.members(project).ok_or(NodeRefusal::NotAProject)?;
        if members.contains(&person) {
            Ok(person)
        } else {
            Err(NodeRefusal::NotAMember)
        }
    }

    fn lens<'a>(&self, person: &'a IdFp, project: &str) -> Lens<'a> {
        Lens {
            person,
            members: self.tree.members(project).unwrap_or_default(),
        }
    }

    /// The first time a Project is fed, its Tasks written before the feed existed (none of
    /// `except`) get marks, oldest first, so a viewer's first catch-up includes them.
    fn adopt_unmarked(
        &mut self,
        project: &str,
        except: &[String],
    ) -> Result<Vec<Effect>, NodeRefusal> {
        if self.feed.next_cursor(project) > 1 {
            return Ok(Vec::new());
        }
        let mut found: Vec<_> = self
            .tree
            .tasks
            .values()
            .filter(|t| t.project == project && !except.contains(&t.id))
            .collect();
        found.sort_by(|a, b| (a.created_at, &a.id).cmp(&(b.created_at, &b.id)));
        let mut marks = Vec::new();
        for (cursor, task) in (1..).zip(found) {
            marks.push(TaskMark {
                task: task.id.clone(),
                project: project.to_owned(),
                cursor,
                deleted: false,
                digest: digest(task)?,
                by: Author {
                    iss: task.coop.owner.clone(),
                    from: UNRECORDED_SENDER,
                },
            });
        }
        self.put_task_marks(marks)
    }

    fn put_task_marks(&mut self, marks: Vec<TaskMark>) -> Result<Vec<Effect>, NodeRefusal> {
        let mut effects = Vec::new();
        for mark in marks {
            effects.push(Effect::Put {
                key: task_mark_key(&mark.task),
                value: task_to_bytes(&mark)?,
            });
            self.feed.put_task(mark);
        }
        Ok(effects)
    }

    /// Marks and sends what `change` made visible: each stored Task whose projection changed,
    /// and each deleted Task once.
    pub(in crate::core) fn feed_change(
        &mut self,
        change: &Change,
        by: &Author,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        let mut effects = Vec::new();
        for id in &change.stored {
            let Some(task) = self.tree.get(id) else {
                continue;
            };
            let (project, digest) = (task.project.clone(), digest(task)?);
            let unchanged = self
                .feed
                .task(id)
                .is_some_and(|m| !m.deleted && m.digest == digest);
            if unchanged {
                continue;
            }
            effects.extend(self.adopt_unmarked(&project, &change.stored)?);
            let cursor = self.feed.next_cursor(&project);
            let mark = TaskMark {
                task: id.clone(),
                project,
                cursor,
                deleted: false,
                digest,
                by: by.clone(),
            };
            effects.extend(self.put_task_marks(vec![mark])?);
            effects.extend(self.fan_out_task(id)?);
        }
        for id in &change.deleted {
            let Some(old) = self.feed.task(id).filter(|m| !m.deleted) else {
                continue;
            };
            let cursor = self.feed.next_cursor(&old.project);
            let mark = TaskMark {
                cursor,
                deleted: true,
                by: by.clone(),
                ..old.clone()
            };
            effects.extend(self.put_task_marks(vec![mark])?);
            effects.extend(self.fan_out_task(id)?);
        }
        Ok(effects)
    }

    /// Marks and sends an Agent's new `STATUS_UPDATE`.
    pub(in crate::core) fn feed_status(
        &mut self,
        by: Author,
        s: &StatusUpdate,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        let Some(task) = self.tree.get(&s.task_id) else {
            return Ok(Vec::new());
        };
        let project = task.project.clone();
        let mut effects = self.adopt_unmarked(&project, &[])?;
        let cursor = self.feed.next_cursor(&project);
        let mark = AgentMark {
            project: project.clone(),
            cursor,
            by,
            status: s.clone(),
        };
        effects.push(Effect::Put {
            key: agent_mark_key(&project, &s.agent_id),
            value: agent_to_bytes(&mark)?,
        });
        self.feed.put_agent(mark.clone());
        effects.extend(self.fan_out(&project, Entry::Agent(&mark))?);
        Ok(effects)
    }

    fn fan_out_task(&self, id: &str) -> Result<Vec<Effect>, NodeRefusal> {
        match self.feed.task(id) {
            Some(m) => self.fan_out(&m.project, Entry::Task(m)),
            None => Ok(Vec::new()),
        }
    }

    /// One `EVENT` per subscribed session whose Person may see something of `entry`.
    fn fan_out(&self, project: &str, entry: Entry<'_>) -> Result<Vec<Effect>, NodeRefusal> {
        let mut effects = Vec::new();
        for cid in self.feed.subscribers(project) {
            let Ok(person) = self.viewer_of(cid, project) else {
                continue;
            };
            if let Some(bytes) = render(&self.tree, &self.lens(&person, project), entry)? {
                effects.push(Effect::Send { to: cid, bytes });
            }
        }
        Ok(effects)
    }
}
