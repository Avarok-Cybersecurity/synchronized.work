//! The scratchpad tools' arguments.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PadPostInput {
    /// The synchronized.work project id.
    pub project: String,
    /// The members to notify, by id_fp or display name; omit to notify every other member.
    pub to: Option<Vec<String>>,
    /// The note (at most 4096 bytes), e.g. "free to merge my PR when you're ready".
    pub body: String,
    /// Links the note refers to: PRs, repos, commits (at most 16).
    pub refs: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PadProjectInput {
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PadInboxInput {
    /// Only this project's notes; omit for every project.
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PadAckInput {
    /// The note_id of a received note.
    pub note_id: String,
    /// A short answer for the sender, sealed to them (optional).
    pub reply: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PadWaitInput {
    /// The cursor of the last event handled; 0 the first time.
    pub after: u64,
    /// At most 25.
    pub max_wait_s: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PadPolicyInput {
    pub project: String,
    /// How long notes live, 1..=168 hours. Project owner only.
    pub retention_hours: u64,
}
