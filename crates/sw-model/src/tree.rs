//! The points of one Project's tree (design §2.1). Times are milliseconds since the Unix epoch;
//! nothing here has a default — a missing field is a refusal upstream (PCND).

use crate::readiness::Blocker;
use crate::tier::Tier;
use crate::{Cid, Id128, IdFp};

/// Project policy, chosen in the create form (D7). Every field is required.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    pub progress_retention_days: u16,
    pub done_lane_compaction_days: u16,
    pub stall_after_ms: u64,
    pub schema_version: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub id: Id128,
    pub name: String,
    pub created_at: u64,
    pub policy: Policy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Status {
    Open,
    Done,
    Cancelled,
}

/// Who sees a Task's progress bodies (design §6.5). Required at creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
    Owner,
    Members,
    Summary,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub id: Id128,
    pub title: String,
    pub description: String,
    pub owner: IdFp,
    pub parent: Option<Id128>,
    pub status: Status,
    pub visibility: Visibility,
    pub tier: Tier,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    Owner,
    Admin,
    Member,
    Viewer,
}

/// A Project member, keyed by `id_fp` (rev 3, D9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub id_fp: IdFp,
    pub handle: String,
    pub display_name: String,
    pub role: Role,
    pub joined_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndpointKind {
    Device,
    Controller,
}

/// One Citadel account on the node enrolled under a Person by an identity-signed `ENROL`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub cid: Cid,
    pub kind: EndpointKind,
    /// `SHA-256` of the Endpoint's ML-DSA-65 verifying key.
    pub signing_kid: [u8; 32],
    /// `SHA-256` of the ML-KEM-768 encapsulation key; present exactly for Devices.
    pub sealing_kid: Option<[u8; 32]>,
}

/// The node's record of a Person (design §5.1): the identity key *is* the Person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonRecord {
    pub id_fp: IdFp,
    /// The ML-DSA-65 identity public key whose fingerprint is `id_fp`.
    pub pk: Vec<u8>,
    pub handle: String,
    pub endpoints: Vec<Endpoint>,
    /// Bumped by every `REVOKE`.
    pub epoch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lane {
    Running,
    Waiting,
    Blocked,
    Done,
    Stalled,
}

/// One coding-agent session on a Device; an Agent run is a lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    pub id: Id128,
    pub task: Id128,
    pub person: IdFp,
    pub device_cid: Cid,
    pub harness: String,
    pub lane: Lane,
    pub blockers: Vec<Blocker>,
    pub started_at: u64,
    pub last_seen: u64,
}

impl Agent {
    /// Design §2.4: stalled when `last_seen` is older than the project's `stall_after`.
    pub fn is_stalled(&self, now_ms: u64, policy: &Policy) -> bool {
        self.lane != Lane::Done && now_ms.saturating_sub(self.last_seen) > policy.stall_after_ms
    }
}

/// The rolled-up state of one Agent (exactly one per Agent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub agent: Id128,
    pub phase: String,
    /// 0..=100.
    pub percent: u8,
    /// At most [`Progress::MAX_LAST_LINE`] bytes.
    pub last_line: String,
    pub updated_at: u64,
    pub event_cursor: u64,
}

impl Progress {
    pub const MAX_LAST_LINE: usize = 512;
}
