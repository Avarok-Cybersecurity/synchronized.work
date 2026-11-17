//! The tier state machine (design §2.5): `off → aware → synchronized` and back.
//!
//! Owner transitions are accepted only from [`Actor::Signed`] — an actor the protocol layer
//! produced from a verified `Signed` envelope — whose `id_fp` is the Task's owner. The
//! ProjectDO's own transitions (first admission, last co-worker leaving) come from
//! [`Actor::ProjectDo`]. A Citadel session alone ([`Actor::Session`]) proves which account is
//! connected, not that a person authorised a write (§6.3), so it can change nothing.

mod apply;

pub use apply::apply;

use std::collections::BTreeSet;

use crate::readiness::Readiness;
use crate::{IdFp, Role, Status};

/// How an `aware` Task admits co-workers; chosen explicitly when the owner opens it up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JoinPolicy {
    /// Every join request waits for the owner's signed `JOIN_ANSWER`.
    Manual,
    /// The ProjectDO admits the readiest pending request while fewer than `max_coworkers`
    /// are admitted.
    Auto { max_coworkers: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tier {
    /// Shown on the tree, not open to cooperation.
    Off,
    /// Open to other Persons' Agents joining under the Task.
    Aware { policy: JoinPolicy },
    /// At least one other Person is admitted and working on it now. Never empty.
    Synchronized {
        policy: JoinPolicy,
        coworkers: BTreeSet<IdFp>,
    },
}

/// Which tier, without its state: what a `CONTROL set_tier` or a `TaskFields.tier` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TierName {
    Off,
    Aware,
    Synchronized,
}

impl Tier {
    pub fn name(&self) -> TierName {
        match self {
            Self::Off => TierName::Off,
            Self::Aware { .. } => TierName::Aware,
            Self::Synchronized { .. } => TierName::Synchronized,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinRequest {
    pub person: IdFp,
    pub readiness: Readiness,
    pub at_ms: u64,
}

/// The cooperation state of one Task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coop {
    pub owner: IdFp,
    pub status: Status,
    pub tier: Tier,
    pub pending: Vec<JoinRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Actor {
    /// The issuer of a verified `Signed` envelope, with the role their Person record holds.
    Signed { id_fp: IdFp, role: Role },
    /// A connected session with no signature behind the request.
    Session { id_fp: IdFp },
    /// The Project's Durable Object, acting on its own rules.
    ProjectDo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TierEvent {
    OpenUp {
        policy: JoinPolicy,
    },
    RequestJoin {
        readiness: Readiness,
        at_ms: u64,
    },
    AcceptJoin {
        person: IdFp,
    },
    RefuseJoin {
        person: IdFp,
    },
    AutoAdmit,
    CoworkerLeft {
        person: IdFp,
    },
    Close,
    Finish {
        status: Status,
    },
    ChangeOwner {
        new_owner: IdFp,
        new_owner_is_member: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetachReason {
    OwnerClosed,
    OwnerChanged,
}

/// What a transition did, for the progress log and the significant-event rules (§4.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    TierChanged { from: TierName, to: TierName },
    JoinRequested { person: IdFp },
    Admitted { person: IdFp },
    Refused { person: IdFp },
    Detached { person: IdFp, reason: DetachReason },
    Left { person: IdFp },
    StatusChanged { status: Status },
    OwnerChanged { new_owner: IdFp },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The request carried no verified signature.
    Unsigned,
    /// The signer is not allowed to make this transition.
    NotAuthorized,
    /// The Task is done or cancelled; its tier is frozen.
    Frozen,
    /// The transition does not exist from the current tier.
    WrongTier,
    /// The Person is the owner, already admitted, or already pending.
    AlreadyPresent,
    /// No pending request from that Person, or no pending request at all.
    NoSuchRequest,
    /// Not a co-worker on this Task.
    NotCoworker,
    /// `auto { max_coworkers }` is reached.
    CapReached,
    /// `AutoAdmit` under a `manual` policy.
    ManualPolicy,
    /// A new owner must be a Project member; `Finish` must name a closed status.
    InvalidTarget,
}
