#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The §2.5 transition table, one row per (state, actor, event).

mod support;

use support::*;
use sw_model::readiness::Readiness;
use sw_model::tier::{Actor, Coop, Refusal, Tier, TierEvent, apply};
use sw_model::{Role, Status};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

type Row = (
    &'static str,
    Coop,
    Actor,
    TierEvent,
    Result<&'static str, Refusal>,
);

#[rustfmt::skip] // a table reads as a table
fn rows() -> Vec<Row> {
    use TierEvent::*;
    let off = || coop(Tier::Off, vec![]);
    let join = || RequestJoin { readiness: Readiness::READY, at_ms: 1 };
    let to_c = |member| ChangeOwner { new_owner: fp('c'), new_owner_is_member: member };
    let b = || fp('b');
    vec![
        ("owner opens up",              off(), owner(), OpenUp { policy: MANUAL }, Ok("aware")),
        ("unsigned open up",            off(), session('o'), OpenUp { policy: MANUAL }, Err(Refusal::Unsigned)),
        ("non-owner admin opens up",    off(), signed('b', Role::Admin), OpenUp { policy: MANUAL }, Err(Refusal::NotAuthorized)),
        ("DO opens up",                 off(), Actor::ProjectDo, OpenUp { policy: MANUAL }, Err(Refusal::NotAuthorized)),
        ("open up twice",               aware(MANUAL), owner(), OpenUp { policy: MANUAL }, Err(Refusal::WrongTier)),
        ("join an off task",            off(), signed('b', Role::Member), join(), Err(Refusal::WrongTier)),
        ("join an aware task",          aware(MANUAL), signed('b', Role::Member), join(), Ok("aware")),
        ("unsigned join",               aware(MANUAL), session('b'), join(), Err(Refusal::Unsigned)),
        ("owner joins own task",        aware(MANUAL), owner(), join(), Err(Refusal::AlreadyPresent)),
        ("join twice",                  aware_pending(MANUAL), signed('b', Role::Member), join(), Err(Refusal::AlreadyPresent)),
        ("co-worker joins again",       coop(sync(MANUAL, &['b']), vec![]), signed('b', Role::Member), join(), Err(Refusal::AlreadyPresent)),
        ("owner accepts",               aware_pending(MANUAL), owner(), AcceptJoin { person: b() }, Ok("synchronized")),
        ("unsigned accept",             aware_pending(MANUAL), session('o'), AcceptJoin { person: b() }, Err(Refusal::Unsigned)),
        ("accept without request",      aware(MANUAL), owner(), AcceptJoin { person: b() }, Err(Refusal::NoSuchRequest)),
        ("admin accepts",               aware_pending(MANUAL), signed('c', Role::Admin), AcceptJoin { person: b() }, Err(Refusal::NotAuthorized)),
        ("DO auto-admits",              aware_pending(AUTO1), Actor::ProjectDo, AutoAdmit, Ok("synchronized")),
        ("auto-admit under manual",     aware_pending(MANUAL), Actor::ProjectDo, AutoAdmit, Err(Refusal::ManualPolicy)),
        ("auto-admit with none pending", aware(AUTO1), Actor::ProjectDo, AutoAdmit, Err(Refusal::NoSuchRequest)),
        ("owner forges auto-admit",     aware_pending(AUTO1), owner(), AutoAdmit, Err(Refusal::NotAuthorized)),
        ("auto cap reached",            coop(sync(AUTO1, &['c']), vec![req('b', 1000, 5)]), Actor::ProjectDo, AutoAdmit, Err(Refusal::CapReached)),
        ("accept past auto cap",        coop(sync(AUTO1, &['c']), vec![req('b', 1000, 5)]), owner(), AcceptJoin { person: b() }, Err(Refusal::CapReached)),
        ("last co-worker leaves",       coop(sync(MANUAL, &['b']), vec![]), Actor::ProjectDo, CoworkerLeft { person: b() }, Ok("aware")),
        ("one of two leaves",           coop(sync(MANUAL, &['b', 'c']), vec![]), Actor::ProjectDo, CoworkerLeft { person: b() }, Ok("synchronized")),
        ("stranger leaves",             coop(sync(MANUAL, &['b']), vec![]), Actor::ProjectDo, CoworkerLeft { person: fp('c') }, Err(Refusal::NotCoworker)),
        ("person forges leave",         coop(sync(MANUAL, &['b']), vec![]), signed('b', Role::Member), CoworkerLeft { person: b() }, Err(Refusal::NotAuthorized)),
        ("owner detaches co-worker",    coop(sync(MANUAL, &['b']), vec![]), owner(), RefuseJoin { person: b() }, Ok("aware")),
        ("owner refuses pending",       aware_pending(MANUAL), owner(), RefuseJoin { person: b() }, Ok("aware")),
        ("refuse nobody",               aware(MANUAL), owner(), RefuseJoin { person: b() }, Err(Refusal::NoSuchRequest)),
        ("owner closes synchronized",   coop(sync(MANUAL, &['b']), vec![]), owner(), Close, Ok("off")),
        ("owner closes aware",          aware(MANUAL), owner(), Close, Ok("off")),
        ("close an off task",           off(), owner(), Close, Err(Refusal::WrongTier)),
        ("unsigned close",              aware(MANUAL), session('o'), Close, Err(Refusal::Unsigned)),
        ("co-worker closes",            coop(sync(MANUAL, &['b']), vec![]), signed('b', Role::Member), Close, Err(Refusal::NotAuthorized)),
        ("owner finishes",              aware(MANUAL), owner(), Finish { status: Status::Done }, Ok("aware")),
        ("admin cancels",               aware(MANUAL), signed('a', Role::Admin), Finish { status: Status::Cancelled }, Ok("aware")),
        ("member finishes",             aware(MANUAL), signed('b', Role::Member), Finish { status: Status::Done }, Err(Refusal::NotAuthorized)),
        ("unsigned finish",             aware(MANUAL), session('o'), Finish { status: Status::Done }, Err(Refusal::Unsigned)),
        ("finish to open",              aware(MANUAL), owner(), Finish { status: Status::Open }, Err(Refusal::InvalidTarget)),
        ("admin changes owner",         coop(sync(MANUAL, &['b']), vec![]), signed('a', Role::Admin), to_c(true), Ok("off")),
        ("owner (member) changes owner", aware(MANUAL), owner(), to_c(true), Err(Refusal::NotAuthorized)),
        ("owner change to non-member",  aware(MANUAL), signed('a', Role::Admin), to_c(false), Err(Refusal::InvalidTarget)),
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
fn no_event_from_a_bare_session_ever_succeeds() {
    for (name, state, _, event, _) in rows() {
        let got = apply(&state, &session('o'), &event);
        assert!(got.is_err(), "row {name:?} accepted an unsigned owner");
    }
}
