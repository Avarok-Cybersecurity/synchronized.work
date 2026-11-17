//! Every message this Device sends, built and signed here (pure: keys, time and nonces in,
//! encoded `SwMessage` bytes out). `seq` is `max(last + 1, now_ms)`: strictly increasing per
//! kid across daemon restarts without a persisted counter, as the wire requires.

use sw_crypto::{Entropy, SigningKey};
use sw_model::readiness::Blocker;
use sw_model::{Cid, EndpointKind, IdFp, Lane};
use sw_protocol::WireError;
use sw_protocol::body::{Audience, Body};
use sw_protocol::payload::{
    Ack, AckResult, Enrol, EnrolOp, Hello, HelloKind, Payload, ProgressEvent, ProgressItem,
    PubKeys, StatusUpdate, Subscribe, SubscribeOp, limits,
};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Bytes;

/// `HELLO.protocol_version` (sw-wire contract §3).
pub const PROTOCOL_VERSION: u16 = 1;
/// `HELLO.client_build`.
pub const CLIENT_BUILD: &str = concat!("sw-mcp/", env!("CARGO_PKG_VERSION"));

#[derive(Debug)]
pub struct Outbound {
    iss: IdFp,
    cid: Cid,
    last_seq: u64,
}

impl Outbound {
    pub fn new(iss: IdFp, cid: Cid) -> Self {
        Self {
            iss,
            cid,
            last_seq: 0,
        }
    }

    fn sign(
        &mut self,
        key: &SigningKey,
        aud: Audience,
        payload: Payload,
        now_ms: u64,
        entropy: &mut dyn Entropy,
    ) -> Result<Vec<u8>, WireError> {
        let seq = self.last_seq.saturating_add(1).max(now_ms);
        self.last_seq = seq;
        let mut nonce = [0u8; 16];
        entropy.fill(&mut nonce);
        let lifetime = payload.max_lifetime_ms();
        let body = Body {
            iss: self.iss.clone(),
            from: self.cid,
            aud,
            iat: now_ms,
            exp: now_ms.saturating_add(lifetime),
            nonce,
            seq,
            payload,
        };
        SwMessage::signed(&body, key)?.to_bytes()
    }

    pub fn hello() -> Result<Vec<u8>, WireError> {
        SwMessage::Unsigned(Payload::Hello(Hello {
            protocol_version: PROTOCOL_VERSION,
            endpoint_kind: HelloKind::Device,
            client_build: CLIENT_BUILD.to_owned(),
        }))
        .to_bytes()
    }

    pub fn subscribe(project_id: &str, since_cursor: u64) -> Result<Vec<u8>, WireError> {
        SwMessage::Unsigned(Payload::Subscribe(Subscribe {
            op: SubscribeOp::Subscribe,
            project_id: project_id.to_owned(),
            since_cursor,
        }))
        .to_bytes()
    }

    /// `ENROL` by assertion (contract §6): signed by this Device's own key, from its own CID,
    /// carrying both public keys and the sign-in assertion that names the signing key.
    pub fn enrol(
        &mut self,
        key: &SigningKey,
        seal_ek: &[u8],
        assertion: &[u8],
        now_ms: u64,
        entropy: &mut dyn Entropy,
    ) -> Result<Vec<u8>, WireError> {
        let payload = Payload::Enrol(Enrol {
            op: EnrolOp::Enrol,
            epoch: 1,
            endpoint_cid: Some(self.cid),
            kind: Some(EndpointKind::Device),
            pubkeys: Some(PubKeys {
                sign: Bytes(key.public_key().to_vec()),
                seal: Some(Bytes(seal_ek.to_vec())),
            }),
            kid: None,
            assertion: Some(Bytes(assertion.to_vec())),
        });
        self.sign(key, Audience::Node, payload, now_ms, entropy)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn status(
        &mut self,
        key: &SigningKey,
        agent_id: &str,
        task_id: &str,
        lane: Lane,
        blockers: Vec<Blocker>,
        harness: &str,
        now_ms: u64,
        entropy: &mut dyn Entropy,
    ) -> Result<Vec<u8>, WireError> {
        if blockers.len() > limits::BLOCKERS_PER_STATUS {
            return Err(WireError::Length("blockers"));
        }
        let payload = Payload::StatusUpdate(StatusUpdate {
            agent_id: agent_id.to_owned(),
            task_id: task_id.to_owned(),
            lane,
            blockers,
            harness: harness.to_owned(),
        });
        self.sign(key, Audience::Node, payload, now_ms, entropy)
    }

    pub fn progress(
        &mut self,
        key: &SigningKey,
        agent_id: &str,
        events: Vec<ProgressItem>,
        now_ms: u64,
        entropy: &mut dyn Entropy,
    ) -> Result<Vec<u8>, WireError> {
        let payload = Payload::ProgressEvent(ProgressEvent {
            agent_id: agent_id.to_owned(),
            events,
        });
        self.sign(key, Audience::Node, payload, now_ms, entropy)
    }

    /// `ACK` to the Controller that sent `signal_id`; `reason` iff `refused`.
    #[allow(clippy::too_many_arguments)]
    pub fn ack(
        &mut self,
        key: &SigningKey,
        controller_cid: Cid,
        signal_id: [u8; 32],
        result: AckResult,
        reason: Option<String>,
        now_ms: u64,
        entropy: &mut dyn Entropy,
    ) -> Result<Vec<u8>, WireError> {
        let payload = Payload::Ack(Ack {
            signal_id,
            result,
            reason,
        });
        self.sign(
            key,
            Audience::Endpoint(controller_cid),
            payload,
            now_ms,
            entropy,
        )
    }
}
