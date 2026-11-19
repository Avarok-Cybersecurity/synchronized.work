//! The daemon's Unix socket (design §9: 0600, never TCP). One JSON request line, one JSON
//! answer line, per connection. `serve`, `hook`, `gate`, `trust` and `revoke` are clients.

use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

/// Sends one request and reads its answer. `blocks_for_s` is how long the daemon may hold the
/// request open (`await_signal`); the read times out after that plus [`ANSWER_GRACE_S`].
pub fn call(socket: &Path, request: &Request, blocks_for_s: u64) -> Result<Value, String> {
    let mut stream = UnixStream::connect(socket).map_err(|e| {
        format!(
            "the sw-mcp daemon is not running ({}: {e}); start it with `sw-mcp daemon`",
            socket.display()
        )
    })?;
    let wait = Duration::from_secs(blocks_for_s.saturating_add(ANSWER_GRACE_S));
    stream
        .set_read_timeout(Some(wait))
        .map_err(|e| e.to_string())?;
    let mut line = serde_json::to_string(request).map_err(|e| e.to_string())?;
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut answer = String::new();
    BufReader::new(stream)
        .read_line(&mut answer)
        .map_err(|e| format!("no answer from the daemon: {e}"))?;
    let reply: Reply =
        serde_json::from_str(&answer).map_err(|e| format!("the daemon's answer: {e}"))?;
    reply.into_result()
}

/// `call` from async code, on the blocking pool.
pub async fn call_async(
    socket: std::path::PathBuf,
    request: Request,
    blocks_for_s: u64,
) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || call(&socket, &request, blocks_for_s))
        .await
        .map_err(|e| e.to_string())?
}
