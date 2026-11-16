//! Cooperation payloads written to the ProjectDO (types 8, 9, 10, 12). Text keys.

use crate::wire::{Raw, wire_str, wire_text};

wire_text! {
    /// 8 `JOIN_REQUEST`: Endpoint-signed. `note` at most 280 bytes; `readiness` is text.
    pub struct JoinRequest {
        task_id: String,
        note: String,
        readiness: String,
        agent_id: Option<String>,
    }
}

wire_text! {
    /// 9 `JOIN_ANSWER`: signed by the owner's Controller key.
    pub struct JoinAnswer {
        request_id: [u8; 32],
        accept: bool,
        reason: Option<String>,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskOp {
    Create,
    Update,
    Delete,
}
wire_str!(TaskOp { "create" => TaskOp::Create, "update" => TaskOp::Update, "delete" => TaskOp::Delete });

wire_text! {
    /// 10 `TASK_WRITE`: Endpoint-signed by the owner or an admin. `fields` is a map whose
    /// schema the contract leaves to the ProjectDO.
    pub struct TaskWrite {
        op: TaskOp,
        task_id: String,
        fields: Raw,
    }
}

wire_text! {
    /// 12 `SUGGESTION`: Endpoint-signed, at most 2 KiB, never delivered to a Device.
    pub struct Suggestion {
        task_id: String,
        text: String,
    }
}
