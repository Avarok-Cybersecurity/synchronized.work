//! What the scratchpad sends without being asked: pending notes and acks when a Device's
//! session opens, the member directory on request, and the deletion of expired entries.

use sw_model::{Cid, EndpointKind};
use sw_protocol::payload::{PadDirectory, PadMember, Payload};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Bytes;

use super::PadRefusal;
use super::ledger::{ack_key, note_key};
use crate::core::{Effect, NodeCore, NodeRefusal};

impl NodeCore {
    /// Drops the project's expired notes and their acks.
    pub(crate) fn pad_prune(&mut self, project: &str, now_ms: u64) -> Vec<Effect> {
        let Some(p) = self.pad.projects.get_mut(project) else {
            return Vec::new();
        };
        let mut effects = Vec::new();
        p.notes.retain(|id, e| {
            if e.exp > now_ms {
                return true;
            }
            effects.push(Effect::Delete {
                key: note_key(project, id),
            });
            for by in e.acks.keys() {
                effects.push(Effect::Delete {
                    key: ack_key(project, id, by),
                });
            }
            false
        });
        effects
    }

    /// A Device came online: every note still pending for its Person and sealed to it, and
    /// every ack of a note it sent. At least once; readers de-duplicate by id.
    pub(crate) fn pad_connected(&mut self, cid: Cid, now_ms: u64) -> Vec<Effect> {
        let projects: Vec<String> = self.pad.projects.keys().cloned().collect();
        let mut effects = Vec::new();
        for project in &projects {
            effects.extend(self.pad_prune(project, now_ms));
        }
        let person = self.directory.endpoint(cid).map(|(p, _)| p.clone());
        for p in self.pad.projects.values() {
            for e in p.notes.values() {
                let for_me = person.as_ref().is_some_and(|me| e.pending_for(me));
                if for_me && e.devices.contains(&cid) {
                    effects.push(Effect::Send {
                        to: cid,
                        bytes: e.bytes.clone(),
                    });
                }
                if e.from_cid == cid {
                    for bytes in e.acks.values() {
                        effects.push(Effect::Send {
                            to: cid,
                            bytes: bytes.clone(),
                        });
                    }
                }
            }
        }
        effects
    }

    /// The members of a project and each one's Device proofs, for a member who asks.
    pub(crate) fn pad_directory(
        &self,
        cid: Cid,
        project: &str,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        let members = self.pad_members(project).ok_or(PadRefusal::NoSuchProject)?;
        let asker = self.directory.endpoint(cid).map(|(p, _)| p.clone());
        if !asker.is_some_and(|p| members.contains(&p)) {
            return Err(PadRefusal::NotAMember.into());
        }
        let limit = sw_protocol::payload::pad::pad_limits::PROOFS_PER_MEMBER;
        let list = members
            .iter()
            .filter_map(|id| self.directory.person(id))
            .map(|record| PadMember {
                id_fp: record.id_fp.clone(),
                display: record.display.clone(),
                proofs: record
                    .endpoints
                    .iter()
                    .filter(|e| e.kind == EndpointKind::Device)
                    .filter_map(|e| self.pad.proofs.get(&e.cid))
                    .take(limit)
                    .map(|b| Bytes(b.clone()))
                    .collect(),
            })
            .collect();
        let answer = PadDirectory {
            project: project.to_owned(),
            members: Some(list),
            retention_ms: Some(self.pad_retention(project)?),
        };
        let bytes = SwMessage::Unsigned(Payload::PadDirectory(answer)).to_bytes()?;
        Ok(vec![Effect::Send { to: cid, bytes }])
    }
}
