//! Fixtures a Device signs: 2, 3, 6 and the P2P 17.

use sw_model::Lane;
use sw_model::readiness::Blocker;
use sw_protocol::body::Audience;
use sw_protocol::p2p::encode_p2p;
use sw_protocol::payload::*;

use super::{AGENT, Fixture, Issuer, TASK};
use crate::{Cast, FIXTURE_NOW_MS, LabelledEntropy};

pub(super) fn fixtures(cast: &Cast, e: &mut LabelledEntropy) -> Vec<Fixture> {
    let device = Issuer {
        key: &cast.alice_device,
        iss: cast.alice.id_fp(),
        from: cast.alice_device_cid,
    };
    let status = StatusUpdate {
        agent_id: AGENT,
        task_id: TASK,
        lane: Lane::Blocked,
        blockers: vec![
            Blocker::Dependency(TASK),
            Blocker::Other("waiting on review".into()),
        ],
        harness: "claude-code".into(),
    };
    let progress = ProgressEvent {
        agent_id: AGENT,
        events: vec![ProgressItem {
            at: FIXTURE_NOW_MS,
            kind: "phase".into(),
            body: "build".into(),
        }],
    };
    let ack = Ack {
        signal_id: [0x5a; 32],
        result: AckResult::Refused {
            reason: "approval mode".into(),
        },
    };
    let tail = ProgressTail {
        agent_id: AGENT,
        at: FIXTURE_NOW_MS,
        line: "test result: ok. 34 passed".into(),
    };
    let node = Audience::Node;
    let peer = Audience::Device(cast.alice_device_cid);
    let tail_msg = device.signed(peer, 4, Payload::ProgressTail(tail), e);
    vec![
        (
            "02-status-update.cbor",
            device.signed(node, 1, Payload::StatusUpdate(status), e),
        ),
        (
            "03-progress-event.cbor",
            device.signed(node, 2, Payload::ProgressEvent(progress), e),
        ),
        ("06-ack.cbor", device.signed(node, 3, Payload::Ack(ack), e)),
        (
            "17-progress-tail.p2p.cbor",
            encode_p2p(&tail_msg).expect("p2p envelope encodes"),
        ),
    ]
}
