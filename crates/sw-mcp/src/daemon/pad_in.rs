//! What the node sends the scratchpad: the member directory (which completes a post), notes
//! and acks (verified against their own proofs before anything is recorded or woken), and
//! receipts. Anything that fails a check is logged and dropped: nobody is woken by it.

use data_encoding::{BASE64URL_NOPAD, HEXLOWER};
use sw_protocol::gate::verify_pad;
use sw_protocol::pad::open_text;
use sw_protocol::payload::{PadReceipt, PadScope, Payload};
use sw_protocol::signed::SwMessage;

use super::{Daemon, now_ms};
use crate::adapters::CitadelIdentity;
use crate::pad::state::{AckRec, Received, Sender};

pub(crate) fn signal_hex(bytes: &[u8]) -> Result<String, String> {
    match SwMessage::from_bytes(bytes).map_err(|e| format!("{e:?}"))? {
        SwMessage::Signed { signed, .. } => {
            Ok(HEXLOWER.encode(&signed.signal_id().map_err(|e| format!("{e:?}"))?))
        }
        SwMessage::Unsigned(_) => Err("not a signed message".into()),
    }
}

impl Daemon {
    pub(super) fn on_pad_receipt(&mut self, r: PadReceipt) {
        let id = HEXLOWER.encode(&r.id);
        let reason = r.reason.unwrap_or_default();
        if r.stored {
            log::info!("scratchpad: the node stored {id}");
        } else {
            log::warn!("scratchpad: the node refused {id}: {reason}");
        }
        if self.pad.receipt(&id, r.stored, reason, now_ms()).is_some() || r.stored {
            self.save_pad();
            self.wake_pad();
        }
    }

    /// A relayed 18 or 19: verified end to end, then recorded and woken, or dropped.
    pub(super) fn on_pad_signed(&mut self, bytes: &[u8]) {
        match self.pad_in(bytes) {
            Ok(cursor) => {
                log::info!("scratchpad: event {cursor}");
                self.save_pad();
                self.wake_pad();
            }
            Err(why) => log::warn!("scratchpad: refused a relayed message: {why}"),
        }
    }

    fn pad_in(&mut self, bytes: &[u8]) -> Result<u64, String> {
        let (Some(signed_in), Some(own_cid)) = (self.signed_in.clone(), self.cid) else {
            return Err("not signed in".into());
        };
        let now = now_ms();
        let message = SwMessage::from_bytes(bytes).map_err(|e| format!("{e:?}"))?;
        let (v, device) = verify_pad(&message, &signed_in.issuer_pk, &CitadelIdentity, now)
            .map_err(|e| format!("{e:?}"))?;
        let id = HEXLOWER.encode(&v.signal_id);
        let me = signed_in.id_fp;
        match v.body.payload {
            Payload::PadNote(n) => {
                if n.scope == PadScope::Members && !n.to.contains(&me) {
                    return Err("a note for other members".into());
                }
                let text =
                    open_text(&n.body, own_cid, &self.keys.kem).map_err(|e| format!("{e:?}"))?;
                let from = Sender {
                    id_fp: device.id_fp.as_str().to_owned(),
                    device_cid: device.cid,
                    seal_pk: BASE64URL_NOPAD.encode(&device.seal_pk),
                };
                let r = Received {
                    project: n.project,
                    from,
                    body: text.body,
                    refs: text.refs,
                    at: v.body.iat,
                    exp: v.body.exp,
                    received_at: now,
                    acked: false,
                };
                self.pad
                    .note_in(&id, r, self.pad_limits())
                    .map_err(|e| format!("{e:?}"))
            }
            Payload::PadAck(a) => {
                let note_id = HEXLOWER.encode(&a.note_id);
                let exp = self
                    .pad
                    .sent
                    .get(&note_id)
                    .map(|s| s.exp)
                    .ok_or("an ack of a note this Device did not send")?;
                let reply = match &a.reply {
                    Some(sealed) => Some(
                        open_text(sealed, own_cid, &self.keys.kem)
                            .map_err(|e| format!("{e:?}"))?
                            .body,
                    ),
                    None => None,
                };
                let ack = AckRec {
                    by: v.body.iss.as_str().to_owned(),
                    at: v.body.iat,
                    reply,
                };
                self.pad
                    .ack_in(&id, &note_id, ack, exp)
                    .map_err(|e| format!("{e:?}"))
            }
            _ => Err("not a scratchpad message".into()),
        }
    }
}
