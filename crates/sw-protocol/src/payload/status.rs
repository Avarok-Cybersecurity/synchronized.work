//! Status and presence payloads (types 1, 2, 3, 6, 7, 17).

use sw_model::readiness::Blocker;
use sw_model::{Cid, EndpointKind, Id128, Lane};

use crate::wire::wire_map;

wire_map! {
    /// 1 `HELLO`: the first message of a session. Unsigned.
    pub struct Hello {
        0 => protocol_version: u16,
        1 => endpoint_kind: EndpointKind,
        2 => client_build: String,
    }
}

wire_map! {
    /// 2 `STATUS_UPDATE`: Device-signed.
    pub struct StatusUpdate {
        0 => agent_id: Id128,
        1 => task_id: Id128,
        2 => lane: Lane,
        3 => blockers: Vec<Blocker>,
        4 => harness: String,
    }
}

wire_map! {
    pub struct ProgressItem {
        0 => at: u64,
        1 => kind: String,
        2 => body: String,
    }
}

wire_map! {
    /// 3 `PROGRESS_EVENT` (batched): Device-signed; at most 64 events, bodies at most 512 B.
    pub struct ProgressEvent {
        0 => agent_id: Id128,
        1 => events: Vec<ProgressItem>,
    }
}

/// The outcome a Device reports for a signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AckResult {
    Delivered,
    Executed,
    Refused { reason: String },
    Expired,
}

wire_map! {
    /// 6 `ACK`: Device-signed. `signal_id` is `SHA-256` of the signal's `Signed` bytes.
    pub struct Ack {
        0 => signal_id: [u8; 32],
        1 => result: AckResult,
    }
}

wire_map! {
    /// 7 `PRESENCE`: node-originated display data derived from the session table. Unsigned.
    /// `username` (rev 3, §4.8) tells a Person's own Endpoints each other's Citadel username
    /// so they can register as P2P peers.
    pub struct Presence {
        0 => device_cid: Cid,
        1 => online: bool,
        2 => agents: Vec<Id128>,
        3 => username: Option<String>,
    }
}

wire_map! {
    /// 17 `PROGRESS_TAIL` (rev 3): P2P only, Device-signed; never written to the ProjectDO.
    pub struct ProgressTail {
        0 => agent_id: Id128,
        1 => at: u64,
        2 => line: String,
    }
}

mod ack_wire {
    use ciborium::value::Value;

    use super::AckResult;
    use crate::WireError;
    use crate::wire::{MapIn, MapOut, Wire};

    /// `{0: code}`: delivered 0, executed 1, refused 2 (with `1: reason`), expired 3.
    impl Wire for AckResult {
        fn to_value(&self) -> Value {
            let mut out = MapOut::default();
            let code: u8 = match self {
                AckResult::Delivered => 0,
                AckResult::Executed => 1,
                AckResult::Refused { reason } => {
                    out.field(1, reason);
                    2
                }
                AckResult::Expired => 3,
            };
            out.field(0, &code);
            out.build()
        }
        fn from_value(value: Value) -> Result<Self, WireError> {
            let mut map = MapIn::new(value)?;
            let result = match map.field::<u8>(0)? {
                0 => AckResult::Delivered,
                1 => AckResult::Executed,
                2 => AckResult::Refused {
                    reason: map.field(1)?,
                },
                3 => AckResult::Expired,
                _ => return Err(WireError::UnknownCode("AckResult")),
            };
            map.finish()?;
            Ok(result)
        }
    }
}
