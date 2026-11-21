//! What arrives from the node, and what the daemon does when the session comes and goes.

use sw_client::{SessionPort as _, SessionState};
use sw_crypto::{kid_of, verify_signature};
use sw_protocol::body::Body;
use sw_protocol::payload::{EnrolOp, Payload};
use sw_protocol::signed::{SwMessage, signed_message};

use super::{Daemon, now_ms};
use crate::outbound::Outbound;

impl Daemon {
    /// Sends now, or keeps the message until the session is up again.
    pub(crate) fn send(&mut self, bytes: Vec<u8>) {
        let Some(session) = &mut self.session else {
            self.unsent.push_back(bytes);
            return;
        };
        if let Err(e) = session.send(bytes.clone()) {
            log::debug!("queued until reconnected: {e:?}");
            self.unsent.push_back(bytes);
        }
    }

    pub(crate) fn on_state(&mut self) {
        let Some(session) = &self.session else { return };
        match session.state() {
            SessionState::Connected { cid } if !self.online => {
                self.online = true;
                self.on_connected(cid);
            }
            SessionState::Connected { .. } => {}
            SessionState::Disconnected { reason } => {
                self.online = false;
                log::warn!("session to the node dropped: {reason}");
            }
            SessionState::Connecting => {
                self.online = false;
                log::debug!("connecting to the node");
            }
        }
    }

    fn on_connected(&mut self, cid: u64) {
        let Some(signed_in) = self.signed_in.clone() else {
            return;
        };
        if self.cid.is_some_and(|known| known != cid) {
            log::error!(
                "the node gave CID {cid}, not this account's {:?}; refusing to continue",
                self.cid
            );
            std::process::exit(1);
        }
        log::info!("connected to the node as CID {cid}");
        self.cid = Some(cid);
        let outbound = self
            .outbound
            .get_or_insert_with(|| Outbound::new(signed_in.id_fp.clone(), cid));
        let mut first = Vec::new();
        match Outbound::hello() {
            Ok(b) => first.push(b),
            Err(e) => log::error!("HELLO: {e:?}"),
        }
        let mut enrol = None;
        let current = crate::home::read_private(&self.home.enrol_proof())
            .ok()
            .flatten()
            .is_some_and(|p| {
                crate::outbound::proof_is_current(
                    &p,
                    self.keys.sign.public_key(),
                    self.keys.kem.encapsulation_key(),
                )
            });
        if !current {
            match outbound.enrol(
                &self.keys.sign,
                self.keys.kem.encapsulation_key(),
                &signed_in.blob,
                now_ms(),
                &mut self.entropy,
            ) {
                Ok(b) => {
                    enrol = Some(b.clone());
                    first.push(b);
                }
                Err(e) => log::error!("ENROL: {e:?}"),
            }
        }
        for rec in self.agents.values() {
            let since = self.projection.cursor(&rec.project);
            match Outbound::subscribe(&rec.project, since) {
                Ok(b) => first.push(b),
                Err(e) => log::error!("SUBSCRIBE: {e:?}"),
            }
        }
        let backlog: Vec<Vec<u8>> = self.unsent.drain(..).collect();
        for bytes in first.into_iter().chain(backlog) {
            self.send(bytes);
        }
        if let Some(proof) = enrol {
            match crate::home::write_private(&self.home.enrol_proof(), &proof) {
                Ok(()) => log::info!("sent ENROL by assertion"),
                Err(e) => log::error!("the enrolment proof: {e}"),
            }
        }
        self.pad_connected();
    }

    pub(crate) fn on_message(&mut self, bytes: &[u8]) {
        // What the node sends the moment a session opens (queued notes) can be read before the
        // state change is: take the session's CID first, or those messages would be dropped.
        if !self.online {
            self.on_state();
        }
        let typ = match SwMessage::from_bytes(bytes) {
            Ok(SwMessage::Unsigned(Payload::Event(ev))) => {
                self.projection.apply_event(ev);
                return;
            }
            Ok(SwMessage::Unsigned(Payload::Notify(n))) => {
                log::info!("notify {}: {}", n.kind, crate::frame::sanitize(&n.subject));
                self.on_notify(&n);
                return;
            }
            Ok(SwMessage::Unsigned(Payload::JoinPending(j))) if j.requests.is_some() => {
                self.on_join_pending(j);
                return;
            }
            Ok(SwMessage::Unsigned(Payload::PadDirectory(d))) if d.members.is_some() => {
                self.on_pad_directory(d);
                return;
            }
            Ok(SwMessage::Unsigned(Payload::PadReceipt(r))) => {
                self.on_pad_receipt(r);
                return;
            }
            Ok(SwMessage::Unsigned(p)) => {
                log::debug!("ignored an unsigned typ {} from the node", p.typ());
                return;
            }
            Ok(SwMessage::Signed { typ: 11, signed }) => {
                self.on_revoke(&signed);
                return;
            }
            Ok(SwMessage::Signed { typ, .. }) => typ,
            Err(e) => {
                log::warn!("refused undecodable bytes from the node: {e:?}");
                return;
            }
        };
        if matches!(typ, 4 | 5) {
            self.on_signal(bytes);
        } else if matches!(typ, 18 | 19) {
            self.on_pad_signed(bytes);
        } else {
            log::debug!("ignored a signed typ {typ} from the node");
        }
    }

    /// A root-signed `REVOKE` (design §5.4): verified against the signed-in Person's identity
    /// key itself, never against anything the node supplies; the kid leaves the allowlist.
    fn on_revoke(&mut self, signed: &sw_protocol::signed::Signed) {
        let Some(signed_in) = &self.signed_in else {
            return;
        };
        let ok_key = kid_of(&signed_in.identity_pk) == signed.kid;
        let ok_sig = verify_signature(
            &signed_in.identity_pk,
            &signed_message(&signed.body.0),
            &signed.sig.0,
        )
        .is_ok();
        let body = Body::from_bytes(&signed.body.0);
        let (true, true, Ok(body)) = (ok_key, ok_sig, body) else {
            log::warn!("refused a typ 11 not signed by this Person's identity key");
            return;
        };
        let Payload::Enrol(e) = &body.payload else {
            return;
        };
        if body.iss != signed_in.id_fp || e.op != EnrolOp::Revoke || body.exp <= now_ms() {
            log::warn!("refused a REVOKE for another Person, of another op, or expired");
            return;
        }
        let Some(kid) = e.kid else { return };
        if self.trust.revoke(&kid) {
            log::info!(
                "revoked controller {} by the Person's REVOKE",
                crate::trust::kid_prefix(&kid)
            );
            if let Err(e) = crate::store::save_trust(&self.home, &self.keys, &self.trust) {
                log::error!("saving the allowlist after a revocation: {e}");
            }
        }
    }
}
