//! `sw-mcp daemon`: the Device. One task owns all state; the Citadel session, the Unix socket
//! and a one-second tick feed it. The gate decides, the inbox persists, the session carries.

mod admin;
mod inbound;
mod inbox;
mod link_code;
mod listen;
mod onboard;
mod onboard_in;
mod pad;
mod pad_in;
mod pad_post;
mod pad_req;
mod requests;
mod signals;

pub use inbox::{Inbox, QueuedSignal, hex};
pub use pad::{PAD_WAIT_MAX_S, pad_ask_timeout_s};
pub use requests::MAX_WAIT_S;

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use citadel_sdk::prelude::{BackendType, WebSocketEndpoint};
use sw_model::{Cid, IdFp};
use sw_protocol::payload::ProgressItem;
use sw_protocol::ports::{Clock as _, IdentityPort as _};
use tokio::sync::{mpsc, oneshot};

use crate::adapters::{CitadelIdentity, OsEntropy, SystemClock};
use crate::config::Config;
use crate::gate::{DeviceGate, GateLimits};
use crate::home::Home;
use crate::ipc::{Reply, Request};
use crate::keys::DeviceKeys;
use crate::outbound::Outbound;
use crate::session::{CitadelSession, SessionSettings, StateWatch};
use crate::signin::load_assertion;
use crate::status::Projection;
use crate::trust::TrustList;

/// Progress batches go out at most this often per agent (design §6.7: 1 per 2 s).
pub const PROGRESS_FLUSH_MS: u64 = 2_000;

#[derive(Debug, Clone)]
pub(crate) struct SignedIn {
    pub(crate) id_fp: IdFp,
    pub(crate) identity_pk: Vec<u8>,
    pub(crate) blob: Vec<u8>,
    pub(crate) exp_s: u64,
    /// The pinned sign-in issuer key, which scratchpad proofs are verified against.
    pub(crate) issuer_pk: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentRec {
    pub(crate) project: String,
    pub(crate) task: String,
    pub(crate) cwd: String,
}

pub(crate) struct Waiter {
    pub(crate) agent_id: String,
    pub(crate) deadline_ms: u64,
    pub(crate) reply: oneshot::Sender<Reply>,
}

pub struct Daemon {
    pub(crate) home: Home,
    pub(crate) config: Config,
    pub(crate) keys: DeviceKeys,
    pub(crate) trust: TrustList,
    pub(crate) gate: DeviceGate,
    pub(crate) signed_in: Option<SignedIn>,
    pub(crate) session: Option<CitadelSession>,
    pub(crate) watch: Option<StateWatch>,
    pub(crate) cid: Option<Cid>,
    /// The session is up and its first messages went out.
    pub(crate) online: bool,
    pub(crate) outbound: Option<Outbound>,
    pub(crate) projection: Projection,
    pub(crate) agents: BTreeMap<String, AgentRec>,
    pub(crate) inbox: Inbox,
    pub(crate) batches: BTreeMap<String, Vec<ProgressItem>>,
    pub(crate) last_flush_ms: u64,
    pub(crate) unsent: VecDeque<Vec<u8>>,
    pub(crate) waiters: Vec<Waiter>,
    pub(crate) pad: crate::pad::state::PadState,
    pub(crate) pad_waiters: Vec<pad::PadWaiter>,
    pub(crate) pad_asks: Vec<pad::PadAsk>,
    pub(crate) onboard: Vec<onboard::OnboardWait>,
    pub(crate) projects: crate::projects::Projects,
    pub(crate) entropy: OsEntropy,
}

impl std::fmt::Debug for Daemon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Daemon")
            .field("cid", &self.cid)
            .field("agents", &self.agents.len())
            .field("queued", &self.inbox.len())
            .finish_non_exhaustive()
    }
}

/// Wall-clock milliseconds (the daemon's `Clock`).
pub fn now_ms() -> u64 {
    SystemClock.now_ms()
}

impl Daemon {
    /// Loads (or, the first time, generates) the Device's state from `home`.
    pub fn open(home: Home, config: Config) -> Result<Self, String> {
        home.ensure_layout().map_err(|e| e.to_string())?;
        let keys = crate::store::load_or_create_keys(&home, &mut OsEntropy)?;
        let trust = crate::store::load_trust(&home, &keys)?;
        let log = crate::store::load_replay_log(&home)?;
        let inbox = Inbox::load(home.inbox())?;
        let gate = DeviceGate::new(GateLimits::from_config(&config), log);
        let pad_state = pad::load_state(&home)?;
        let projects = onboard_in::load_projects(&home)?;
        let mut daemon = Self {
            home,
            config,
            keys,
            trust,
            gate,
            signed_in: None,
            session: None,
            watch: None,
            cid: None,
            online: false,
            outbound: None,
            projection: Projection::empty(),
            agents: BTreeMap::new(),
            inbox,
            batches: BTreeMap::new(),
            last_flush_ms: 0,
            unsent: VecDeque::new(),
            waiters: Vec::new(),
            pad: pad_state,
            pad_waiters: Vec::new(),
            pad_asks: Vec::new(),
            onboard: Vec::new(),
            projects,
            entropy: OsEntropy,
        };
        daemon.load_sign_in();
        Ok(daemon)
    }

    /// Reads the stored assertion; without one the daemon serves the socket and waits.
    pub(crate) fn load_sign_in(&mut self) {
        let issuer = crate::signin::issuer_key(&self.home);
        match (load_assertion(&self.home, now_ms() / 1000), issuer) {
            (Ok(a), Ok(issuer_pk)) => match CitadelIdentity.id_fp(&a.identity_pk) {
                Ok(id_fp) => {
                    log::info!("signed in as {}; device {}", id_fp.as_str(), a.device);
                    self.signed_in = Some(SignedIn {
                        id_fp,
                        identity_pk: a.identity_pk,
                        blob: a.blob,
                        exp_s: a.exp_s,
                        issuer_pk,
                    });
                }
                Err(e) => log::error!("the stored assertion's identity key: {e:?}"),
            },
            (Err(e), _) | (_, Err(e)) => log::warn!("{e}"),
        }
    }

    /// Starts the Citadel session once signed in (and only once).
    pub(crate) fn ensure_session(&mut self) -> Result<(), String> {
        if self.session.is_some() {
            return Ok(());
        }
        let Some(signed_in) = &self.signed_in else {
            return Ok(());
        };
        let username = self
            .config
            .device_username(&signed_in.id_fp)
            .map_err(|e| e.to_string())?;
        let endpoint = WebSocketEndpoint::parse(&self.config.server).map_err(|e| e.to_string())?;
        let backend_dir = self.home.citadel_backend().to_string_lossy().into_owned();
        let settings = SessionSettings {
            endpoint,
            username,
            password: self.keys.citadel_password.clone(),
            backend: BackendType::Filesystem(backend_dir),
            reconnect_delay: Duration::from_secs(self.config.reconnect_delay_s),
            stun_servers: self.config.stun_servers.clone(),
            cert_verification: self.config.cert_verification().map_err(|e| e.to_string())?,
        };
        let (session, done) = CitadelSession::start(settings)?;
        tokio::spawn(async move {
            match done.await {
                Ok(Ok(())) => log::warn!("the Citadel node stopped"),
                Ok(Err(e)) => log::error!("the Citadel node stopped: {e}"),
                Err(_) => log::error!("the Citadel node thread ended without a result"),
            }
        });
        self.watch = Some(session.watcher());
        self.session = Some(session);
        Ok(())
    }

    pub async fn run(mut self) -> Result<(), String> {
        let (ipc_tx, mut ipc_rx) = mpsc::unbounded_channel::<(Request, oneshot::Sender<Reply>)>();
        listen::bind(self.home.ipc(), ipc_tx)?;
        self.ensure_session()?;
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                Some((request, reply)) = ipc_rx.recv() => self.handle(request, reply),
                Some(bytes) = recv_from(&mut self.session) => self.on_message(&bytes),
                () = state_change(&self.watch) => self.on_state(),
                _ = tick.tick() => self.on_tick(),
            }
        }
    }
}

async fn recv_from(session: &mut Option<CitadelSession>) -> Option<Vec<u8>> {
    match session {
        Some(s) => s.recv().await,
        None => std::future::pending().await,
    }
}

async fn state_change(watch: &Option<StateWatch>) {
    match watch {
        Some(w) => w.changed().await,
        None => std::future::pending().await,
    }
}
