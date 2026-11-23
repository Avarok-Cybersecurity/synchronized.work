//! The daemon's local endpoint (design §9: owner-only, never TCP), over [`transport`]: a Unix
//! socket (0600) or, on Windows, a named pipe only this user can open. One JSON request line,
//! one JSON answer line, per connection. `serve`, `hook`, `gate`, `trust` and `revoke` are
//! clients.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

pub mod transport;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub use transport::{Listener, Transport};

/// This platform's endpoint.
#[cfg(unix)]
pub type Endpoint = unix::UnixSocket;
/// This platform's endpoint.
#[cfg(windows)]
pub type Endpoint = windows::NamedPipe;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockerIn {
    /// `human`, `dependency`, `lock`, `disk` or `rate_limit`.
    pub kind: String,
    pub text: String,
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    WhoAmI,
    Attach {
        agent_id: String,
        project: String,
        task: String,
        cwd: String,
    },
    PostStatus {
        agent_id: String,
        lane: String,
        blockers: Vec<BlockerIn>,
    },
    PostProgress {
        agent_id: String,
        phase: Option<String>,
        percent: Option<u8>,
        line: Option<String>,
    },
    ReadFeed {
        agent_id: String,
        task: String,
        since: usize,
        limit: usize,
    },
    AwaitSignal {
        agent_id: String,
        max_wait_s: u64,
    },
    AckSignal {
        agent_id: String,
        signal_id: String,
        executed: bool,
        reason: Option<String>,
    },
    /// The hook asks for the next deliverable prompt of the agent working in `cwd`.
    NextHookPrompt {
        cwd: String,
        interrupt_only: bool,
    },
    ProjectStatus {
        project: String,
    },
    TaskStatus {
        project: String,
        task: String,
    },
    AgentStatus {
        agent_id: String,
    },
    ProgressTail {
        project: String,
        task: Option<String>,
        limit: usize,
    },
    GateList,
    GateApprove {
        signal_id: String,
    },
    GateRefuse {
        signal_id: String,
    },
    /// The allowlist or the stored assertion changed on disk; read them again.
    Reload,
    /// Scratchpad: leave a note for some members (`to`: id_fps or displays) or, with `to`
    /// absent, for every member but this Person.
    PadPost {
        project: String,
        to: Option<Vec<String>>,
        body: String,
        refs: Vec<String>,
    },
    PadMembers {
        project: String,
    },
    PadInbox {
        project: Option<String>,
    },
    PadAck {
        note_id: String,
        reply: Option<String>,
    },
    /// The next scratchpad event after `after`, waiting up to `max_wait_s`.
    PadWait {
        after: u64,
        max_wait_s: u64,
    },
    PadPolicy {
        project: String,
        retention_hours: u64,
    },
    /// Trust a sender's hand-offs (`label` given) or stop trusting them (`label` absent).
    PadTrust {
        id_fp: String,
        label: Option<String>,
    },
    /// Create a project owned by this Person; its scratchpad starts with the 7-day policy.
    ProjectCreate {
        name: String,
    },
    /// The projects this Device created, joined or asked to join.
    ProjectList,
    /// A one-time invite code for one person, live for `valid_hours` (1..=168).
    InviteCreate {
        project: String,
        valid_hours: u64,
    },
    /// Spend an invite code: a join request the project's owner answers.
    InviteRedeem {
        code: String,
        note: String,
    },
    /// Join requests waiting on this Person's answer, in one project or in all.
    JoinRequests {
        project: Option<String>,
    },
    JoinAnswer {
        request_id: String,
        accept: bool,
        reason: Option<String>,
    },
    /// A link code for the Citadel App, bound to `project` (10 minutes, one use).
    LinkCode {
        project: String,
    },
}

impl Request {
    /// Answered when the node does (a receipt or an answer), not at once.
    pub fn waits_for_the_node(&self) -> bool {
        matches!(
            self,
            Self::ProjectCreate { .. }
                | Self::InviteCreate { .. }
                | Self::InviteRedeem { .. }
                | Self::JoinRequests { .. }
                | Self::JoinAnswer { .. }
                | Self::LinkCode { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Reply {
    Ok(Value),
    Error(String),
}

impl Reply {
    pub fn into_result(self) -> Result<Value, String> {
        match self {
            Self::Ok(v) => Ok(v),
            Self::Error(e) => Err(e),
        }
    }
}

/// Seconds a client waits for an answer beyond what the request itself may block for.
pub const ANSWER_GRACE_S: u64 = 5;

/// Sends one request and reads its answer, from synchronous code (the CLI and the hook).
/// `blocks_for_s` is how long the daemon may hold the request open (`await_signal`); the read
/// times out after that plus [`ANSWER_GRACE_S`]. Async code calls [`call_async`].
pub fn call(endpoint: &Endpoint, request: &Request, blocks_for_s: u64) -> Result<Value, String> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return Err("ipc::call from inside an async runtime; use ipc::call_async".into());
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?
        .block_on(call_async(endpoint.clone(), request.clone(), blocks_for_s))
}

/// One request and its answer over `endpoint`; see [`call`].
pub async fn call_async<T: Transport>(
    endpoint: T,
    request: Request,
    blocks_for_s: u64,
) -> Result<Value, String> {
    let stream = endpoint.connect().await.map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => format!("refusing {}: {e}", endpoint.describe()),
        _ => format!(
            "the sw-mcp daemon is not running ({}: {e}); start it with `sw-mcp daemon`",
            endpoint.describe()
        ),
    })?;
    // The write half stays open until the answer is read: the daemon takes a hang-up as the
    // client giving up.
    let (read, mut write) = tokio::io::split(stream);
    let mut line = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    line.push('\n');
    write
        .write_all(line.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    let wait = Duration::from_secs(blocks_for_s.saturating_add(ANSWER_GRACE_S));
    let mut answer = String::new();
    tokio::time::timeout(wait, BufReader::new(read).read_line(&mut answer))
        .await
        .map_err(|_| format!("no answer from the daemon within {}s", wait.as_secs()))?
        .map_err(|e| format!("no answer from the daemon: {e}"))?;
    let reply: Reply =
        serde_json::from_str(&answer).map_err(|e| format!("the daemon's answer: {e}"))?;
    reply.into_result()
}
