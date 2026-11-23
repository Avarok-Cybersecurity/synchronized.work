//! One inbound message: decode, gate, apply.

use sw_model::{Cid, EndpointKind};
use sw_protocol::body::Audience;
use sw_protocol::gate::{Verified, Verifier, accept_unsigned, verify};
use sw_protocol::payload::Payload;
use sw_protocol::signed::SwMessage;

use super::feed::Author;
use super::inbox::{self, Queued};
use super::{Effect, NodeCore, NodeRefusal, enrol};
use crate::identity::CitadelIdentity;

pub fn seq_key(kid: &[u8; 32]) -> String {
    let hex: String = kid.iter().map(|b| format!("{b:02x}")).collect();
    format!("seq/{hex}")
}

pub fn person_key(id: &sw_model::IdFp) -> String {
    format!("person/{}", id.as_str())
}

pub fn task_key(id: &str) -> String {
    format!("task/{id}")
}

impl NodeCore {
    /// Applies one `SwMessage` from session `cid`, or refuses it with nothing changed.
    pub fn received(
        &mut self,
        cid: Cid,
        bytes: &[u8],
        now_ms: u64,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        let message = SwMessage::from_bytes(bytes)?;
        if let SwMessage::Unsigned(_) = &message {
            let payload = accept_unsigned(message)?;
            return self.unsigned(cid, payload, bytes, now_ms);
        }
        if !self.greeted.contains(&cid) {
            return Err(NodeRefusal::NoHello);
        }
        self.replay.prune(now_ms);
        let verifier = Verifier::Node {
            session_cid: cid,
            sign_in_issuer: &self.rules.sign_in_issuer,
        };
        let mut scratch = self.replay.clone();
        let verified = verify(
            &message,
            &self.directory,
            &CitadelIdentity,
            &mut scratch,
            verifier,
            now_ms,
        )?;
        let receipted = self.receipt_project(&verified.body.payload);
        let mut effects = match self.signed(cid, &verified, bytes, now_ms) {
            Ok(effects) => effects,
            Err(NodeRefusal::Pad(r)) => return self.pad_refused(&verified, r),
            Err(NodeRefusal::Link(r)) => return self.link_refused(cid, &verified, r),
            Err(e) => return Err(e),
        };
        self.replay = scratch;
        if let Some(project) = receipted {
            let typ = verified.body.payload.typ();
            effects.push(Self::stored_receipt(cid, project, verified.signal_id, typ)?);
        }
        effects.push(Effect::Put {
            key: seq_key(&verified.kid),
            value: verified.body.seq.to_be_bytes().to_vec(),
        });
        Ok(effects)
    }

    fn unsigned(
        &mut self,
        cid: Cid,
        payload: Payload,
        bytes: &[u8],
        now_ms: u64,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        match payload {
            Payload::Hello(_) => {
                self.greeted.insert(cid);
                Ok(Vec::new())
            }
            Payload::Subscribe(s) => self.subscription(cid, &s),
            Payload::Link(l) => self.link_redeem(cid, &l, bytes, now_ms),
            Payload::PadDirectory(d) if d.members.is_none() => self.pad_directory(cid, &d.project),
            Payload::JoinPending(j) if j.requests.is_none() => self.join_pending(cid, j.project),
            p @ (Payload::Presence(_)
            | Payload::JoinPending(_)
            | Payload::Event(_)
            | Payload::Notify(_)
            | Payload::PadDirectory(_)
            | Payload::PadReceipt(_)) => Err(NodeRefusal::NotForTheNode(p.typ())),
            p => Err(NodeRefusal::Unsupported(p.typ())),
        }
    }

    /// The rules for a message the gate accepted. Nothing is stored unless this returns
    /// `Ok`, and the replay state is committed only then (a refused write burns no `seq`).
    fn signed(
        &mut self,
        cid: Cid,
        v: &Verified,
        bytes: &[u8],
        now_ms: u64,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        let iss = &v.body.iss;
        let limits = self.rules.limits;
        let by = Author {
            iss: iss.clone(),
            from: v.body.from,
        };
        let change = match &v.body.payload {
            Payload::Enrol(e) => {
                let policy = self.rules.new_person_policy;
                let record = enrol::apply(&self.directory, v, e, policy, limits.max_persons)?;
                let value = sw_protocol::record::record_to_bytes(&record)?;
                let key = person_key(&record.id_fp);
                self.directory.put(record);
                let mut effects = vec![Effect::Put { key, value }];
                if v.sign_in.is_some() && e.kind == Some(EndpointKind::Device) {
                    self.pad.proofs.insert(v.body.from, bytes.to_vec());
                    effects.push(Effect::Put {
                        key: super::pad::ledger::proof_key(v.body.from),
                        value: bytes.to_vec(),
                    });
                }
                return Ok(effects);
            }
            Payload::TaskWrite(w) => {
                let dir = &self.directory;
                self.tree.write(iss, w, now_ms, limits.max_tasks, &|p| {
                    dir.person(p).is_some()
                })?
            }
            Payload::JoinRequest(r) => return self.join(&by, r, v.signal_id, now_ms),
            Payload::Invite(i) => return self.invite(v, i, now_ms),
            Payload::JoinAnswer(a) => self.tree.join_answer(iss, a, now_ms)?,
            Payload::StatusUpdate(s) => {
                let change = self.tree.status(iss, cid, s, now_ms)?;
                let mut effects = self.apply_change(change, &by, now_ms)?;
                effects.extend(self.feed_status(by, s)?);
                return Ok(effects);
            }
            Payload::Link(l) => return self.link_mint(cid, v, l, now_ms),
            Payload::Prompt(_) | Payload::Control(_) => {
                return self.relay(v, bytes, EndpointKind::Device, true, now_ms);
            }
            Payload::Ack(ack) => {
                let mut effects = self.relay(v, bytes, EndpointKind::Controller, false, now_ms)?;
                if self.inbox.acknowledged(cid, &ack.signal_id) {
                    effects.push(Effect::Delete {
                        key: inbox::key(cid, &ack.signal_id),
                    });
                }
                return Ok(effects);
            }
            Payload::ProgressTail(_) => return Err(NodeRefusal::NotForTheNode(17)),
            Payload::PadNote(n) => return self.pad_note(v, n, bytes, now_ms),
            Payload::PadAck(a) => return self.pad_ack(v, a, bytes, now_ms),
            Payload::PadPolicy(p) => return self.pad_policy(v, p),
            p => return Err(NodeRefusal::Unsupported(p.typ())),
        };
        self.apply_change(change, &by, now_ms)
    }

    /// Sends a signal to its audience's live session and, for a Device, queues it until the
    /// Device acknowledges it; an ACK for an offline Controller waits for its next session.
    fn relay(
        &mut self,
        v: &Verified,
        bytes: &[u8],
        kind: EndpointKind,
        until_ack: bool,
        now_ms: u64,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        let Audience::Endpoint(to) = v.body.aud else {
            return Err(NodeRefusal::NoSuchRecipient);
        };
        if self.directory.endpoint(to).map(|(_, k)| *k) != Some(kind) {
            return Err(NodeRefusal::NoSuchRecipient);
        }
        let online = self.online.contains(&to) && now_ms < v.body.exp;
        let mut effects = Vec::new();
        if online {
            effects.push(Effect::Send {
                to,
                bytes: bytes.to_vec(),
            });
        }
        if until_ack || !online {
            let q = Queued {
                signal_id: v.signal_id,
                exp: v.body.exp,
                until_ack,
                bytes: bytes.to_vec(),
            };
            let value = inbox::encode(&q);
            self.inbox.push(to, q, &self.rules.limits)?;
            effects.push(Effect::Put {
                key: inbox::key(to, &v.signal_id),
                value,
            });
        }
        Ok(effects)
    }

    /// A scratchpad refusal is answered to its writer; nothing was stored or burnt.
    fn pad_refused(&self, v: &Verified, r: super::PadRefusal) -> Result<Vec<Effect>, NodeRefusal> {
        let project = match &v.body.payload {
            Payload::PadNote(n) => &n.project,
            Payload::PadAck(a) => &a.project,
            _ => return Err(NodeRefusal::Pad(r)),
        };
        Ok(vec![Self::pad_receipt(v.body.from, project, v, Some(r))?])
    }
}
