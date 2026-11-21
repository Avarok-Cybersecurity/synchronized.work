//! The developer scratchpad's rules at the node (types 18–22). The node checks who may write
//! to whom and how often, stores the signed bytes and relays them; it never opens a body
//! (each is sealed to the recipients' Devices) and readers verify every note themselves.

mod ack;
mod deliver;
pub mod ledger;
mod load;

use std::collections::BTreeSet;

use sw_model::{Cid, EndpointKind, IdFp};
use sw_protocol::gate::{Verified, verify_proof};
use sw_protocol::payload::{PadNote, PadPolicy, PadReceipt, PadScope, Payload};
use sw_protocol::signed::SwMessage;

use super::{Effect, NodeCore, NodeRefusal};
use crate::identity::CitadelIdentity;
use ledger::{Entry, note_key, policy_key, policy_value};

/// Why the scratchpad refused a write; the sender gets it in a `PAD_RECEIPT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadRefusal {
    NoSuchProject,
    /// The project owner has not set `PAD_POLICY` (no retention is assumed).
    NoPolicy,
    /// `exp - iat` exceeds the project's retention.
    TooLong,
    /// The carried proof is not the signing Device's.
    BadProof,
    NotAMember,
    NoTargets,
    /// A sealed key for a CID that is not a target's Device, or a target with none.
    BadRecipients,
    RateLimited,
    PendingFull,
    NoSuchNote,
    NotATarget,
    AlreadyAcked,
    NotTheOwner,
}

impl PadRefusal {
    pub fn reason(self) -> String {
        format!("{self:?}")
    }
}

impl From<PadRefusal> for NodeRefusal {
    fn from(r: PadRefusal) -> Self {
        Self::Pad(r)
    }
}

impl NodeCore {
    /// The members of a project: its owner and every co-worker admitted to one of its tasks
    /// (admission is the owner's decision). Owning a child task is not membership: the tree
    /// lets any Person create one. `None` if no such project.
    pub fn pad_members(&self, project: &str) -> Option<BTreeSet<IdFp>> {
        let root = self.tree.get(project)?;
        if root.parent.is_some() {
            return None;
        }
        let mut out = BTreeSet::from([self.tree.project_owner(root)]);
        for task in self.tree.tasks.values().filter(|t| t.project == project) {
            out.extend(task.coworkers());
        }
        Some(out)
    }

    pub(super) fn pad_receipt(
        to: Cid,
        project: &str,
        v: &Verified,
        refused: Option<PadRefusal>,
    ) -> Result<Effect, NodeRefusal> {
        let receipt = PadReceipt {
            project: project.to_owned(),
            id: v.signal_id,
            typ: v.body.payload.typ(),
            stored: refused.is_none(),
            reason: refused.map(PadRefusal::reason),
        };
        let bytes = SwMessage::Unsigned(Payload::PadReceipt(receipt)).to_bytes()?;
        Ok(Effect::Send { to, bytes })
    }

    /// The carried proof must be the very Device that signed.
    fn pad_proof(&self, v: &Verified, proof: &[u8]) -> Result<(), PadRefusal> {
        let issuer = &self.rules.sign_in_issuer;
        let bound =
            verify_proof(proof, issuer, &CitadelIdentity).map_err(|_| PadRefusal::BadProof)?;
        let same = sw_crypto::kid_of(&bound.sign_pk) == v.kid
            && bound.cid == v.body.from
            && bound.id_fp == v.body.iss;
        same.then_some(()).ok_or(PadRefusal::BadProof)
    }

    pub(super) fn pad_retention(&self, project: &str) -> Result<u64, PadRefusal> {
        self.pad
            .projects
            .get(project)
            .and_then(|p| p.retention_ms)
            .ok_or(PadRefusal::NoPolicy)
    }

    pub(super) fn pad_policy(
        &mut self,
        v: &Verified,
        p: &PadPolicy,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        let root = self.tree.get(&p.project).ok_or(PadRefusal::NoSuchProject)?;
        if root.parent.is_some() {
            return Err(PadRefusal::NoSuchProject.into());
        }
        if self.tree.project_owner(root) != v.body.iss {
            return Err(PadRefusal::NotTheOwner.into());
        }
        self.pad
            .projects
            .entry(p.project.clone())
            .or_default()
            .retention_ms = Some(p.retention_ms);
        Ok(vec![Effect::Put {
            key: policy_key(&p.project),
            value: policy_value(p.retention_ms),
        }])
    }

    fn pad_targets(&self, iss: &IdFp, n: &PadNote) -> Result<BTreeSet<IdFp>, PadRefusal> {
        let members = self
            .pad_members(&n.project)
            .ok_or(PadRefusal::NoSuchProject)?;
        if !members.contains(iss) {
            return Err(PadRefusal::NotAMember);
        }
        let targets: BTreeSet<IdFp> = match n.scope {
            PadScope::All => members.iter().filter(|m| *m != iss).cloned().collect(),
            PadScope::Members => n.to.iter().cloned().collect(),
        };
        if targets.is_empty() || targets.contains(iss) {
            return Err(PadRefusal::NoTargets);
        }
        if !targets.is_subset(&members) {
            return Err(PadRefusal::NotAMember);
        }
        let mut covered = BTreeSet::new();
        for key in &n.body.keys {
            match self.directory.endpoint(key.cid) {
                Some((p, EndpointKind::Device)) if targets.contains(p) => covered.insert(p.clone()),
                _ => return Err(PadRefusal::BadRecipients),
            };
        }
        if covered != targets {
            return Err(PadRefusal::BadRecipients);
        }
        Ok(targets)
    }

    pub(super) fn pad_note(
        &mut self,
        v: &Verified,
        n: &PadNote,
        bytes: &[u8],
        now_ms: u64,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        let retention = self.pad_retention(&n.project)?;
        if v.body.exp.saturating_sub(v.body.iat) > retention {
            return Err(PadRefusal::TooLong.into());
        }
        self.pad_proof(v, &n.proof.0)?;
        let targets = self.pad_targets(&v.body.iss, n)?;
        let max = self.rules.limits.pad_pending_max;
        let full = self.pad.projects.get(&n.project).is_some_and(|project| {
            targets.iter().any(|t| {
                let live = |e: &&Entry| e.exp > now_ms && e.pending_for(t);
                project.notes.values().filter(live).count() >= max
            })
        });
        if full {
            return Err(PadRefusal::PendingFull.into());
        }
        let rate = self.rules.limits.pad_rate_per_min;
        if !self.pad.take_rate(&v.body.iss, now_ms, rate) {
            return Err(PadRefusal::RateLimited.into());
        }
        let mut effects = self.pad_prune(&n.project, now_ms);
        let entry = Entry::new(bytes.to_vec(), targets, now_ms)?;
        effects.push(Effect::Put {
            key: note_key(&n.project, &v.signal_id),
            value: entry.encode()?,
        });
        for cid in entry.devices.iter().filter(|c| self.online.contains(c)) {
            effects.push(Effect::Send {
                to: *cid,
                bytes: bytes.to_vec(),
            });
        }
        if let Some(project) = self.pad.projects.get_mut(&n.project) {
            project.notes.insert(v.signal_id, entry);
        }
        effects.push(Self::pad_receipt(v.body.from, &n.project, v, None)?);
        Ok(effects)
    }
}
