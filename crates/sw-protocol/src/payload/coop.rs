//! Cooperation payloads written to the ProjectDO (types 8, 9, 10, 12). Text keys.

use sw_model::embedding::DIMS;
use sw_model::readiness::Readiness;
use sw_model::tier::{JoinPolicy, TierName};
use sw_model::{IdFp, Status, Visibility};

use crate::WireError;
use crate::payload::limits;
use crate::wire::{wire_str, wire_text};

wire_text! {
    /// 8 `JOIN_REQUEST`: Endpoint-signed. `note` at most 280 bytes; `readiness` permille.
    /// A request to join a project (its root Task) spends an invite token (type 23); a
    /// request for any other Task carries none.
    pub struct JoinRequest {
        task_id: String,
        note: String,
        readiness: Readiness,
        agent_id: Option<String>,
        invite: Option<[u8; 32]>,
    }
}

wire_text! {
    /// 9 `JOIN_ANSWER`: signed by one of the Task owner's Endpoints.
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
    /// A Task's writable fields; each absent unless written. The node recomputes `embedding`
    /// from `title` + `description` and refuses a mismatch.
    pub struct TaskFields {
        title: Option<String>,
        description: Option<String>,
        parent: Option<String>,
        tier: Option<TierName>,
        visibility: Option<Visibility>,
        status: Option<Status>,
        owner: Option<IdFp>,
        join_policy: Option<JoinPolicy>,
        embedding: Option<Vec<u16>>,
    }
    check = TaskFields::check;
}

impl TaskFields {
    fn check(&self) -> Result<(), WireError> {
        let over = |t: &Option<String>, max: usize| t.as_ref().is_some_and(|t| t.len() > max);
        let ok = !over(&self.title, limits::TASK_TITLE_BYTES)
            && !over(&self.description, limits::TASK_DESCRIPTION_BYTES)
            && self.embedding.as_ref().is_none_or(|e| e.len() == DIMS);
        if ok {
            Ok(())
        } else {
            Err(WireError::Length("TaskFields limit"))
        }
    }

    /// The empty map a `delete` carries.
    pub const EMPTY: Self = Self {
        title: None,
        description: None,
        parent: None,
        tier: None,
        visibility: None,
        status: None,
        owner: None,
        join_policy: None,
        embedding: None,
    };
}

wire_text! {
    /// 10 `TASK_WRITE`: Endpoint-signed by the owner or an admin. `create` requires `title`;
    /// `delete` carries an empty map.
    pub struct TaskWrite {
        op: TaskOp,
        task_id: String,
        fields: TaskFields,
    }
    check = TaskWrite::check;
}

impl TaskWrite {
    fn check(&self) -> Result<(), WireError> {
        let ok = match self.op {
            TaskOp::Create => self.fields.title.is_some(),
            TaskOp::Update => true,
            TaskOp::Delete => self.fields == TaskFields::EMPTY,
        };
        if ok {
            Ok(())
        } else {
            Err(WireError::Shape("TASK_WRITE fields for its op"))
        }
    }
}

wire_text! {
    /// 12 `SUGGESTION`: Endpoint-signed, at most 2 KiB, never delivered to a Device.
    pub struct Suggestion {
        task_id: String,
        text: String,
    }
}
