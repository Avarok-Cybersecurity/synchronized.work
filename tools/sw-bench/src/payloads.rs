//! The payloads the bench and the tests send.

use sw_model::tier::{JoinPolicy, TierName};
use sw_model::{Visibility, readiness::Readiness};
use sw_protocol::payload::*;

pub fn create(id: &str, title: &str) -> Payload {
    Payload::TaskWrite(TaskWrite {
        op: TaskOp::Create,
        task_id: id.into(),
        fields: TaskFields {
            title: Some(title.into()),
            visibility: Some(Visibility::Members),
            ..TaskFields::EMPTY
        },
    })
}

pub fn retitle(id: &str, title: &str) -> Payload {
    Payload::TaskWrite(TaskWrite {
        op: TaskOp::Update,
        task_id: id.into(),
        fields: TaskFields {
            title: Some(title.into()),
            ..TaskFields::EMPTY
        },
    })
}

pub fn open_up(id: &str) -> Payload {
    Payload::TaskWrite(TaskWrite {
        op: TaskOp::Update,
        task_id: id.into(),
        fields: TaskFields {
            tier: Some(TierName::Aware),
            join_policy: Some(JoinPolicy::Manual),
            ..TaskFields::EMPTY
        },
    })
}

pub fn join(id: &str, readiness: Readiness) -> Payload {
    Payload::JoinRequest(JoinRequest {
        task_id: id.into(),
        note: "let me help".into(),
        readiness,
        agent_id: None,
        invite: None,
    })
}
