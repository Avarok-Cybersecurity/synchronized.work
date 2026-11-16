#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The §2.5 transition table, one row per (state, actor, event).

use std::collections::BTreeSet;

use sw_model::readiness::Readiness;
use sw_model::tier::{
    Actor, Coop, DetachReason, Effect, JoinPolicy, JoinRequest, Refusal, Tier, TierEvent, apply,
};
use sw_model::{IdFp, Role, Status};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn fp(c: char) -> IdFp {
    IdFp::parse(&c.to_string().repeat(16)).unwrap()
}
fn owner() -> Actor {
    Actor::Signed {
        id_fp: fp('o'),
        role: Role::Member,
    }
}
fn signed(c: char, role: Role) -> Actor {
    Actor::Signed { id_fp: fp(c), role }
}
fn req(c: char, milli: u16, at: u64) -> JoinRequest {
    JoinRequest {
        person: fp(c),
        readiness: Readiness::from_milli(milli).unwrap(),
        at_ms: at,
    }
}
const MANUAL: JoinPolicy = JoinPolicy::Manual;
const AUTO1: JoinPolicy = JoinPolicy::Auto { max_coworkers: 1 };

fn coop(tier: Tier, pending: Vec<JoinRequest>) -> Coop {
    Coop {
        owner: fp('o'),
        status: Status::Open,
        tier,
        pending,
    }
}
fn sync(policy: JoinPolicy, who: &[char]) -> Tier {
    Tier::Synchronized {
        policy,
        coworkers: who.iter().map(|c| fp(*c)).collect::<BTreeSet<_>>(),
    }
}

type Row = (
    &'static str,
    Coop,
    Actor,
    TierEvent,
    Result<&'static str, Refusal>,
);

fn rows() -> Vec<Row> {
    use TierEvent::*;
    let aware = |p| coop(Tier::Aware { policy: p }, vec![]);
    let aware_pending = |p| coop(Tier::Aware { policy: p }, vec![req('b', 1000, 5)]);
    vec![
        (
            "owner opens up",
            coop(Tier::Off, vec![]),
            owner(),
            OpenUp { policy: MANUAL },
            Ok("aware"),
        ),
        (
            "unsigned open up",
            coop(Tier::Off, vec![]),
            Actor::Session { id_fp: fp('o') },
            OpenUp { policy: MANUAL },
            Err(Refusal::Unsigned),
        ),
        (
            "non-owner open up",
            coop(Tier::Off, vec![]),
            signed('b', Role::Admin),
            OpenUp { policy: MANUAL },
            Err(Refusal::NotAuthorized),
        ),
        (
            "DO open up",
            coop(Tier::Off, vec![]),
            Actor::ProjectDo,
            OpenUp { policy: MANUAL },
            Err(Refusal::NotAuthorized),
        ),
        (
            "open up twice",
            aware(MANUAL),
            owner(),
            OpenUp { policy: MANUAL },
            Err(Refusal::WrongTier),
        ),
        (
            "join an off task",
            coop(Tier::Off, vec![]),
            signed('b', Role::Member),
            RequestJoin {
                readiness: Readiness::READY,
                at_ms: 1,
            },
            Err(Refusal::WrongTier),
        ),
        (
            "join an aware task",
            aware(MANUAL),
            signed('b', Role::Member),
            RequestJoin {
                readiness: Readiness::READY,
                at_ms: 1,
            },
            Ok("aware"),
        ),
        (
            "owner joins own task",
            aware(MANUAL),
            owner(),
            RequestJoin {
                readiness: Readiness::READY,
                at_ms: 1,
            },
            Err(Refusal::AlreadyPresent),
        ),
        (
            "join twice",
            aware_pending(MANUAL),
            signed('b', Role::Member),
            RequestJoin {
                readiness: Readiness::READY,
                at_ms: 1,
            },
            Err(Refusal::AlreadyPresent),
        ),
        (
            "owner accepts",
            aware_pending(MANUAL),
            owner(),
            AcceptJoin { person: fp('b') },
            Ok("synchronized"),
        ),
        (
            "accept without request",
            aware(MANUAL),
            owner(),
            AcceptJoin { person: fp('b') },
            Err(Refusal::NoSuchRequest),
        ),
        (
            "co-worker accepts",
            aware_pending(MANUAL),
            signed('c', Role::Admin),
            AcceptJoin { person: fp('b') },
            Err(Refusal::NotAuthorized),
        ),
        (
            "DO auto-admits",
            aware_pending(AUTO1),
            Actor::ProjectDo,
            AutoAdmit,
            Ok("synchronized"),
        ),
        (
            "DO auto-admits under manual",
            aware_pending(MANUAL),
            Actor::ProjectDo,
            AutoAdmit,
            Err(Refusal::ManualPolicy),
        ),
        (
            "owner forges auto-admit",
            aware_pending(AUTO1),
            owner(),
            AutoAdmit,
            Err(Refusal::NotAuthorized),
        ),
        (
            "auto cap reached",
            coop(sync(AUTO1, &['c']), vec![req('b', 1000, 5)]),
            Actor::ProjectDo,
            AutoAdmit,
            Err(Refusal::CapReached),
        ),
        (
            "accept past auto cap",
            coop(sync(AUTO1, &['c']), vec![req('b', 1000, 5)]),
            owner(),
            AcceptJoin { person: fp('b') },
            Err(Refusal::CapReached),
        ),
        (
            "last co-worker leaves",
            coop(sync(MANUAL, &['b']), vec![]),
            Actor::ProjectDo,
            CoworkerLeft { person: fp('b') },
            Ok("aware"),
        ),
        (
            "one of two leaves",
            coop(sync(MANUAL, &['b', 'c']), vec![]),
            Actor::ProjectDo,
            CoworkerLeft { person: fp('b') },
            Ok("synchronized"),
        ),
        (
            "stranger leaves",
            coop(sync(MANUAL, &['b']), vec![]),
            Actor::ProjectDo,
            CoworkerLeft { person: fp('c') },
            Err(Refusal::NotCoworker),
        ),
        (
            "person forges leave",
            coop(sync(MANUAL, &['b']), vec![]),
            signed('b', Role::Member),
            CoworkerLeft { person: fp('b') },
            Err(Refusal::NotAuthorized),
        ),
        (
            "owner detaches co-worker",
            coop(sync(MANUAL, &['b']), vec![]),
            owner(),
            RefuseJoin { person: fp('b') },
            Ok("aware"),
        ),
        (
            "owner refuses pending",
            aware_pending(MANUAL),
            owner(),
            RefuseJoin { person: fp('b') },
            Ok("aware"),
        ),
        (
            "owner closes synchronized",
            coop(sync(MANUAL, &['b']), vec![]),
            owner(),
            Close,
            Ok("off"),
        ),
        (
            "owner closes aware",
            aware(MANUAL),
            owner(),
            Close,
            Ok("off"),
        ),
        (
            "close an off task",
            coop(Tier::Off, vec![]),
            owner(),
            Close,
            Err(Refusal::WrongTier),
        ),
        (
            "unsigned close",
            aware(MANUAL),
            Actor::Session { id_fp: fp('o') },
            Close,
            Err(Refusal::Unsigned),
        ),
        (
            "owner finishes",
            aware(MANUAL),
            owner(),
            Finish {
                status: Status::Done,
            },
            Ok("aware"),
        ),
        (
            "admin cancels",
            aware(MANUAL),
            signed('a', Role::Admin),
            Finish {
                status: Status::Cancelled,
            },
            Ok("aware"),
        ),
        (
            "member finishes",
            aware(MANUAL),
            signed('b', Role::Member),
            Finish {
                status: Status::Done,
            },
            Err(Refusal::NotAuthorized),
        ),
        (
            "finish to open",
            aware(MANUAL),
            owner(),
            Finish {
                status: Status::Open,
            },
            Err(Refusal::InvalidTarget),
        ),
        (
            "admin changes owner",
            coop(sync(MANUAL, &['b']), vec![]),
            signed('a', Role::Admin),
            ChangeOwner {
                new_owner: fp('c'),
                new_owner_is_member: true,
            },
            Ok("off"),
        ),
        (
            "owner (member role) changes owner",
            aware(MANUAL),
            owner(),
            ChangeOwner {
                new_owner: fp('c'),
                new_owner_is_member: true,
            },
            Err(Refusal::NotAuthorized),
        ),
        (
            "owner change to non-member",
            aware(MANUAL),
            signed('a', Role::Admin),
            ChangeOwner {
                new_owner: fp('c'),
                new_owner_is_member: false,
            },
            Err(Refusal::InvalidTarget),
        ),
    ]
}

#[test]
fn transition_table() {
    for (name, state, actor, event, expect) in rows() {
        let got = apply(&state, &actor, &event).map(|(next, _)| next.tier.name());
        assert_eq!(got, expect, "row {name:?}");
    }
}

#[test]
fn a_done_task_is_frozen_for_every_event() {
    for (name, mut state, actor, event, _) in rows() {
        state.status = Status::Done;
        assert_eq!(
            apply(&state, &actor, &event),
            Err(Refusal::Frozen),
            "row {name:?}"
        );
    }
}

#[test]
fn closing_detaches_coworkers_and_refuses_pending_with_owner_closed() {
    let state = coop(sync(MANUAL, &['b']), vec![req('c', 1000, 1)]);
    let (next, effects) = apply(&state, &owner(), &TierEvent::Close).unwrap();
    assert!(next.pending.is_empty());
    assert_eq!(
        effects,
        vec![
            Effect::TierChanged {
                from: "synchronized",
                to: "off"
            },
            Effect::Refused { person: fp('c') },
            Effect::Detached {
                person: fp('b'),
                reason: DetachReason::OwnerClosed
            },
        ]
    );
}

#[test]
fn auto_admission_takes_the_readiest_request_not_the_first() {
    let state = coop(
        Tier::Aware {
            policy: JoinPolicy::Auto { max_coworkers: 3 },
        },
        vec![req('b', 200, 1), req('c', 1000, 9), req('d', 1000, 4)],
    );
    let (next, effects) = apply(&state, &Actor::ProjectDo, &TierEvent::AutoAdmit).unwrap();
    assert!(effects.contains(&Effect::Admitted { person: fp('d') }));
    assert_eq!(next.pending.len(), 2);
}

#[test]
fn owner_change_turns_the_tier_off_and_repoints_the_owner() {
    let state = coop(sync(MANUAL, &['b']), vec![]);
    let event = TierEvent::ChangeOwner {
        new_owner: fp('c'),
        new_owner_is_member: true,
    };
    let (next, effects) = apply(&state, &signed('a', Role::Owner), &event).unwrap();
    assert_eq!(next.owner, fp('c'));
    assert!(effects.contains(&Effect::Detached {
        person: fp('b'),
        reason: DetachReason::OwnerChanged
    }));
}
