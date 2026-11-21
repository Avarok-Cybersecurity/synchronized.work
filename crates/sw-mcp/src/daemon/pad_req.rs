//! The scratchpad requests answered at once: inbox, ack, policy and trust.

use serde_json::{Value, json};
use sw_protocol::body::Audience;
use sw_protocol::pad::{Recipient, seal_text};
use sw_protocol::payload::{PadAck, PadPolicy, PadText, Payload};
use sw_protocol::wire::Bytes;

use super::pad::hex32;
use super::{Daemon, now_ms};
use crate::home::read_private;
use crate::pad::event::render;

impl Daemon {
    pub(super) fn pad_answer(&mut self, request: crate::ipc::Request) -> Result<Value, String> {
        use crate::ipc::Request as R;
        match request {
            R::PadInbox { project } => {
                let keep = |p: &str| project.as_deref().is_none_or(|want| want == p);
                let own = self.own_id();
                let mut pending = Vec::new();
                for rec in self.pad.events.clone() {
                    let live = self
                        .pad
                        .received
                        .get(&rec.note_id)
                        .is_some_and(|n| !n.acked && keep(&n.project));
                    if live && let Some(e) = render(&self.pad, &rec, &own) {
                        pending.push(e);
                    }
                }
                let sent: Vec<Value> = self.pad.sent.iter().filter(|(_, s)| keep(&s.project))
                    .map(|(id, s)| json!({"note_id": id, "project": s.project, "to": s.to, "stored": s.stored, "acks": s.acks, "body": s.body}))
                    .collect();
                Ok(json!({ "pending": pending, "sent": sent, "cursor": self.pad.next_cursor }))
            }
            R::PadAck { note_id, reply } => self.pad_ack(&note_id, reply),
            R::PadPolicy {
                project,
                retention_hours,
            } => {
                let payload = Payload::PadPolicy(PadPolicy {
                    project,
                    retention_ms: retention_hours * 3_600_000,
                });
                let outbound = self
                    .outbound
                    .as_mut()
                    .ok_or("not connected to the node yet")?;
                let bytes = outbound
                    .sign(
                        &self.keys.sign,
                        Audience::Node,
                        payload,
                        now_ms(),
                        &mut self.entropy,
                    )
                    .map_err(|e| format!("{e:?}"))?;
                self.send(bytes);
                Ok(json!({ "sent": true }))
            }
            R::PadTrust { id_fp, label } => {
                sw_model::IdFp::parse(&id_fp).map_err(|_| "not an id_fp")?;
                match label {
                    Some(l) => self.pad.trusted.insert(id_fp.clone(), l),
                    None => self.pad.trusted.remove(&id_fp),
                };
                self.save_pad();
                Ok(json!({ "trusted": self.pad.trusted }))
            }
            _ => Err("not a scratchpad request".into()),
        }
    }

    fn pad_ack(&mut self, note_id: &str, reply: Option<String>) -> Result<Value, String> {
        let id = hex32(note_id)?;
        let note = self
            .pad
            .received
            .get(note_id)
            .cloned()
            .ok_or("no such note here")?;
        if note.acked {
            return Err("already acknowledged".into());
        }
        let proof = read_private(&self.home.enrol_proof())
            .map_err(|e| e.to_string())?
            .ok_or("this Device has not enrolled yet")?;
        let sealed = match reply {
            Some(text) => {
                let seal_pk = data_encoding::BASE64URL_NOPAD
                    .decode(note.from.seal_pk.as_bytes())
                    .map_err(|e| e.to_string())?;
                let to = [Recipient {
                    cid: note.from.device_cid,
                    seal_pk: &seal_pk,
                }];
                let text = PadText {
                    body: text,
                    refs: Vec::new(),
                };
                Some(seal_text(&text, &to, &mut self.entropy).map_err(|e| format!("{e:?}"))?)
            }
            None => None,
        };
        let payload = Payload::PadAck(PadAck {
            project: note.project.clone(),
            note_id: id,
            reply: sealed,
            proof: Bytes(proof),
        });
        let outbound = self
            .outbound
            .as_mut()
            .ok_or("not connected to the node yet")?;
        let bytes = outbound
            .sign_until(
                &self.keys.sign,
                Audience::Node,
                payload,
                now_ms(),
                note.exp,
                &mut self.entropy,
            )
            .map_err(|e| format!("{e:?}"))?;
        let ack_id = super::pad_in::signal_hex(&bytes)?;
        if let Some(n) = self.pad.received.get_mut(note_id) {
            n.acked = true;
        }
        self.pad.acks_out.insert(ack_id.clone(), note_id.to_owned());
        self.save_pad();
        self.send(bytes);
        Ok(json!({ "acked": note_id, "ack_id": ack_id }))
    }
}
