//! One Task as the node holds it: the tree point (design §2.1), its cooperation state from
//! `sw-model` (owner, status, tier, pending), the Agents working on it, and the open join
//! requests by `signal_id` so a `JOIN_ANSWER` can name one.

use std::collections::BTreeMap;

use sw_model::tier::Coop;
use sw_model::{Cid, IdFp, Lane, Visibility};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentState {
    pub person: IdFp,
    pub device: Cid,
    pub lane: Lane,
    pub last_seen: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskState {
    pub id: String,
    /// The root Task's id: a Project is the tree under a parentless Task.
    pub project: String,
    pub parent: Option<String>,
    pub title: String,
    pub description: String,
    pub visibility: Visibility,
    pub coop: Coop,
    pub agents: BTreeMap<String, AgentState>,
    pub requests: BTreeMap<[u8; 32], IdFp>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl TaskState {
    /// Persons with an Agent on the Task that is not done.
    pub fn working(&self) -> Vec<IdFp> {
        let mut people: Vec<IdFp> = self
            .agents
            .values()
            .filter(|a| a.lane != Lane::Done)
            .map(|a| a.person.clone())
            .collect();
        people.sort();
        people.dedup();
        people
    }

    /// Whether `person` may run Agents here: the owner, or an admitted co-worker.
    pub fn may_work(&self, person: &IdFp) -> bool {
        use sw_model::tier::Tier;
        *person == self.coop.owner
            || matches!(&self.coop.tier, Tier::Synchronized { coworkers, .. } if coworkers.contains(person))
    }

    pub fn coworkers(&self) -> Vec<IdFp> {
        use sw_model::tier::Tier;
        match &self.coop.tier {
            Tier::Synchronized { coworkers, .. } => coworkers.iter().cloned().collect(),
            Tier::Off | Tier::Aware { .. } => Vec::new(),
        }
    }
}
