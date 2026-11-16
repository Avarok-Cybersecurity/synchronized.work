//! Payloads between an Endpoint and the node itself (types 11, 13, 14, 15, 16).

use sw_model::{Cid, EndpointKind, Id128};

use crate::model_wire::wire_code;
use crate::wire::{Bytes, wire_map};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrolOp {
    Enrol,
    Revoke,
}
wire_code!(EnrolOp { 0 => EnrolOp::Enrol, 1 => EnrolOp::Revoke });

wire_map! {
    /// 11 `ENROL` / `REVOKE`: signed by the Person's **identity** key (rev 3). An enrol
    /// carries `signing_pk`, plus `sealing_pk` exactly for a Device; a revoke names `kid`.
    pub struct Enrol {
        0 => op: EnrolOp,
        1 => endpoint_cid: Cid,
        2 => kind: EndpointKind,
        3 => epoch: u64,
        4 => signing_pk: Option<Bytes>,
        5 => sealing_pk: Option<Bytes>,
        6 => kid: Option<[u8; 32]>,
    }
}

wire_map! {
    /// 13 `EVENT`: node → viewers, the visibility-filtered projection. Unsigned.
    pub struct Event {
        0 => project_id: Id128,
        1 => cursor: u64,
        2 => kind: String,
        3 => body: Bytes,
    }
}

wire_map! {
    /// 14 `NOTIFY`: a significant event for one Person's Endpoints. Unsigned.
    pub struct Notify {
        0 => event_id: Id128,
        1 => kind: String,
        2 => subject: String,
        3 => text: String,
        4 => at: u64,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscribeOp {
    Subscribe,
    Unsubscribe,
}
wire_code!(SubscribeOp { 0 => SubscribeOp::Subscribe, 1 => SubscribeOp::Unsubscribe });

wire_map! {
    /// 15 `SUBSCRIBE` / `UNSUBSCRIBE`. Unsigned (a read).
    pub struct Subscribe {
        0 => op: SubscribeOp,
        1 => project_id: Id128,
        2 => since_cursor: Option<u64>,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkOp {
    Code,
    Redeem,
}
wire_code!(LinkOp { 0 => LinkOp::Code, 1 => LinkOp::Redeem });

wire_map! {
    /// 16 `LINK_CODE` (Endpoint-signed; asks the node to mint a code) / `LINK_REDEEM`
    /// (unsigned; carries the code and the App context, design §5.3).
    pub struct Link {
        0 => op: LinkOp,
        1 => code: Option<String>,
        2 => acting_username: Option<String>,
        3 => install_id: Option<String>,
        4 => node_id: Option<String>,
    }
}
