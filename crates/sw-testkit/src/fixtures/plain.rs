//! Unsigned fixtures (1, 7, 13, 14, 15, 16 `LINK_REDEEM`), Bob's 8 and 12, and the
//! identity-signed 11.

use sw_model::readiness::Readiness;
use sw_model::{EndpointKind, Id128};
use sw_protocol::body::Audience;
use sw_protocol::payload::*;
use sw_protocol::wire::Bytes;

use super::{AGENT, Fixture, Issuer, PROJECT, TASK, unsigned};
use crate::{Cast, FIXTURE_NOW_MS, LabelledEntropy};

pub(super) fn fixtures(cast: &Cast, e: &mut LabelledEntropy) -> Vec<Fixture> {
    let bob = Issuer {
        key: &cast.bob_controller,
        iss: cast.bob.id_fp(),
        from: cast.bob_controller_cid,
    };
    let identity = Issuer {
        key: &cast.alice,
        iss: cast.alice.id_fp(),
        from: cast.alice_controller_cid,
    };
    let join = JoinRequest {
        task_id: TASK,
        note: "idle agent, same crate".into(),
        readiness: Readiness::READY,
        agent_id: None,
    };
    let suggestion = Suggestion {
        task_id: TASK,
        text: "the cap test needs a control".into(),
    };
    let enrol = Enrol {
        op: EnrolOp::Enrol,
        endpoint_cid: cast.alice_device_cid,
        kind: EndpointKind::Device,
        epoch: 1,
        signing_pk: Some(Bytes(cast.alice_device.public_key().to_vec())),
        sealing_pk: Some(Bytes(cast.alice_device_kem.encapsulation_key().to_vec())),
        kid: None,
    };
    let node = Audience::Node;
    vec![
        ("01-hello.cbor", unsigned(Payload::Hello(hello()))),
        (
            "07-presence.cbor",
            unsigned(Payload::Presence(presence(cast))),
        ),
        (
            "08-join-request.cbor",
            bob.signed(node, 1, Payload::JoinRequest(join), e),
        ),
        (
            "11-enrol.cbor",
            identity.signed(node, 1, Payload::Enrol(enrol), e),
        ),
        (
            "12-suggestion.cbor",
            bob.signed(node, 2, Payload::Suggestion(suggestion), e),
        ),
        ("13-event.cbor", unsigned(Payload::Event(event()))),
        ("14-notify.cbor", unsigned(Payload::Notify(notify()))),
        (
            "15-subscribe.cbor",
            unsigned(Payload::Subscribe(subscribe())),
        ),
        ("16-link-redeem.cbor", unsigned(Payload::Link(redeem()))),
    ]
}

fn hello() -> Hello {
    Hello {
        protocol_version: 1,
        endpoint_kind: EndpointKind::Device,
        client_build: "sw-mcp 0.1.0".into(),
    }
}

fn presence(cast: &Cast) -> Presence {
    Presence {
        device_cid: cast.alice_device_cid,
        online: true,
        agents: vec![AGENT],
        username: Some(format!("{}.laptop@synchronized.work", cast.alice.id_fp())),
    }
}

fn event() -> Event {
    Event {
        project_id: PROJECT,
        cursor: 42,
        kind: "status".into(),
        body: Bytes(vec![0xa0]),
    }
}

fn notify() -> Notify {
    Notify {
        event_id: Id128([0xe5; 16]),
        kind: "join_requested".into(),
        subject: "Port the WAL".into(),
        text: "bob asked to join".into(),
        at: FIXTURE_NOW_MS,
    }
}

fn subscribe() -> Subscribe {
    Subscribe {
        op: SubscribeOp::Subscribe,
        project_id: PROJECT,
        since_cursor: Some(40),
    }
}

fn redeem() -> Link {
    Link {
        op: LinkOp::Redeem,
        code: Some("K7PX-29QD".into()),
        acting_username: Some("alice@acme.example".into()),
        install_id: Some("inst-01".into()),
        node_id: Some("node-01".into()),
    }
}
