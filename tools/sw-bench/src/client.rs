//! One Endpoint account on the node.

use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use citadel_identity::{Identity, ServerKey, SignInAssertion};
use citadel_io::WebSocketEndpoint;
use citadel_sdk::prelude::*;
use data_encoding::BASE64URL_NOPAD;
use futures::StreamExt;
use sw_crypto::{SigningKey, kid_of};
use sw_model::{EndpointKind, IdFp};
use sw_protocol::body::{Audience, Body};
use sw_protocol::payload::*;
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::Bytes;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Where the node listens: a TCP address (in-process tests) or a `ws(s)://` URL (a Worker).
#[derive(Debug, Clone)]
pub enum Dial {
    Tcp(SocketAddr),
    Ws(WebSocketEndpoint),
}

#[derive(Debug)]
pub struct Account {
    pub identity: Identity,
    pub key: SigningKey,
    pub slug: String,
}

impl Account {
    /// Deterministic from `n`: the identity seed, the Endpoint key and the account name.
    pub fn numbered(n: u32) -> Self {
        let mut seed = [0u8; 32];
        seed[..4].copy_from_slice(&n.to_be_bytes());
        let mut key_seed = seed;
        key_seed[31] = 0xee;
        Self {
            identity: Identity::from_seed(&seed),
            key: SigningKey::from_seed(&key_seed),
            slug: format!("bench{n}"),
        }
    }

    pub fn id_fp(&self) -> IdFp {
        IdFp::parse(&self.identity.id_fp()).unwrap_or_else(|_| unreachable!("the crate's id_fp"))
    }

    /// `<id_fp>.<slug>@synchronized.work` (design §5.2).
    pub fn username(&self) -> String {
        format!("{}.{}@synchronized.work", self.identity.id_fp(), self.slug)
    }
}

/// The Endpoint password the MCP generates and keeps (sign-in-with-citadel §6).
const PASSWORD: &str = "a generated endpoint password";

pub struct Client {
    pub account: Account,
    pub cid: u64,
    tx: PeerChannelSendHalf<StackedRatchet>,
    rx: PeerChannelRecvHalf<StackedRatchet>,
    seq: u64,
}

pub async fn register(
    remote: &NodeRemote<StackedRatchet>,
    dial: &Dial,
    a: &Account,
) -> Result<(), NetworkError> {
    let user = a.username();
    match dial {
        Dial::Tcp(addr) => remote
            .register_with_defaults(*addr, user.as_str(), user.as_str(), PASSWORD)
            .await
            .map(|_| ()),
        Dial::Ws(url) => remote
            .register_to_endpoint(
                url.clone(),
                user.as_str(),
                user.as_str(),
                PASSWORD,
                Default::default(),
                None,
            )
            .await
            .map(|_| ()),
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("cid", &self.cid)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Signs in through the node's post-quantum sign-in and sends `HELLO`.
    pub async fn sign_in(
        remote: &NodeRemote<StackedRatchet>,
        account: Account,
    ) -> Result<Self, NetworkError> {
        let request = AuthenticationRequest::credentialed(account.username(), PASSWORD);
        let mut conn = remote.connect_with_defaults(request).await?;
        let channel = conn
            .take_channel()
            .ok_or_else(|| NetworkError::msg("no C2S channel"))?;
        let (tx, rx) = channel.split();
        let mut client = Self {
            account,
            cid: conn.cid,
            tx,
            rx,
            seq: 0,
        };
        let hello = SwMessage::Unsigned(Payload::Hello(Hello {
            protocol_version: 1,
            endpoint_kind: HelloKind::Controller,
            client_build: "sw-bench".into(),
        }));
        client.send_raw(&hello).await?;
        Ok(client)
    }

    /// ENROL by assertion, minted by `issuer` as work.avarok.net would.
    pub async fn enrol(&mut self, issuer: &ServerKey) -> Result<(), NetworkError> {
        let device = format!(
            "{}#{}",
            self.account.slug,
            BASE64URL_NOPAD.encode(&kid_of(self.account.key.public_key()))
        );
        let assertion = issuer
            .sign_assertion(&SignInAssertion {
                identity: self.account.identity.public_key().clone(),
                device,
                aud: "sw-mcp".into(),
                exp_s: now_ms() / 1000 + 600,
                nonce: [9; 32],
            })
            .map_err(|e| NetworkError::msg(format!("{e:?}")))?;
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
            assertion: Some(Bytes(assertion)),
        });
        self.send(Audience::Node, enrol).await
    }

    pub async fn send(&mut self, aud: Audience, payload: Payload) -> Result<(), NetworkError> {
        self.seq += 1;
        let now = now_ms();
        let body = Body {
            iss: self.account.id_fp(),
            from: self.cid,
            aud,
            iat: now,
            exp: now + 60_000,
            nonce: nonce(self.seq),
            seq: self.seq,
            payload,
        };
        let message = SwMessage::signed(&body, &self.account.key)
            .map_err(|e| NetworkError::msg(format!("{e:?}")))?;
        self.send_raw(&message).await
    }

    async fn send_raw(&mut self, message: &SwMessage) -> Result<(), NetworkError> {
        let bytes = message
            .to_bytes()
            .map_err(|e| NetworkError::msg(format!("{e:?}")))?;
        self.tx.send(bytes).await
    }

    /// The next `NOTIFY` the node pushes on this session.
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

fn nonce(seq: u64) -> [u8; 16] {
    let mut n = [0u8; 16];
    n[8..].copy_from_slice(&seq.to_be_bytes());
    n
}
