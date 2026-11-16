//! Cooperation payloads written to the ProjectDO (types 8, 9, 10, 12).

use sw_model::readiness::Readiness;
use sw_model::{Id128, IdFp, Status, Visibility};

use crate::model_wire::wire_code;
use crate::wire::wire_map;

wire_map! {
    /// 8 `JOIN_REQUEST`: Endpoint-signed. `note` at most 280 bytes.
    pub struct JoinRequest {
        0 => task_id: Id128,
        1 => note: String,
        2 => readiness: Readiness,
        3 => agent_id: Option<Id128>,
    }
}

wire_map! {
    /// 9 `JOIN_ANSWER`: signed by the owner's Controller key.
    pub struct JoinAnswer {
        0 => request_id: Id128,
        1 => accept: bool,
        2 => reason: Option<String>,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskOp {
    Create,
    Update,
    Delete,
}
wire_code!(TaskOp { 0 => TaskOp::Create, 1 => TaskOp::Update, 2 => TaskOp::Delete });

wire_map! {
    /// 10 `TASK_WRITE`: Endpoint-signed by the owner or an admin. `embedding` is the 256
    /// integer buckets of `sw_model::embedding::buckets`, which the ProjectDO recomputes
    /// from `title` and `description` and refuses on mismatch. A create carries `title`,
    /// `description`, `visibility` and `embedding`; nothing is defaulted.
    pub struct TaskWrite {
        0 => task_id: Id128,
        1 => op: TaskOp,
        2 => title: Option<String>,
        3 => description: Option<String>,
        4 => visibility: Option<Visibility>,
        5 => parent: Option<Id128>,
        6 => owner: Option<IdFp>,
        7 => status: Option<Status>,
        8 => embedding: Option<Vec<i32>>,
    }
}

wire_map! {
    /// 12 `SUGGESTION`: Endpoint-signed, at most 2 KiB, never delivered to a Device.
    pub struct Suggestion {
        0 => task_id: Id128,
        1 => text: String,
    }
}
