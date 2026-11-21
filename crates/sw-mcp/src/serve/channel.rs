//! The push wake-up path: Claude Code's channels (research preview). With `serve --channel`
//! the server declares `experimental["claude/channel"]` and turns every scratchpad event the
//! daemon's `PadWait` returns into a `notifications/claude/channel`, which Claude Code injects
//! into the session as a new turn. Claude Code does not acknowledge it; the agent's
//! `scratchpad_ack` is the acknowledgement. The permission-relay capability is never declared:
//! a colleague must not be able to approve tool use here.

use std::time::Duration;

use rmcp::model::{CustomNotification, JsonObject, ServerNotification};
use rmcp::service::{Peer, RoleServer};
use serde_json::{Value, json};

use crate::ipc::{Request, call_async};

pub const CAPABILITY: &str = "claude/channel";
pub const METHOD: &str = "notifications/claude/channel";
/// One `PadWait` per call; the loop re-arms after each.
const PUSH_WAIT_S: u64 = 300;
const RETRY_S: u64 = 5;

/// The experimental capabilities to declare: the channel listener, nothing else.
pub fn experimental() -> std::collections::BTreeMap<String, JsonObject> {
    std::collections::BTreeMap::from([(CAPABILITY.to_owned(), JsonObject::new())])
}

/// The notification's params for one rendered `PadEvent`. `content` is the event's fixed
/// frame; `meta` carries only values this process generated (identifiers, never sender text),
/// because Claude Code renders them as tag attributes.
pub fn params(event: &Value) -> Value {
    let text = |k: &str| {
        event
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let cursor = event
        .get("cursor")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    json!({
        "content": text("frame"),
        "meta": {
            "kind": text("kind"),
            "note_id": text("note_id"),
            "trust": text("trust"),
            "verified": event.get("verified").and_then(Value::as_bool).unwrap_or(false).to_string(),
            "cursor": cursor.to_string(),
        }
    })
}

/// Streams events to the client until the transport closes. Starts at cursor 0, so every
/// note still waiting for an ack is pushed again when a session starts.
pub async fn push_loop(peer: Peer<RoleServer>, socket: crate::ipc::Endpoint) {
    let mut after = 0u64;
    loop {
        let request = Request::PadWait {
            after,
            max_wait_s: PUSH_WAIT_S,
        };
        match call_async(socket.clone(), request, PUSH_WAIT_S).await {
            Ok(v) if v.get("timeout").and_then(Value::as_bool) == Some(true) => {}
            Ok(v) => {
                after = v.get("cursor").and_then(Value::as_u64).unwrap_or(after);
                let n = CustomNotification::new(METHOD, Some(params(&v)));
                if let Err(e) = peer
                    .send_notification(ServerNotification::CustomNotification(n))
                    .await
                {
                    eprintln!("sw-mcp serve: the channel closed: {e}");
                    return;
                }
            }
            Err(e) => {
                eprintln!("sw-mcp serve: scratchpad wait: {e}");
                tokio::time::sleep(Duration::from_secs(RETRY_S)).await;
            }
        }
    }
}
