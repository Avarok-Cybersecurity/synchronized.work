//! The Citadel adapter: a [`NetKernel`] whose sessions feed [`NodeCore`]. Each C2S session's
//! channel is read for as long as it is open (the SDK runs each event handler concurrently);
//! every effect the core returns is carried out here — stored first, then sent — and a store
//! that fails stops the node, because memory would otherwise run ahead of storage.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use citadel_sdk::prelude::*;
use futures::StreamExt;
use futures::lock::Mutex as AsyncMutex;
use sw_model::Cid;
use sw_protocol::ports::Clock;

use crate::core::{Effect, NodeCore};
use crate::store::{NodeStore, StoreError};

type Sinks = AsyncMutex<BTreeMap<Cid, PeerChannelSendHalf<StackedRatchet>>>;

pub struct SwKernel<S: NodeStore, C: Clock + Send + Sync + 'static> {
    remote: Option<NodeRemote<StackedRatchet>>,
    core: Arc<Mutex<NodeCore>>,
    store: Arc<S>,
    clock: Arc<C>,
    sinks: Arc<Sinks>,
}

impl<S: NodeStore, C: Clock + Send + Sync + 'static> std::fmt::Debug for SwKernel<S, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwKernel").finish_non_exhaustive()
    }
}

impl<S: NodeStore, C: Clock + Send + Sync + 'static> SwKernel<S, C> {
    pub fn new(core: NodeCore, store: S, clock: C) -> Self {
        Self {
            remote: None,
            core: Arc::new(Mutex::new(core)),
            store: Arc::new(store),
            clock: Arc::new(clock),
            sinks: Arc::new(AsyncMutex::new(BTreeMap::new())),
        }
    }

    /// The core, for a host that reports on it (sessions, counts). Never held across an await.
    pub fn core(&self) -> Arc<Mutex<NodeCore>> {
        self.core.clone()
    }

    fn with_core<T>(&self, f: impl FnOnce(&mut NodeCore) -> T) -> Result<T, NetworkError> {
        let mut core = self
            .core
            .lock()
            .map_err(|_| NetworkError::msg("the node core is poisoned"))?;
        Ok(f(&mut core))
    }

    async fn run(&self, effects: Vec<Effect>) -> Result<(), NetworkError> {
        let (writes, sends): (Vec<_>, Vec<_>) = effects
            .into_iter()
            .partition(|e| !matches!(e, Effect::Send { .. }));
        for effect in writes {
            let stored = match &effect {
                Effect::Put { key, value } => self.store.put(key, value).await,
                Effect::Delete { key } => self.store.delete(key).await,
                Effect::Send { .. } => Ok(()),
            };
            stored.map_err(|StoreError(why)| NetworkError::msg(format!("node store: {why}")))?;
        }
        let mut sinks = self.sinks.lock().await;
        for effect in sends {
            if let Effect::Send { to, bytes } = effect
                && let Some(sink) = sinks.get_mut(&to)
                && let Err(e) = sink.send(bytes).await
            {
                log::warn!(target: "sw_node", "send to session {to} failed: {e:?}");
            }
        }
        Ok(())
    }

    async fn session(
        &self,
        cid: Cid,
        channel: PeerChannel<StackedRatchet>,
    ) -> Result<(), NetworkError> {
        let (send, mut recv) = channel.split();
        self.sinks.lock().await.insert(cid, send);
        let now = self.clock.now_ms();
        let effects = self.with_core(|core| core.connected(cid, now))?;
        let mut outcome = self.run(effects).await;
        while outcome.is_ok() {
            let Some(message) = recv.next().await else {
                break;
            };
            let now = self.clock.now_ms();
            match self.with_core(|core| core.received(cid, message.as_ref(), now))? {
                Ok(effects) => outcome = self.run(effects).await,
                Err(refusal) => {
                    log::info!(target: "sw_node", "refused from {cid}: {refusal:?}");
                    let receipt = self
                        .with_core(|core| core.refusal_receipt(cid, message.as_ref(), &refusal))?;
                    outcome = self.run(receipt).await;
                }
            }
        }
        self.sinks.lock().await.remove(&cid);
        self.with_core(|core| core.disconnected(cid))?;
        outcome
    }
}

#[async_trait]
impl<S: NodeStore, C: Clock + Send + Sync + 'static> NetKernel<StackedRatchet> for SwKernel<S, C> {
    fn load_remote(&mut self, remote: NodeRemote<StackedRatchet>) -> Result<(), NetworkError> {
        self.remote = Some(remote);
        Ok(())
    }

    async fn on_start(&self) -> Result<(), NetworkError> {
        Ok(())
    }

    async fn on_node_event_received(
        &self,
        message: NodeResult<StackedRatchet>,
    ) -> Result<(), NetworkError> {
        match message {
            NodeResult::ConnectSuccess(ConnectSuccess {
                session_cid,
                channel,
                ..
            }) => {
                let outcome = self.session(session_cid, *channel).await;
                if let Err(e) = &outcome
                    && let Some(remote) = &self.remote
                {
                    log::error!(target: "sw_node", "stopping the node: {e:?}");
                    if let Err(e) = remote.clone().shutdown().await {
                        log::error!(target: "sw_node", "the node did not stop cleanly: {e:?}");
                    }
                }
                outcome
            }
            _ => Ok(()),
        }
    }

    async fn on_stop(&mut self) -> Result<(), NetworkError> {
        Ok(())
    }
}
