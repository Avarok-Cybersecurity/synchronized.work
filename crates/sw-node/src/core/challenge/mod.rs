//! The post-connect server identity challenge (`server-identity-challenge.md` §2, identity-api
//! §6): a Citadel agent that signed in to an identity account sends a fresh nonce over the
//! session, and the node answers with its canonical domain, its server key and
//! `ServerKey::sign_challenge(domain, cid, nonce)`, where `cid` is the session's own CID as the
//! node knows it. Any session may ask, before or after `HELLO`; nothing is stored or changed,
//! and a session is answered at most once per the provisioned interval.

pub mod frame;

use std::collections::BTreeMap;
use std::sync::Arc;

use citadel_identity::{CanonicalDomain, ServerKey};
use sw_model::Cid;

use super::{Effect, NodeCore, NodeRefusal};

/// What the node answers a challenge with: its canonical domain and its server key, both from
/// provisioning.
#[derive(Debug, Clone)]
pub struct ServerIdentity {
    domain: CanonicalDomain,
    key: Arc<ServerKey>,
}

impl ServerIdentity {
    pub fn new(domain: CanonicalDomain, key: ServerKey) -> Self {
        Self {
            domain,
            key: Arc::new(key),
        }
    }
}

/// When each live session was last answered, in the clock's milliseconds.
#[derive(Debug, Default)]
pub(super) struct Answered(BTreeMap<Cid, u64>);

impl Answered {
    pub(super) fn forget(&mut self, cid: Cid) {
        self.0.remove(&cid);
    }
}

impl NodeCore {
    /// `Some` when `bytes` is a challenge (its answer, or why it is refused); `None` for every
    /// other frame, which the node's own wire then reads.
    pub(super) fn identity_challenge(
        &mut self,
        cid: Cid,
        bytes: &[u8],
        now_ms: u64,
    ) -> Option<Result<Vec<Effect>, NodeRefusal>> {
        let nonce = frame::challenge_nonce(bytes)?;
        Some(self.answer_challenge(cid, &nonce, now_ms))
    }

    fn answer_challenge(
        &mut self,
        cid: Cid,
        nonce: &[u8; frame::NONCE_LEN],
        now_ms: u64,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        let identity = self
            .rules
            .server_identity
            .as_ref()
            .ok_or(NodeRefusal::NoServerIdentity)?;
        let interval = self.rules.limits.identity_challenge_interval_ms;
        if let Some(at) = self.answered.0.get(&cid)
            && now_ms.saturating_sub(*at) < interval
        {
            return Err(NodeRefusal::ChallengeTooSoon);
        }
        let domain = &identity.domain;
        let sig = identity.key.sign_challenge(domain, cid, nonce);
        let pk = identity.key.public_key().as_bytes();
        let bytes =
            frame::answer_frame(domain.as_str(), pk, &sig).map_err(|_| NodeRefusal::Malformed)?;
        self.answered.0.insert(cid, now_ms);
        Ok(vec![Effect::Send { to: cid, bytes }])
    }
}
