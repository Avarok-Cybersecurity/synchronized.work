//! Significant events (design §4.7): the subset of Task changes people are told about, each
//! with the Persons it goes to. The tier machine's [`Effect`]s are the source; nothing here
//! invents an event the machine did not report.

use sw_model::IdFp;
use sw_model::Status;
use sw_model::tier::Effect;

use super::task::TaskState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    PersonJoinedTask,
    PersonLeftTask,
    JoinRequested,
    TierChanged,
    TaskClaimed,
    TaskDone,
    TaskCancelled,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::PersonJoinedTask => "person_joined_task",
            Self::PersonLeftTask => "person_left_task",
            Self::JoinRequested => "join_requested",
            Self::TierChanged => "tier_changed",
            Self::TaskClaimed => "task_claimed",
            Self::TaskDone => "task_done",
            Self::TaskCancelled => "task_cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Significant {
    pub kind: Kind,
    pub task: String,
    /// Whom the event is about (the joiner, the leaver, the requester, the claimant); the
    /// owner for events about the Task itself.
    pub about: IdFp,
    pub recipients: Vec<IdFp>,
}

/// The events one transition's effects make, given the Task before and after it and the
/// Project owner (the only admin role the node has).
pub fn from_effects(
    before: &TaskState,
    after: &TaskState,
    project_owner: &IdFp,
    effects: &[Effect],
) -> Vec<Significant> {
    let owner = &after.coop.owner;
    let event = |kind, about: &IdFp, mut recipients: Vec<IdFp>| {
        recipients.sort();
        recipients.dedup();
        Significant {
            kind,
            task: after.id.clone(),
            about: about.clone(),
            recipients,
        }
    };
    let mut out = Vec::new();
    for effect in effects {
        match effect {
            Effect::Admitted { person } => out.push(event(
                Kind::PersonJoinedTask,
                person,
                vec![owner.clone(), project_owner.clone(), person.clone()],
            )),
            Effect::Left { person } | Effect::Detached { person, .. } => out.push(event(
                Kind::PersonLeftTask,
                person,
                vec![owner.clone(), person.clone()],
            )),
            Effect::JoinRequested { person } => {
                out.push(event(Kind::JoinRequested, person, vec![owner.clone()]))
            }
            Effect::TierChanged { .. } => {
                let mut to = before.coworkers();
                to.extend(after.coworkers());
                to.push(project_owner.clone());
                out.push(event(Kind::TierChanged, owner, to));
            }
            Effect::StatusChanged { status } => {
                let kind = match status {
                    Status::Done => Kind::TaskDone,
                    Status::Cancelled => Kind::TaskCancelled,
                    Status::Open => continue,
                };
                let mut to = after.working();
                to.push(project_owner.clone());
                out.push(event(kind, owner, to));
            }
            Effect::Refused { .. } | Effect::OwnerChanged { .. } => {}
        }
    }
    out
}

/// A Task's first Agent (its first `WORKS_ON` edge).
pub fn claimed(task: &TaskState, claimant: &IdFp, project_owner: &IdFp) -> Significant {
    let mut recipients = vec![project_owner.clone()];
    if *claimant != task.coop.owner {
        recipients.push(task.coop.owner.clone());
    }
    recipients.sort();
    recipients.dedup();
    Significant {
        kind: Kind::TaskClaimed,
        task: task.id.clone(),
        about: claimant.clone(),
        recipients,
    }
}
