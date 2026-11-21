//! Invitations (type 23) and what they open: a `JOIN_REQUEST` for a project's root Task must
//! spend one live invite of that project, once. The node keeps each invite's token hash and
//! expiry only; the owner's signed `JOIN_ANSWER` still decides admission (design §2.5).

use std::collections::BTreeMap;

use sw_model::IdFp;
use sw_model::tier::{JoinPolicy, Tier, TierName};
use sw_protocol::gate::Verified;
use sw_protocol::invite::token_hash;
use sw_protocol::payload::invite::invite_limits::{LIVE_PER_PROJECT, MAX_LIFETIME_MS};
use sw_protocol::payload::{Invite, JoinRequest, TaskFields};

use super::pad::ledger::hex;
use super::{Effect, NodeCore, NodeRefusal};

/// Live invites: project → token hash → `exp`.
#[derive(Debug, Default)]
pub struct Invites {
    live: BTreeMap<String, BTreeMap<[u8; 32], u64>>,
}

pub fn invite_key(project: &str, hash: &[u8; 32]) -> String {
    format!("invite/{}/{}", hex(project.as_bytes()), hex(hash))
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

impl NodeCore {
    /// The owner's invite: stored by hash; the first one opens the project to join requests
    /// (`off → aware`, manual admission) through the tier machine, signed by the same owner.
    pub(super) fn invite(
        &mut self,
        v: &Verified,
        invite: &Invite,
        now_ms: u64,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        let project = &invite.project;
        let root = self.tree.get(project).ok_or(NodeRefusal::NoSuchTask)?;
        if root.parent.is_some() {
            return Err(NodeRefusal::NotAProject);
        }
        if self.tree.project_owner(root) != v.body.iss {
            return Err(NodeRefusal::Tier(sw_model::tier::Refusal::NotAuthorized));
        }
        if invite.exp <= now_ms || invite.exp - now_ms > MAX_LIFETIME_MS {
            return Err(NodeRefusal::InviteLifetime);
        }
        let live = self.invites.live.get(project);
        if live.is_some_and(|m| m.contains_key(&invite.token_hash)) {
            return Err(NodeRefusal::AlreadyExists);
        }
        let count = live.map_or(0, |m| m.values().filter(|exp| **exp > now_ms).count());
        if count >= LIVE_PER_PROJECT {
            return Err(NodeRefusal::CapReached("invites"));
        }
        let mut effects = Vec::new();
        if root.coop.tier == Tier::Off {
            let open = TaskFields {
                tier: Some(TierName::Aware),
                join_policy: Some(JoinPolicy::Manual),
                ..TaskFields::EMPTY
            };
            let change = self
                .tree
                .update_fields(&v.body.iss, project, &open, now_ms)?;
            effects.extend(self.apply_change(change, now_ms)?);
        }
        effects.extend(self.prune_invites(project, now_ms));
        self.invites
            .live
            .entry(project.clone())
            .or_default()
            .insert(invite.token_hash, invite.exp);
        effects.push(Effect::Put {
            key: invite_key(project, &invite.token_hash),
            value: invite.exp.to_be_bytes().to_vec(),
        });
        Ok(effects)
    }

    /// A join request. For a project's root it must spend a live invite of that project,
    /// which is deleted only once the tier machine accepted the request.
    pub(super) fn join(
        &mut self,
        iss: &IdFp,
        request: &JoinRequest,
        signal_id: [u8; 32],
        now_ms: u64,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        let task = self
            .tree
            .get(&request.task_id)
            .ok_or(NodeRefusal::NoSuchTask)?;
        let spend = match (task.parent.is_none(), &request.invite) {
            (true, Some(token)) => {
                let hash = token_hash(token);
                let live = self
                    .invites
                    .live
                    .get(&request.task_id)
                    .and_then(|m| m.get(&hash))
                    .is_some_and(|exp| *exp > now_ms);
                if !live {
                    return Err(NodeRefusal::InviteInvalid);
                }
                Some(hash)
            }
            (true, None) => return Err(NodeRefusal::NeedsInvite),
            (false, Some(_)) => return Err(NodeRefusal::Malformed),
            (false, None) => None,
        };
        let change = self.tree.join_request(iss, request, signal_id, now_ms)?;
        let mut effects = self.apply_change(change, now_ms)?;
        if let Some(hash) = spend {
            if let Some(m) = self.invites.live.get_mut(&request.task_id) {
                m.remove(&hash);
            }
            effects.push(Effect::Delete {
                key: invite_key(&request.task_id, &hash),
            });
        }
        Ok(effects)
    }

    /// Forgets the project's expired invites.
    fn prune_invites(&mut self, project: &str, now_ms: u64) -> Vec<Effect> {
        let Some(m) = self.invites.live.get_mut(project) else {
            return Vec::new();
        };
        let gone: Vec<[u8; 32]> = m
            .iter()
            .filter(|(_, exp)| **exp <= now_ms)
            .map(|(h, _)| *h)
            .collect();
        gone.iter()
            .map(|h| {
                m.remove(h);
                Effect::Delete {
                    key: invite_key(project, h),
                }
            })
            .collect()
    }

    /// Loads one invite key; `Ok(false)` if `key` is not one.
    pub(crate) fn invite_load(&mut self, key: &str, value: &[u8]) -> Result<bool, String> {
        let Some(rest) = key.strip_prefix("invite/") else {
            return Ok(false);
        };
        let (project, hash) = rest.split_once('/').ok_or("bad invite key")?;
        let project = unhex(project)
            .and_then(|b| String::from_utf8(b).ok())
            .ok_or("bad invite project")?;
        let hash: [u8; 32] = unhex(hash)
            .and_then(|b| b.try_into().ok())
            .ok_or("bad invite hash")?;
        let exp: [u8; 8] = value.try_into().map_err(|_| "bad invite exp")?;
        self.invites
            .live
            .entry(project)
            .or_default()
            .insert(hash, u64::from_be_bytes(exp));
        Ok(true)
    }
}
