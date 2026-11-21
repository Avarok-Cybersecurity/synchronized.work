//! Completing a post: the node's directory answers, every member's Device proofs are verified
//! against the pinned issuer, and the note is sealed to the targets' verified Devices only.

use std::collections::BTreeMap;

use serde_json::json;
use sw_protocol::body::Audience;
use sw_protocol::gate::{BoundDevice, verify_proof};
use sw_protocol::pad::{Recipient, seal_text};
use sw_protocol::payload::{PadDirectory, PadNote, PadScope, Payload};
use sw_protocol::wire::Bytes;

use super::pad::{AskKind, PadAsk};
use super::pad_in::signal_hex;
use super::{Daemon, now_ms};
use crate::adapters::CitadelIdentity;
use crate::home::read_private;
use crate::ipc::Reply;
use crate::pad::state::Sent;

/// Each member's Devices whose proofs verify against the pinned issuer and name that member.
fn verified_devices(
    d: &PadDirectory,
    issuer: &[u8],
) -> BTreeMap<String, (String, Vec<BoundDevice>)> {
    let mut out = BTreeMap::new();
    for m in d.members.iter().flatten() {
        let devices = m
            .proofs
            .iter()
            .filter_map(|p| match verify_proof(&p.0, issuer, &CitadelIdentity) {
                Ok(b) if b.id_fp == m.id_fp => Some(b),
                other => {
                    log::warn!(
                        "scratchpad: dropped a Device proof of {}: {other:?}",
                        m.id_fp.as_str()
                    );
                    None
                }
            })
            .collect();
        out.insert(m.id_fp.as_str().to_owned(), (m.display.clone(), devices));
    }
    out
}

impl Daemon {
    pub(super) fn on_pad_directory(&mut self, d: PadDirectory) {
        let Some(issuer) = self.signed_in.as_ref().map(|s| s.issuer_pk.clone()) else {
            return;
        };
        let members = verified_devices(&d, &issuer);
        let (mine, rest): (Vec<PadAsk>, Vec<PadAsk>) = std::mem::take(&mut self.pad_asks)
            .into_iter()
            .partition(|a| a.project == d.project);
        self.pad_asks = rest;
        for ask in mine {
            let answer = match ask.kind {
                AskKind::Members => Ok(
                    json!({ "project": d.project, "retention_ms": d.retention_ms, "members": members.iter().map(|(id, (display, devs))| json!({
                    "id_fp": id, "display_as_the_node_reports": display, "verified_devices": devs.len(),
                    "fingerprint": crate::trust::fingerprint_words(&sw_crypto::sha3_256(id.as_bytes())),
                })).collect::<Vec<_>>() }),
                ),
                AskKind::Post { to, text } => self.post(&d, &members, to, text),
            };
            let _ = ask.reply.send(match answer {
                Ok(v) => Reply::Ok(v),
                Err(e) => Reply::Error(e),
            });
        }
    }

    fn post(
        &mut self,
        d: &PadDirectory,
        members: &BTreeMap<String, (String, Vec<BoundDevice>)>,
        to: Option<Vec<String>>,
        text: sw_protocol::payload::PadText,
    ) -> Result<serde_json::Value, String> {
        let me = self
            .signed_in
            .as_ref()
            .map(|s| s.id_fp.as_str().to_owned())
            .ok_or("not signed in")?;
        let retention = d
            .retention_ms
            .ok_or("the project owner has not set the scratchpad policy")?;
        let targets: Vec<String> = match &to {
            None => members.keys().filter(|id| **id != me).cloned().collect(),
            Some(names) => names
                .iter()
                .map(|n| {
                    members
                        .iter()
                        .find(|(id, (display, _))| *id == n || display == n)
                        .map(|(id, _)| id.clone())
                        .ok_or_else(|| format!("{n:?} is not a member of {}", d.project))
                })
                .collect::<Result<_, _>>()?,
        };
        if targets.is_empty() || targets.contains(&me) {
            return Err("a note needs at least one other member".into());
        }
        let mut recipients = Vec::new();
        for t in &targets {
            let devices = &members[t].1;
            if devices.is_empty() {
                return Err(format!("{t} has no Device with a verified proof yet"));
            }
            recipients.extend(devices.iter().map(|b| Recipient {
                cid: b.cid,
                seal_pk: &b.seal_pk,
            }));
        }
        let body =
            seal_text(&text, &recipients, &mut self.entropy).map_err(|e| format!("{e:?}"))?;
        let proof = read_private(&self.home.enrol_proof())
            .map_err(|e| e.to_string())?
            .ok_or("this Device has not enrolled yet")?;
        let (scope, listed) = match to {
            None => (PadScope::All, Vec::new()),
            Some(_) => (
                PadScope::Members,
                targets
                    .iter()
                    .map(|t| sw_model::IdFp::parse(t))
                    .collect::<Result<_, _>>()
                    .map_err(|e| format!("{e:?}"))?,
            ),
        };
        let note = PadNote {
            project: d.project.clone(),
            scope,
            to: listed,
            body,
            proof: Bytes(proof),
        };
        let now = now_ms();
        let outbound = self
            .outbound
            .as_mut()
            .ok_or("not connected to the node yet")?;
        let bytes = outbound
            .sign_until(
                &self.keys.sign,
                Audience::Node,
                Payload::PadNote(note),
                now,
                now + retention,
                &mut self.entropy,
            )
            .map_err(|e| format!("{e:?}"))?;
        let id = signal_hex(&bytes)?;
        let sent = Sent {
            project: d.project.clone(),
            to: targets.clone(),
            body: text.body,
            refs: text.refs,
            at: now,
            exp: now + retention,
            acks: Vec::new(),
            stored: None,
        };
        self.pad.sent.insert(id.clone(), sent);
        self.save_pad();
        self.send(bytes);
        let sealed_to: BTreeMap<&String, usize> =
            targets.iter().map(|t| (t, members[t].1.len())).collect();
        Ok(json!({ "note_id": id, "sealed_to_devices": sealed_to, "expires_at": now + retention }))
    }
}
