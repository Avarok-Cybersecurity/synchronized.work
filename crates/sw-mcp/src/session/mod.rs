//! `SessionPort` over the native Citadel SDK (design §9; master plan lane S4): this Device is a
//! Citadel client of the synchronized.work node, reached by its `wss://` URL. The SDK does the
//! PQ key exchange, the ratchets, TLS and framing; this adapter turns the C2S channel into the
//! port's non-blocking `send` / `poll_recv`, and keeps the account signed in across drops.
//!
//! The account is `<id_fp>.<slug>@<node>` with the generated password from `keys/` (contract
//! §6); it is registered once, and its CID is permanent, so later sessions are logins.

mod kernel;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use citadel_sdk::prelude::{
    BackendType, CertVerification, DefaultNodeBuilder, LinkManagerMode, ServerMiscSettings,
    WebSocketEndpoint,
};
use citadel_sdk::types::crypto::MaxSecurityLevel;
use citadel_user::credentials::CredentialRequirements;
use sw_client::{SessionError, SessionPort, SessionState};
use tokio::sync::{Notify, mpsc};
use zeroize::Zeroizing;

use kernel::SessionKernel;

pub struct SessionSettings {
    pub endpoint: WebSocketEndpoint,
    pub username: String,
    pub password: Zeroizing<String>,
    /// Where the SDK keeps the account (a directory under `~/.synchronized/citadel/`).
    pub backend: BackendType,
    pub reconnect_delay: Duration,
    pub stun_servers: Vec<String>,
    /// What the session demands of the node's certificate ([`crate::config::Config::cert_verification`]).
    pub cert_verification: CertVerification,
}

impl std::fmt::Debug for SessionSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionSettings")
            .field("endpoint", &self.endpoint.to_string())
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

/// The state both halves share: what the port reports, and a wake-up for whoever waits on it.
#[derive(Debug)]
pub(crate) struct Shared {
    pub(crate) state: Mutex<SessionState>,
    pub(crate) changed: Notify,
}

impl Shared {
    pub(crate) fn set(&self, state: SessionState) {
        match self.state.lock() {
            Ok(mut s) => *s = state,
            Err(poisoned) => *poisoned.into_inner() = state,
        }
        self.changed.notify_one();
    }
}

#[derive(Debug, Clone)]
pub struct StateWatch(Arc<Shared>);

impl StateWatch {
    /// Waits until the session state changes (connected, dropped, reconnecting).
    pub async fn changed(&self) {
        self.0.changed.notified().await;
    }
}

#[derive(Debug)]
pub struct CitadelSession {
    outbound: mpsc::UnboundedSender<Vec<u8>>,
    inbound: mpsc::UnboundedReceiver<Vec<u8>>,
    shared: Arc<Shared>,
}

impl CitadelSession {
    /// Builds the client node and runs it on a thread of its own (the SDK's node future is
    /// not `Send`, so it cannot be a task of the daemon's runtime). The returned receiver
    /// yields once, if the node ever stops, with the SDK's reason.
    pub fn start(
        settings: SessionSettings,
    ) -> Result<(Self, tokio::sync::oneshot::Receiver<Result<(), String>>), String> {
        let (outbound, outbound_rx) = mpsc::unbounded_channel();
        let (inbound_tx, inbound) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared {
            state: Mutex::new(SessionState::Connecting),
            changed: Notify::new(),
        });
        let backend = settings.backend.clone();
        let stun = settings.stun_servers.clone();
        let kernel = SessionKernel::new(settings, shared.clone(), outbound_rx, inbound_tx);
        let (done_tx, done) = tokio::sync::oneshot::channel();
        std::thread::Builder::new()
            .name("sw-mcp-citadel".into())
            .spawn(move || {
                let result = run_node(kernel, backend, stun);
                let _ = done_tx.send(result);
            })
            .map_err(|e| e.to_string())?;
        Ok((
            Self {
                outbound,
                inbound,
                shared,
            },
            done,
        ))
    }

    /// Waits for the next message from the node; `None` once the node has stopped.
    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        self.inbound.recv().await
    }

    /// A handle that waits for state changes, held apart from the session so one task can
    /// await both a message and a change.
    pub fn watcher(&self) -> StateWatch {
        StateWatch(self.shared.clone())
    }
}

fn run_node(kernel: SessionKernel, backend: BackendType, stun: Vec<String>) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async move {
        let max_level = MaxSecurityLevel::new(kernel::SESSION_LEVEL).map_err(|e| e.to_string())?;
        let node = DefaultNodeBuilder::default()
            .with_link_manager(LinkManagerMode::Legacy)
            .with_max_security_level(max_level)
            .with_backend(backend)
            .with_stun_servers(stun)
            .with_server_misc_settings(v2_account_rules()?)
            .build(kernel)
            .map_err(|e| e.to_string())?;
        node.await.map(|_| ()).map_err(|e| e.to_string())
    })
}

/// The SDK's account store checks a new account's username against these rules on the client
/// too. Its default cap (37) predates V2 names: `<id_fp>.<slug>@<node>` needs the identity
/// crate's cap, the kernel's u8 maximum (identity doc D4).
pub fn v2_account_rules() -> Result<ServerMiscSettings, String> {
    let max = u8::try_from(citadel_identity::MAX_USERNAME_BYTES).map_err(|e| e.to_string())?;
    Ok(ServerMiscSettings {
        credential_requirements: CredentialRequirements {
            max_username_length: max,
            ..CredentialRequirements::default()
        },
        ..ServerMiscSettings::default()
    })
}

impl SessionPort for CitadelSession {
    fn state(&self) -> SessionState {
        match self.shared.state.lock() {
            Ok(s) => s.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn send(&mut self, sw_message: Vec<u8>) -> Result<(), SessionError> {
        if !matches!(self.state(), SessionState::Connected { .. }) {
            return Err(SessionError::NotConnected);
        }
        self.outbound
            .send(sw_message)
            .map_err(|_| SessionError::Transport("the Citadel node has stopped".into()))
    }

    fn poll_recv(&mut self) -> Result<Option<Vec<u8>>, SessionError> {
        match self.inbound.try_recv() {
            Ok(m) => Ok(Some(m)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Err(SessionError::Transport(
                "the Citadel node has stopped".into(),
            )),
        }
    }
}
