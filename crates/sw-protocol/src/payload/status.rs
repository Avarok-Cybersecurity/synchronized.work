//! Status and presence payloads (types 1, 2, 3, 6, 7, 17). Text keys.

use sw_model::readiness::Blocker;
use sw_model::{Cid, Lane};

use crate::WireError;
use crate::wire::{wire_str, wire_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelloKind {
    Device,
    Controller,
    Viewer,
}
wire_str!(HelloKind { "device" => HelloKind::Device, "controller" => HelloKind::Controller, "viewer" => HelloKind::Viewer });

wire_text! {
    /// 1 `HELLO`: the first message of a session. Unsigned.
    pub struct Hello {
        protocol_version: u16,
        endpoint_kind: HelloKind,
        client_build: String,
    }
}

wire_text! {
    /// 2 `STATUS_UPDATE`: Device-signed; at most 16 blockers.
    pub struct StatusUpdate {
        agent_id: String,
        task_id: String,
        lane: Lane,
        blockers: Vec<Blocker>,
        harness: String,
    }
}

wire_text! {
    pub struct ProgressItem {
        at: u64,
        kind: String,
        body: String,
    }
}

wire_text! {
    /// 3 `PROGRESS_EVENT` (batched): Device-signed; at most 64 events, bodies at most 512 B.
    pub struct ProgressEvent {
        agent_id: String,
        events: Vec<ProgressItem>,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckResult {
    Delivered,
    Executed,
    Refused,
    Expired,
}
wire_str!(AckResult {
    "delivered" => AckResult::Delivered, "executed" => AckResult::Executed,
    "refused" => AckResult::Refused, "expired" => AckResult::Expired,
});

wire_text! {
    /// 6 `ACK`: Device-signed, addressed to the Controller's CID. `signal_id` is `SHA3-256` of
    /// the signal's `Signed` bytes.
    pub struct Ack {
        signal_id: [u8; 32],
        result: AckResult,
        reason: Option<String>,
    }
    check = Ack::check;
}

impl Ack {
    /// `reason` iff `refused`.
    fn check(&self) -> Result<(), WireError> {
        if self.reason.is_some() == (self.result == AckResult::Refused) {
            Ok(())
        } else {
            Err(WireError::Shape("ACK reason iff refused"))
        }
    }
}

wire_text! {
    /// 7 `PRESENCE`: node-originated, unsigned. `username` is the peer's Citadel username for
    /// P2P registration (design §4.8).
    pub struct Presence {
        device_cid: Cid,
        online: bool,
        agents: Vec<String>,
        username: String,
    }
}

wire_text! {
    /// 17 `PROGRESS_TAIL`: P2P only, Device-signed, addressed to the Controller's CID.
    pub struct ProgressTail {
        agent_id: String,
        at: u64,
        line: String,
    }
}
