//! The sealed inner maps of 4 `PROMPT` and 5 `CONTROL`. On the wire `Body[9]` **is** the
//! [`crate::sealed::Sealed`] map; these are what it opens to, `aud` included.

use sw_model::Cid;

use crate::wire::{wire_str, wire_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptMode {
    Queue,
    Interrupt,
}
wire_str!(PromptMode { "queue" => PromptMode::Queue, "interrupt" => PromptMode::Interrupt });

wire_text! {
    pub struct PromptInner {
        /// The Device CID this prompt is for; the Device refuses any other.
        aud: Cid,
        task_id: String,
        text: String,
        mode: PromptMode,
        agent_id: Option<String>,
        reply_to: Option<[u8; 32]>,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlOp {
    Pause,
    Resume,
    Stop,
    SetTier,
    ApproveJoin,
    RefuseJoin,
}
wire_str!(ControlOp {
    "pause" => ControlOp::Pause, "resume" => ControlOp::Resume, "stop" => ControlOp::Stop,
    "set_tier" => ControlOp::SetTier, "approve_join" => ControlOp::ApproveJoin,
    "refuse_join" => ControlOp::RefuseJoin,
});

wire_text! {
    /// `tier` for `set_tier`; `id` (a join request id) for the join answers; `reason` for a
    /// refusal.
    pub struct ControlInner {
        aud: Cid,
        op: ControlOp,
        tier: Option<String>,
        id: Option<[u8; 32]>,
        reason: Option<String>,
    }
}
