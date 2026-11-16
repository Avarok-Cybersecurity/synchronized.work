//! The sealed inner maps of 4 `PROMPT` and 5 `CONTROL`. On the wire `Body[9]` **is** the
//! [`crate::sealed::Sealed`] map; these are what it opens to, `aud` included.

use sw_model::Cid;
use sw_model::tier::TierName;

use crate::WireError;
use crate::payload::limits;
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
    /// `tier` iff `set_tier`; `id` (the `JOIN_REQUEST`'s `signal_id`) iff a join answer;
    /// `reason` (at most 280 bytes) iff `refuse_join`.
    pub struct ControlInner {
        aud: Cid,
        op: ControlOp,
        tier: Option<TierName>,
        id: Option<[u8; 32]>,
        reason: Option<String>,
    }
    check = ControlInner::check;
}

impl ControlInner {
    fn check(&self) -> Result<(), WireError> {
        use ControlOp::*;
        let reason_ok = match (&self.reason, self.op) {
            (Some(r), RefuseJoin) => r.len() <= limits::REFUSAL_REASON_BYTES,
            (None, RefuseJoin) => false,
            (reason, _) => reason.is_none(),
        };
        let ok = reason_ok
            && self.tier.is_some() == (self.op == SetTier)
            && self.id.is_some() == matches!(self.op, ApproveJoin | RefuseJoin);
        if ok {
            Ok(())
        } else {
            Err(WireError::Shape("CONTROL fields for its op"))
        }
    }
}
