#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! What transitions do besides changing the tier: detach, refuse, rank, re-point.

mod support;

use support::*;
use sw_model::Role;
use sw_model::tier::{Actor, DetachReason, Effect, JoinPolicy, Tier, TierEvent, apply};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

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
    let policy = JoinPolicy::Auto { max_coworkers: 3 };
    let pending = vec![req('b', 200, 1), req('c', 1000, 9), req('d', 1000, 4)];
    let state = coop(Tier::Aware { policy }, pending);
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
    let detached = Effect::Detached {
        person: fp('b'),
        reason: DetachReason::OwnerChanged,
    };
    assert!(effects.contains(&detached));
}
