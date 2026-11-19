//! Joining and working (design §2.4, §2.5): `JOIN_REQUEST`, `JOIN_ANSWER` and the
//! `STATUS_UPDATE` lanes that make a Person a worker on a Task or let them leave it.

use sw_model::tier::{Actor, JoinPolicy, Tier, TierEvent, apply};
use sw_model::{Cid, IdFp, Lane};
use sw_protocol::payload::{JoinAnswer, JoinRequest, StatusUpdate};

use super::NodeRefusal;
use super::significant::{claimed, from_effects};
use super::task::{AgentState, TaskState};
use super::tree::{Change, Tree};

impl Tree {
    /// A request is recorded under its `signal_id`; under `auto` the ProjectDO's own rule
    /// admits the readiest pending request at once.
    pub fn join_request(
        &mut self,
        iss: &IdFp,
        request: &JoinRequest,
        signal_id: [u8; 32],
        now_ms: u64,
    ) -> Result<Change, NodeRefusal> {
        let before = self.task(&request.task_id)?.clone();
        let actor = self.actor(&before, iss);
        let event = TierEvent::RequestJoin {
            readiness: request.readiness,
            at_ms: now_ms,
        };
        let (coop, mut effects) = apply(&before.coop, &actor, &event).map_err(NodeRefusal::Tier)?;
        let mut after = before.clone();
        after.coop = coop;
        after.requests.insert(signal_id, iss.clone());
        if matches!(
            after.coop.tier,
            Tier::Aware {
                policy: JoinPolicy::Auto { .. }
            } | Tier::Synchronized {
                policy: JoinPolicy::Auto { .. },
                ..
            }
        ) && let Ok((coop, mut admitted)) =
            apply(&after.coop, &Actor::ProjectDo, &TierEvent::AutoAdmit)
        {
            after.coop = coop;
            effects.append(&mut admitted);
        }
        self.commit(before, after, &effects, now_ms)
    }

    /// The owner's answer, naming the request by its `signal_id`.
    pub fn join_answer(
        &mut self,
        iss: &IdFp,
        answer: &JoinAnswer,
        now_ms: u64,
    ) -> Result<Change, NodeRefusal> {
        let before = self
            .tasks
            .values()
            .find(|t| t.requests.contains_key(&answer.request_id))
            .ok_or(NodeRefusal::NoSuchRequest)?
            .clone();
        let Some(person) = before.requests.get(&answer.request_id).cloned() else {
            return Err(NodeRefusal::NoSuchRequest);
        };
        let actor = self.actor(&before, iss);
        let event = if answer.accept {
            TierEvent::AcceptJoin { person }
        } else {
            TierEvent::RefuseJoin { person }
        };
        let (coop, effects) = apply(&before.coop, &actor, &event).map_err(NodeRefusal::Tier)?;
        let mut after = before.clone();
        after.coop = coop;
        after.requests.remove(&answer.request_id);
        self.commit(before, after, &effects, now_ms)
    }

    /// A Device's lane for one Agent. A Person may work only on a Task they own or were
    /// admitted to; a co-worker whose last open lane is `done` has left (`CoworkerLeft`).
    pub fn status(
        &mut self,
        iss: &IdFp,
        device: Cid,
        update: &StatusUpdate,
        now_ms: u64,
    ) -> Result<Change, NodeRefusal> {
        let before = self.task(&update.task_id)?.clone();
        if !before.may_work(iss) {
            return Err(NodeRefusal::NotAWorker);
        }
        if before
            .agents
            .get(&update.agent_id)
            .is_some_and(|a| a.person != *iss)
        {
            return Err(NodeRefusal::TakenByAnotherPerson);
        }
        let mut after = before.clone();
        let first = after.agents.is_empty();
        after.agents.insert(
            update.agent_id.clone(),
            AgentState {
                person: iss.clone(),
                device,
                lane: update.lane,
                last_seen: now_ms,
            },
        );
        let mut effects = Vec::new();
        let left =
            update.lane == Lane::Done && *iss != after.coop.owner && !after.working().contains(iss);
        if left {
            let event = TierEvent::CoworkerLeft {
                person: iss.clone(),
            };
            let (coop, mut made) =
                apply(&after.coop, &Actor::ProjectDo, &event).map_err(NodeRefusal::Tier)?;
            after.coop = coop;
            effects.append(&mut made);
        }
        let mut change = self.commit(before, after, &effects, now_ms)?;
        if first && let Some(task) = self.tasks.get(&update.task_id) {
            let owner = self.project_owner(task);
            change.events.insert(0, claimed(task, iss, &owner));
        }
        Ok(change)
    }

    fn task(&self, id: &str) -> Result<&TaskState, NodeRefusal> {
        self.tasks.get(id).ok_or(NodeRefusal::NoSuchTask)
    }

    fn commit(
        &mut self,
        before: TaskState,
        mut after: TaskState,
        effects: &[sw_model::tier::Effect],
        now_ms: u64,
    ) -> Result<Change, NodeRefusal> {
        after.updated_at = now_ms;
        let owner = self.project_owner(&after);
        let events = from_effects(&before, &after, &owner, effects);
        let id = after.id.clone();
        self.tasks.insert(id.clone(), after);
        Ok(Change {
            stored: vec![id],
            deleted: Vec::new(),
            events,
        })
    }
}
