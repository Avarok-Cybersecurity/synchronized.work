use std::collections::BTreeSet;

use super::{Actor, Coop, DetachReason, Effect, JoinPolicy, JoinRequest, Refusal, Tier, TierEvent};
use crate::readiness::{Candidate, rank};
use crate::{IdFp, Role, Status};

type Outcome = Result<(Coop, Vec<Effect>), Refusal>;

/// Applies one event to one Task's cooperation state, or refuses it. Pure: the caller stores
/// the new state and logs the effects.
pub fn apply(state: &Coop, actor: &Actor, event: &TierEvent) -> Outcome {
    if state.status != Status::Open {
        return Err(Refusal::Frozen);
    }
    let mut next = state.clone();
    let mut effects = Vec::new();
    match event {
        TierEvent::OpenUp { policy } => {
            owner_only(state, actor)?;
            if state.tier != Tier::Off {
                return Err(Refusal::WrongTier);
            }
            next.tier = Tier::Aware { policy: *policy };
        }
        TierEvent::RequestJoin { readiness, at_ms } => {
            let person = signer(actor)?;
            if matches!(state.tier, Tier::Off) {
                return Err(Refusal::WrongTier);
            }
            if *person == state.owner || is_coworker(state, person) || is_pending(state, person) {
                return Err(Refusal::AlreadyPresent);
            }
            next.pending.push(JoinRequest {
                person: person.clone(),
                readiness: *readiness,
                at_ms: *at_ms,
            });
            effects.push(Effect::JoinRequested {
                person: person.clone(),
            });
        }
        TierEvent::AcceptJoin { person } => {
            owner_only(state, actor)?;
            admit(&mut next, person, &mut effects)?;
        }
        TierEvent::AutoAdmit => {
            if *actor != Actor::ProjectDo {
                return Err(Refusal::NotAuthorized);
            }
            if matches!(policy_of(&state.tier)?, JoinPolicy::Manual) {
                return Err(Refusal::ManualPolicy);
            }
            let person = readiest_pending(state).ok_or(Refusal::NoSuchRequest)?;
            admit(&mut next, &person, &mut effects)?;
        }
        TierEvent::RefuseJoin { person } => {
            owner_only(state, actor)?;
            if is_pending(state, person) {
                next.pending.retain(|r| r.person != *person);
                effects.push(Effect::Refused {
                    person: person.clone(),
                });
            } else if is_coworker(state, person) {
                remove_coworker(&mut next, person);
                effects.push(Effect::Left {
                    person: person.clone(),
                });
            } else {
                return Err(Refusal::NoSuchRequest);
            }
        }
        TierEvent::CoworkerLeft { person } => {
            if *actor != Actor::ProjectDo {
                return Err(Refusal::NotAuthorized);
            }
            if !is_coworker(state, person) {
                return Err(Refusal::NotCoworker);
            }
            remove_coworker(&mut next, person);
            effects.push(Effect::Left {
                person: person.clone(),
            });
        }
        TierEvent::Close => {
            owner_only(state, actor)?;
            if state.tier == Tier::Off {
                return Err(Refusal::WrongTier);
            }
            detach_all(&mut next, DetachReason::OwnerClosed, &mut effects);
        }
        TierEvent::Finish { status } => {
            let (_, role) = signed(actor)?;
            if signer(actor)? != &state.owner && !is_admin(role) {
                return Err(Refusal::NotAuthorized);
            }
            if *status == Status::Open {
                return Err(Refusal::InvalidTarget);
            }
            next.status = *status;
            effects.push(Effect::StatusChanged { status: *status });
        }
        TierEvent::ChangeOwner {
            new_owner,
            new_owner_is_member,
        } => {
            let (_, role) = signed(actor)?;
            if !is_admin(role) {
                return Err(Refusal::NotAuthorized);
            }
            if !new_owner_is_member || *new_owner == state.owner {
                return Err(Refusal::InvalidTarget);
            }
            detach_all(&mut next, DetachReason::OwnerChanged, &mut effects);
            next.owner = new_owner.clone();
            effects.push(Effect::OwnerChanged {
                new_owner: new_owner.clone(),
            });
        }
    }
    if state.tier.name() != next.tier.name() {
        effects.insert(
            0,
            Effect::TierChanged {
                from: state.tier.name(),
                to: next.tier.name(),
            },
        );
    }
    Ok((next, effects))
}

fn signed(actor: &Actor) -> Result<(&IdFp, Role), Refusal> {
    match actor {
        Actor::Signed { id_fp, role } => Ok((id_fp, *role)),
        Actor::Session { .. } => Err(Refusal::Unsigned),
        Actor::ProjectDo => Err(Refusal::NotAuthorized),
    }
}

fn signer(actor: &Actor) -> Result<&IdFp, Refusal> {
    signed(actor).map(|(id, _)| id)
}

fn owner_only(state: &Coop, actor: &Actor) -> Result<(), Refusal> {
    if signer(actor)? == &state.owner {
        Ok(())
    } else {
        Err(Refusal::NotAuthorized)
    }
}

fn is_admin(role: Role) -> bool {
    matches!(role, Role::Owner | Role::Admin)
}

fn policy_of(tier: &Tier) -> Result<JoinPolicy, Refusal> {
    match tier {
        Tier::Off => Err(Refusal::WrongTier),
        Tier::Aware { policy } | Tier::Synchronized { policy, .. } => Ok(*policy),
    }
}

fn coworkers(tier: &Tier) -> usize {
    match tier {
        Tier::Synchronized { coworkers, .. } => coworkers.len(),
        Tier::Off | Tier::Aware { .. } => 0,
    }
}

fn is_coworker(state: &Coop, person: &IdFp) -> bool {
    matches!(&state.tier, Tier::Synchronized { coworkers, .. } if coworkers.contains(person))
}

fn is_pending(state: &Coop, person: &IdFp) -> bool {
    state.pending.iter().any(|r| r.person == *person)
}

/// The same ranking `sw-mcp` uses for its own lanes (design §2.4).
fn readiest_pending(state: &Coop) -> Option<IdFp> {
    let candidates: Vec<Candidate<IdFp>> = state
        .pending
        .iter()
        .map(|r| Candidate {
            key: r.person.clone(),
            readiness: r.readiness,
            since_ms: r.at_ms,
        })
        .collect();
    rank(&candidates).into_iter().next().map(|c| c.key)
}

fn admit(next: &mut Coop, person: &IdFp, effects: &mut Vec<Effect>) -> Result<(), Refusal> {
    let policy = policy_of(&next.tier)?;
    if !is_pending(next, person) {
        return Err(Refusal::NoSuchRequest);
    }
    if let JoinPolicy::Auto { max_coworkers } = policy
        && coworkers(&next.tier) >= usize::from(max_coworkers)
    {
        return Err(Refusal::CapReached);
    }
    next.pending.retain(|r| r.person != *person);
    let mut set = match &next.tier {
        Tier::Synchronized { coworkers, .. } => coworkers.clone(),
        Tier::Off | Tier::Aware { .. } => BTreeSet::new(),
    };
    set.insert(person.clone());
    next.tier = Tier::Synchronized {
        policy,
        coworkers: set,
    };
    effects.push(Effect::Admitted {
        person: person.clone(),
    });
    Ok(())
}

fn remove_coworker(next: &mut Coop, person: &IdFp) {
    if let Tier::Synchronized { policy, coworkers } = &next.tier {
        let mut rest = coworkers.clone();
        rest.remove(person);
        next.tier = if rest.is_empty() {
            Tier::Aware { policy: *policy }
        } else {
            Tier::Synchronized {
                policy: *policy,
                coworkers: rest,
            }
        };
    }
}

fn detach_all(next: &mut Coop, reason: DetachReason, effects: &mut Vec<Effect>) {
    for request in next.pending.drain(..) {
        effects.push(Effect::Refused {
            person: request.person,
        });
    }
    if let Tier::Synchronized { coworkers, .. } = &next.tier {
        for person in coworkers {
            effects.push(Effect::Detached {
                person: person.clone(),
                reason,
            });
        }
    }
    next.tier = Tier::Off;
}
