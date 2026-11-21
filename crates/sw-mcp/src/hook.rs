//! `sw-mcp hook stop` and `sw-mcp hook pre-tool-use` (design §9.3 adapter 1): Claude Code runs
//! these with the hook's JSON on stdin. If a verified, approved prompt is queued for the agent
//! working in the hook's `cwd`, the hook blocks with the prompt, framed as the owner's
//! instruction, as its reason; otherwise it prints nothing and the agent goes on.

use std::io::Read as _;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::home::Home;
use crate::ipc::{Request, call};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookKind {
    /// `Stop`: the agent finished its turn; a queued prompt (any mode) continues it.
    Stop,
    /// `PreToolUse`: only an `interrupt` prompt stops the next tool call.
    PreToolUse,
}

/// The fields of Claude Code's hook input this hook reads; the rest are ignored.
#[derive(Debug, Deserialize)]
struct HookInput {
    cwd: String,
}

/// The hook's stdout for `instruction`, or nothing.
pub fn output(kind: HookKind, instruction: Option<&str>) -> Option<Value> {
    let reason = instruction?;
    Some(match kind {
        HookKind::Stop => json!({ "decision": "block", "reason": reason }),
        HookKind::PreToolUse => json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            }
        }),
    })
}

pub fn run(home: &Home, kind: HookKind) -> Result<Option<Value>, String> {
    let mut stdin = String::new();
    std::io::stdin()
        .read_to_string(&mut stdin)
        .map_err(|e| e.to_string())?;
    let input: HookInput = serde_json::from_str(&stdin).map_err(|e| format!("hook input: {e}"))?;
    let request = Request::NextHookPrompt {
        cwd: input.cwd,
        interrupt_only: kind == HookKind::PreToolUse,
    };
    let answer = call(home.ipc(), &request, 0)?;
    let instruction = answer.get("instruction").and_then(Value::as_str);
    Ok(output(kind, instruction))
}
