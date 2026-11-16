//! Fixtures a Controller signs: 4, 5 (sealed to Alice's Device), 9, 10 and 16 `LINK_CODE`.

use sw_model::tier::JoinPolicy;
use sw_model::{Id128, Visibility};
use sw_protocol::body::Audience;
use sw_protocol::payload::*;
use sw_protocol::sealed::Sealed;

use super::{AGENT, Fixture, Issuer, TASK};
use crate::{Cast, LabelledEntropy};

pub(super) fn fixtures(cast: &Cast, e: &mut LabelledEntropy) -> Vec<Fixture> {
    let ctl = Issuer {
        key: &cast.alice_controller,
        iss: cast.alice.id_fp(),
        from: cast.alice_controller_cid,
    };
    let ek = cast.alice_device_kem.encapsulation_key();
    let device = cast.alice_device_cid;
    let prompt = PromptInner {
        task_id: TASK,
        agent_id: Some(AGENT),
        text: "run the tier table tests".into(),
        mode: PromptMode::Queue,
        reply_to: None,
    };
    let control = ControlInner {
        op: ControlCode::SetTier,
        task_id: Some(TASK),
        open_with: Some(JoinPolicy::Auto { max_coworkers: 2 }),
        request_id: None,
        reason: None,
    };
    let sealed_prompt = SealedPayload {
        sealed: Sealed::seal(ek, device, &prompt, e).expect("seals"),
    };
    let sealed_control = SealedPayload {
        sealed: Sealed::seal(ek, device, &control, e).expect("seals"),
    };
    let answer = JoinAnswer {
        request_id: Id128([0xd4; 16]),
        accept: true,
        reason: None,
    };
    let link = Link {
        op: LinkOp::Code,
        code: None,
        acting_username: None,
        install_id: None,
        node_id: None,
    };
    let to_device = Audience::Device(device);
    let node = Audience::Node;
    vec![
        (
            "04-prompt.cbor",
            ctl.signed(to_device, 1, Payload::Prompt(sealed_prompt), e),
        ),
        (
            "05-control.cbor",
            ctl.signed(to_device, 2, Payload::Control(sealed_control), e),
        ),
        (
            "09-join-answer.cbor",
            ctl.signed(node, 3, Payload::JoinAnswer(answer), e),
        ),
        (
            "10-task-write.cbor",
            ctl.signed(node, 4, Payload::TaskWrite(task_create()), e),
        ),
        (
            "16-link-code.cbor",
            ctl.signed(node, 5, Payload::Link(link), e),
        ),
    ]
}

fn task_create() -> TaskWrite {
    let (title, description) = (
        "Port the WAL",
        "durable write-ahead log for latticedb on wasm32",
    );
    TaskWrite {
        task_id: TASK,
        op: TaskOp::Create,
        title: Some(title.into()),
        description: Some(description.into()),
        visibility: Some(Visibility::Members),
        parent: None,
        owner: None,
        status: None,
        embedding: Some(sw_model::embedding::buckets(title, description).to_vec()),
    }
}
