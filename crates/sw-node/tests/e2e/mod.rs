//! A Citadel client for the end-to-end test: register, sign in, and speak `SwMessage`s over
//! the C2S channel.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

mod raw;

use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use citadel_sdk::prelude::*;
use data_encoding::BASE64URL_NOPAD;
use futures::StreamExt;
use futures::lock::Mutex;
use sw_crypto::{SigningKey, kid_of};
use sw_model::tier::{JoinPolicy, TierName};
use sw_model::{EndpointKind, Visibility};
use sw_protocol::body::{Audience, Body};
use sw_protocol::payload::*;
use sw_protocol::ports::{MessageSigner, SignInClaims};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Bytes;
use sw_testkit::StandInIdentity;
use sw_testkit::assertion::sign_assertion;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub struct Account {
    pub identity: StandInIdentity,
    pub key: SigningKey,
    pub username: String,
}

impl Account {
    pub fn alice() -> Self {
        Self::of(0x01, "alice")
    }
    pub fn bob() -> Self {
        Self::of(0x02, "bob")
    }
    fn of(seed: u8, slug: &str) -> Self {
        let identity = StandInIdentity::from_seed(&[seed; 32]);
        let username = format!("{}.{slug}@synchronized.work", identity.id_fp().as_str());
        Self {
            identity,
            key: SigningKey::from_seed(&[seed + 0x10; 32]),
            username,
        }
    }
}

const PASSWORD: &str = "a generated endpoint password";
/// The test server is plain TCP on loopback: there is no certificate to verify.
const LOOPBACK_CERTS: CertVerification = CertVerification::SelfSignedAccepted;

pub struct Client {
    pub account: Account,
    pub cid: u64,
    tx: PeerChannelSendHalf<StackedRatchet>,
    rx: PeerChannelRecvHalf<StackedRatchet>,
    seq: u64,
}

impl Client {
    /// Registers and signs in, then says `HELLO`.
    pub async fn sign_in(
        remote: &NodeRemote<StackedRatchet>,
        addr: SocketAddr,
        account: Account,
    ) -> Result<Self, NetworkError> {
        let mut client = Self::open(remote, addr, account).await?;
        let hello = SwMessage::Unsigned(Payload::Hello(Hello {
            protocol_version: 1,
            endpoint_kind: HelloKind::Controller,
            client_build: "e2e".into(),
        }));
        client.tx.send(hello.to_bytes().unwrap()).await?;
        Ok(client)
    }

    /// ENROL by assertion: the test plays work.avarok.net with the provisioned issuer key.
    pub async fn enrol(&mut self) -> Result<(), NetworkError> {
        let issuer = SigningKey::from_seed(&[0x51; 32]);
        let device = format!(
            "e2e#{}",
            BASE64URL_NOPAD.encode(&kid_of(self.account.key.public_key()))
        );
        let claims = SignInClaims {
            identity: self.account.identity.public_key().to_vec(),
            device,
            aud: "sw-mcp".into(),
            exp_s: now_ms() / 1000 + 600,
        };
        let enrol = Payload::Enrol(Enrol {
            op: EnrolOp::Enrol,
            epoch: 1,
            endpoint_cid: Some(self.cid),
            kind: Some(EndpointKind::Controller),
            pubkeys: Some(PubKeys {
                sign: Bytes(self.account.key.public_key().to_vec()),
                seal: None,
            }),
            kid: None,
            assertion: Some(Bytes(sign_assertion(&issuer, &claims, &[9; 32]))),
        });
        self.send(Audience::Node, enrol).await
    }

    pub async fn send(&mut self, aud: Audience, payload: Payload) -> Result<(), NetworkError> {
        self.seq += 1;
        let now = now_ms();
        let body = Body {
            iss: self.account.identity.id_fp(),
            from: self.cid,
            aud,
            iat: now,
            exp: now + 60_000,
            nonce: [self.seq as u8; 16],
            seq: self.seq,
            payload,
        };
        let message = SwMessage::signed(&body, &self.account.key).unwrap();
        self.tx.send(message.to_bytes().unwrap()).await
    }

    pub async fn next_notify(&mut self) -> Result<Notify, NetworkError> {
        while let Some(bytes) = self.rx.next().await {
            if let Ok(SwMessage::Unsigned(Payload::Notify(n))) =
                SwMessage::from_bytes(bytes.as_ref())
            {
                return Ok(n);
            }
        }
        Err(NetworkError::msg("the session closed"))
    }
}

pub fn create(id: &str) -> Payload {
    Payload::TaskWrite(TaskWrite {
        op: TaskOp::Create,
        task_id: id.into(),
        fields: TaskFields {
            title: Some("Ship the node".into()),
            visibility: Some(Visibility::Members),
            ..TaskFields::EMPTY
        },
    })
}

pub fn open_up(id: &str) -> Payload {
    Payload::TaskWrite(TaskWrite {
        op: TaskOp::Update,
        task_id: id.into(),
        fields: TaskFields {
            tier: Some(TierName::Aware),
            join_policy: Some(JoinPolicy::Manual),
            ..TaskFields::EMPTY
        },
    })
}

pub fn join(id: &str, invite: Option<[u8; 32]>) -> Payload {
    Payload::JoinRequest(JoinRequest {
        task_id: id.into(),
        note: "let me help".into(),
        readiness: sw_model::readiness::Readiness::from_permille(1000).unwrap(),
        agent_id: None,
        invite,
    })
}

/// The owner's invite for `token`, live for an hour of wall-clock time.
pub fn invite(project: &str, token: &[u8; 32]) -> Payload {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    Payload::Invite(Invite {
        project: project.into(),
        token_hash: sw_protocol::invite::token_hash(token),
        exp: now + 3_600_000,
    })
}

/// Runs `f` once the client node starts.
pub struct ClientKernel<F> {
    f: Mutex<Option<F>>,
    remote: Option<NodeRemote<StackedRatchet>>,
}

impl<F> ClientKernel<F> {
    pub fn new(f: F) -> Self {
        Self {
            f: Mutex::new(Some(f)),
            remote: None,
        }
    }
}

#[async_trait]
impl<F, Fut> NetKernel<StackedRatchet> for ClientKernel<F>
where
    F: FnOnce(NodeRemote<StackedRatchet>) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<(), NetworkError>> + Send,
{
    fn load_remote(&mut self, remote: NodeRemote<StackedRatchet>) -> Result<(), NetworkError> {
        self.remote = Some(remote);
        Ok(())
    }
    async fn on_start(&self) -> Result<(), NetworkError> {
        let f = self.f.lock().await.take().expect("started once");
        f(self.remote.clone().expect("remote loaded")).await
    }
    async fn on_node_event_received(
        &self,
        _: NodeResult<StackedRatchet>,
    ) -> Result<(), NetworkError> {
        Ok(())
    }
    async fn on_stop(&mut self) -> Result<(), NetworkError> {
        Ok(())
    }
}
