//! Alice owns project `p` and invites; Bob spends invites. Builders for the invite tests.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::support::*;
use sw_model::EndpointKind::Device;
use sw_model::{Cid, Visibility};
use sw_node::core::NodeRefusal;
use sw_protocol::body::Audience;
use sw_protocol::invite::token_hash;
use sw_protocol::payload::*;
use sw_protocol::signed::SwMessage;

pub const HOUR: u64 = 3_600_000;

pub fn create(id: &str, parent: Option<&str>) -> Payload {
    Payload::TaskWrite(TaskWrite {
        op: TaskOp::Create,
        task_id: id.into(),
        fields: TaskFields {
            title: Some(format!("task {id}")),
            visibility: Some(Visibility::Members),
            parent: parent.map(str::to_owned),
            ..TaskFields::EMPTY
        },
    })
}

pub fn invite(token: u8, exp: u64) -> Payload {
    Payload::Invite(Invite {
        project: "p".into(),
        token_hash: token_hash(&[token; 32]),
        exp,
    })
}

pub fn join(token: Option<u8>) -> Payload {
    Payload::JoinRequest(JoinRequest {
        task_id: "p".into(),
        note: "it's me".into(),
        readiness: sw_model::readiness::Readiness::from_permille(1000).unwrap(),
        agent_id: None,
        invite: token.map(|t| [t; 32]),
    })
}

pub fn answer(request_id: [u8; 32], accept: bool) -> Payload {
    Payload::JoinAnswer(JoinAnswer {
        request_id,
        accept,
        reason: None,
    })
}

/// Alice owns `p` and invited token 1.
pub fn invited() -> World {
    let mut w = World::enrolled();
    w.send_as(Who::Alice, Device, Audience::Node, create("p", None))
        .unwrap();
    w.send_as(Who::Alice, Device, Audience::Node, invite(1, NOW + HOUR))
        .unwrap();
    w
}

/// Bob's join request with `token`, at `now`; its signal id and the outcome.
pub fn bob_joins(
    w: &mut World,
    token: Option<u8>,
    now: u64,
) -> ([u8; 32], Result<(), NodeRefusal>) {
    let bob = std::mem::replace(&mut w.bob, World::new().bob);
    let m = w.envelope(&bob, &bob.device, Audience::Node, join(token));
    w.bob = bob;
    let SwMessage::Signed { signed, .. } = &m else {
        unreachable!()
    };
    let id = signed.signal_id().unwrap();
    let out = w
        .node
        .received(w.bob.device.cid, &m.to_bytes().unwrap(), now)
        .map(|e| drop(w.apply(e)));
    (id, out)
}

pub fn members(w: &World) -> usize {
    w.node.pad_members("p").unwrap().len()
}

pub fn receipts(sent: &[(Cid, Vec<u8>)]) -> Vec<(Cid, u8, bool)> {
    sent.iter()
        .filter_map(|(to, b)| match SwMessage::from_bytes(b).ok()? {
            SwMessage::Unsigned(Payload::PadReceipt(r)) => Some((*to, r.typ, r.stored)),
            _ => None,
        })
        .collect()
}
