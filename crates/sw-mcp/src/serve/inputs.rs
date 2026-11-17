//! The MCP tools' arguments (JSON Schema via schemars).

use schemars::JsonSchema;
use serde::Deserialize;

use crate::ipc::BlockerIn;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SignInCompleteInput {
    /// The one-time code the sign-in page showed, `XXXX-XXXX` (case and dash optional).
    pub code: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AttachInput {
    /// The synchronized.work project id.
    pub project: String,
    /// The task id this agent works on.
    pub task: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BlockerInput {
    /// `human`, `dependency`, `lock`, `disk` or `rate_limit`.
    pub kind: String,
    /// At most 280 bytes.
    pub text: String,
    /// The task this one waits on; required iff `kind` is `dependency`.
    pub task_id: Option<String>,
}

impl From<BlockerInput> for BlockerIn {
    fn from(b: BlockerInput) -> Self {
        Self {
            kind: b.kind,
            text: b.text,
            task_id: b.task_id,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PostStatusInput {
    /// `running`, `waiting`, `blocked`, `done` or `stalled`.
    pub lane: String,
    /// At most 16.
    pub blockers: Vec<BlockerInput>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PostProgressInput {
    pub phase: Option<String>,
    pub percent: Option<u8>,
    /// One line, at most 512 bytes once control characters are removed.
    pub line: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadFeedInput {
    pub task: String,
    /// Items already read (the previous call's `next_since`; 0 the first time).
    pub since: usize,
    /// At most 64.
    pub limit: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AwaitSignalInput {
    /// At most 25.
    pub max_wait_s: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AckSignalInput {
    pub signal_id: String,
    /// `true` once done; `false` to refuse it.
    pub executed: bool,
    /// Why, when refusing.
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectInput {
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskInput {
    pub project: String,
    pub task: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentInput {
    pub agent_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TailInput {
    pub project: String,
    /// Only this task's agents, when given.
    pub task: Option<String>,
    /// Lines per agent, at most 64.
    pub limit: usize,
}
