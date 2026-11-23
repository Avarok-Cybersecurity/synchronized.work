//! A project with a member, and the viewer's side of the node: subscribe, decode what arrived.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::support::*;
use sw_model::Visibility;
use sw_model::tier::{JoinPolicy, TierName};
use sw_node::core::NodeRefusal;
use sw_protocol::body::Audience;
use sw_protocol::payload::*;
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Wire;

pub type Sent = Vec<(u64, Vec<u8>)>;

pub fn create(id: &str, parent: Option<&str>, visibility: Visibility) -> Payload {
    Payload::TaskWrite(TaskWrite {
        op: TaskOp::Create,
        task_id: id.into(),
        fields: TaskFields {
            title: Some(format!("title of {id}")),
            description: Some(format!("body of {id}")),
            visibility: Some(visibility),
            parent: parent.map(str::to_owned),
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

pub fn alice(w: &mut World, payload: Payload) -> Sent {
    w.send_as(
        Who::Alice,
        sw_model::EndpointKind::Controller,
        Audience::Node,
        payload,
    )
    .unwrap()
}

/// Bob asks to join `task` from his Controller; returns the request's `signal_id`.
pub fn bob_requests(w: &mut World, task: &str) -> [u8; 32] {
    let request = Payload::JoinRequest(JoinRequest {
        task_id: task.into(),
        note: "me".into(),
        readiness: sw_model::readiness::Readiness::from_permille(1000).unwrap(),
        agent_id: None,
        invite: None,
    });
    let bob = std::mem::replace(&mut w.bob, World::new().bob);
    let m = w.envelope(&bob, &bob.controller, Audience::Node, request);
    let SwMessage::Signed { signed, .. } = &m else {
        unreachable!()
    };
    let id = signed.signal_id().unwrap();
    let effects = w
        .node
        .received(bob.controller.cid, &m.to_bytes().unwrap(), NOW)
        .unwrap();
    w.bob = bob;
    w.apply(effects);
    id
}

pub fn accept(request_id: [u8; 32]) -> Payload {
    Payload::JoinAnswer(JoinAnswer {
        request_id,
        accept: true,
        reason: None,
    })
}

/// Alice owns project `p` (members) with task `t` opened up; Bob is not a member yet.
pub fn project() -> World {
    let mut w = World::enrolled();
    alice(&mut w, create("p", None, Visibility::Members));
    alice(&mut w, create("t", Some("p"), Visibility::Members));
    alice(&mut w, open_up("t"));
    w
}

/// [`project`], with Bob admitted to `t`, so a member of `p`.
pub fn with_member() -> World {
    let mut w = project();
    let id = bob_requests(&mut w, "t");
    alice(&mut w, accept(id));
    w
}

pub fn subscribe_bytes(op: SubscribeOp, project: &str, since: u64) -> Vec<u8> {
    SwMessage::Unsigned(Payload::Subscribe(Subscribe {
        op,
        project_id: project.into(),
        since_cursor: since,
    }))
    .to_bytes()
    .unwrap()
}

pub fn subscribe(w: &mut World, cid: u64, project: &str, since: u64) -> Result<Sent, NodeRefusal> {
    let bytes = subscribe_bytes(SubscribeOp::Subscribe, project, since);
    let effects = w.node.received(cid, &bytes, NOW)?;
    Ok(w.apply(effects))
}

pub fn unsubscribe(w: &mut World, cid: u64, project: &str) {
    let bytes = subscribe_bytes(SubscribeOp::Unsubscribe, project, 0);
    let effects = w.node.received(cid, &bytes, NOW).unwrap();
    w.apply(effects);
}

/// The `EVENT`s sent to `to`, in order.
pub fn events(sent: &Sent, to: u64) -> Vec<Event> {
    sent.iter()
        .filter(|(cid, _)| *cid == to)
        .filter_map(|(_, bytes)| match SwMessage::from_bytes(bytes).unwrap() {
            SwMessage::Unsigned(Payload::Event(e)) => Some(e),
            _ => None,
        })
        .collect()
}

/// `(cursor, task_id)` of each Task event.
pub fn task_ids(events: &[Event]) -> Vec<(u64, String)> {
    events
        .iter()
        .filter(|e| e.body.typ == 10)
        .map(|e| {
            let w = written(e);
            (e.cursor, w.task_id)
        })
        .collect()
}

/// A Task event's payload, read field by field as a viewer's fold reads it.
pub fn written(e: &Event) -> TaskWrite {
    TaskWrite::from_value(e.body.payload.0.clone()).unwrap()
}

/// The `PAD_RECEIPT`s sent to `to`.
pub fn receipts(sent: &Sent, to: u64) -> Vec<PadReceipt> {
    sent.iter()
        .filter(|(cid, _)| *cid == to)
        .filter_map(|(_, bytes)| match SwMessage::from_bytes(bytes).unwrap() {
            SwMessage::Unsigned(Payload::PadReceipt(r)) => Some(r),
            _ => None,
        })
        .collect()
}

/// A fresh core rebuilt from everything `w` stored, as the Durable Object would on wake.
pub fn restarted(w: &World) -> sw_node::core::NodeCore {
    let mut node = sw_node::core::NodeCore::new(rules(&w.issuer));
    for (key, value) in &w.stored {
        node.load(key, value).unwrap();
    }
    node
}

/// Alice's Controller follows `p` (replaying nothing) and mints `code`; what the node sent.
pub fn mint(w: &mut World, code: &str) -> Sent {
    subscribe(w, w.alice.controller.cid, "p", u64::MAX).unwrap();
    let link = Payload::Link(Link {
        op: LinkOp::Code,
        code: code.into(),
        acting_username: None,
        install_id: None,
        node_id: None,
    });
    w.send_as(
        Who::Alice,
        sw_model::EndpointKind::Controller,
        Audience::Node,
        link,
    )
    .unwrap()
}
