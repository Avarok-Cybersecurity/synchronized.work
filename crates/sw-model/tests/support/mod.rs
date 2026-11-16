//! Shared builders for the tier tests: Persons are named by one letter (`'o'` owns the Task).
#![allow(dead_code, clippy::unwrap_used)]

use std::collections::BTreeSet;

use sw_model::readiness::Readiness;
use sw_model::tier::{Actor, Coop, JoinPolicy, JoinRequest, Tier};
use sw_model::{IdFp, Role, Status};

pub const MANUAL: JoinPolicy = JoinPolicy::Manual;
pub const AUTO1: JoinPolicy = JoinPolicy::Auto { max_coworkers: 1 };

pub fn fp(c: char) -> IdFp {
    IdFp::parse(&c.to_string().repeat(16)).unwrap()
}

/// The owner signing with an ordinary member role: ownership, not role, authorises.
pub fn owner() -> Actor {
    signed('o', Role::Member)
}

pub fn signed(c: char, role: Role) -> Actor {
    Actor::Signed { id_fp: fp(c), role }
}

pub fn session(c: char) -> Actor {
    Actor::Session { id_fp: fp(c) }
}

pub fn req(c: char, permille: u16, at: u64) -> JoinRequest {
    let readiness = Readiness::from_permille(permille).unwrap();
    JoinRequest {
        person: fp(c),
        readiness,
        at_ms: at,
    }
}

pub fn coop(tier: Tier, pending: Vec<JoinRequest>) -> Coop {
    Coop {
        owner: fp('o'),
        status: Status::Open,
        tier,
        pending,
    }
}

pub fn aware(policy: JoinPolicy) -> Coop {
    coop(Tier::Aware { policy }, vec![])
}

pub fn aware_pending(policy: JoinPolicy) -> Coop {
    coop(Tier::Aware { policy }, vec![req('b', 1000, 5)])
}

pub fn sync(policy: JoinPolicy, who: &[char]) -> Tier {
    let coworkers = who.iter().map(|c| fp(*c)).collect::<BTreeSet<_>>();
    Tier::Synchronized { policy, coworkers }
}
