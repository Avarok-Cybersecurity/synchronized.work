//! Signals to a Device (types 4 and 5). The outer payload carries only the [`Sealed`]
//! envelope; the node verifies the outer signature and cannot read the rest.

use sw_model::Id128;
use sw_model::tier::JoinPolicy;

use crate::model_wire::wire_code;
use crate::sealed::Sealed;
use crate::wire::wire_map;

wire_map! {
    /// The outer payload of 4 `PROMPT` and 5 `CONTROL`.
    pub struct SealedPayload {
        0 => sealed: Sealed,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptMode {
    Queue,
    Interrupt,
}
wire_code!(PromptMode { 0 => PromptMode::Queue, 1 => PromptMode::Interrupt });

wire_map! {
    /// The sealed inner payload of a `PROMPT`.
    pub struct PromptInner {
        0 => task_id: Id128,
        1 => agent_id: Option<Id128>,
        2 => text: String,
        3 => mode: PromptMode,
        4 => reply_to: Option<Id128>,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCode {
    Pause,
    Resume,
    Stop,
    SetTier,
    ApproveJoin,
    RefuseJoin,
}
wire_code!(ControlCode {
    0 => ControlCode::Pause, 1 => ControlCode::Resume, 2 => ControlCode::Stop,
    3 => ControlCode::SetTier, 4 => ControlCode::ApproveJoin, 5 => ControlCode::RefuseJoin,
});

wire_map! {
    /// The sealed inner payload of a `CONTROL`. `set_tier` names `task_id` and either
    /// `open_with` (→ aware under that policy) or neither (→ off); the join answers name
    /// `request_id`, and a refusal a `reason`.
    pub struct ControlInner {
        0 => op: ControlCode,
        1 => task_id: Option<Id128>,
        2 => open_with: Option<JoinPolicy>,
        3 => request_id: Option<Id128>,
        4 => reason: Option<String>,
    }
}
