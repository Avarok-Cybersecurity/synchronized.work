//! The SDK kernel behind [`super::CitadelSession`]: register once, log in, pump the C2S
//! channel both ways, and after a drop wait the configured delay and log in again with
//! `force_login` (a reconnecting client displaces its own half-open session, per the SDK).

use std::sync::Arc;

use citadel_sdk::async_trait;
use citadel_sdk::prelude::*;
use futures::StreamExt as _;
use sw_client::SessionState;
use tokio::sync::{Mutex, mpsc};

use super::{SessionSettings, Shared};

/// The level the session is built at; the client node's maximum is this too.
pub(super) const SESSION_LEVEL: SecurityLevel = SecurityLevel::Standard;

/// The session's suite, every part named: ML-KEM-1024 (FIPS 203) with AES-256-GCM, and no
/// per-packet signature, since every `SwMessage` carries its own ML-DSA-65 signature.
fn session_security() -> SessionSecuritySettings {
    SessionSecuritySettings {
        security_level: SESSION_LEVEL,
        secrecy_mode: SecrecyMode::BestEffort,
        crypto_params: CryptoParameters::new(
            EncryptionAlgorithm::AES_GCM_256,
            KemAlgorithm::MlKem1024Fips203,
            SigAlgorithm::None,
        ),
        header_obfuscator_settings: HeaderObfuscatorSettings::Disabled,
    }
}

pub(super) struct SessionKernel {
    remote: Option<NodeRemote<StackedRatchet>>,
    settings: SessionSettings,
    shared: Arc<Shared>,
    outbound: Mutex<mpsc::UnboundedReceiver<Vec<u8>>>,
    inbound: mpsc::UnboundedSender<Vec<u8>>,
}

impl SessionKernel {
    pub(super) fn new(
        settings: SessionSettings,
        shared: Arc<Shared>,
        outbound: mpsc::UnboundedReceiver<Vec<u8>>,
        inbound: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Self {
        Self {
            remote: None,
            settings,
            shared,
            outbound: Mutex::new(outbound),
            inbound,
        }
    }

    async fn login(
        &self,
        remote: &NodeRemote<StackedRatchet>,
    ) -> Result<CitadelClientServerConnection<StackedRatchet>, NetworkError> {
        let s = &self.settings;
        let known = remote
            .account_manager()
            .get_client_by_username(&s.username)
            .await
            .map_err(|e| NetworkError::msg(format!("the local account store: {e:?}")))?;
        if known.is_none() {
            remote
                .register_to_endpoint(
                    s.endpoint.clone(),
                    s.username.as_str(),
                    s.username.as_str(),
                    s.password.as_str(),
                    session_security(),
                    None,
                    s.cert_verification,
                )
                .await?;
        }
        remote
            .connect(
                AuthenticationRequest::credentialed(s.username.clone(), s.password.as_str()),
                ConnectMode::Standard { force_login: true },
                UdpMode::Disabled,
                None,
                session_security(),
                None,
                s.cert_verification,
            )
            .await
    }

    /// Carries messages until the channel closes; answers why.
    async fn pump(&self, mut conn: CitadelClientServerConnection<StackedRatchet>) -> String {
        let Some(channel) = conn.take_channel() else {
            return "the session opened without a channel".into();
        };
        let (mut tx, mut rx) = channel.split();
        self.shared.set(SessionState::Connected { cid: conn.cid });
        let mut outbound = self.outbound.lock().await;
        loop {
            tokio::select! {
                out = outbound.recv() => match out {
                    Some(bytes) => {
                        if let Err(e) = tx.send(SecBuffer::from(bytes)).await {
                            return format!("send failed: {e}");
                        }
                    }
                    None => return "the daemon closed the session".into(),
                },
                inbound = rx.next() => match inbound {
                    Some(message) => {
                        if self.inbound.send(message.as_ref().to_vec()).is_err() {
                            return "the daemon stopped reading".into();
                        }
                    }
                    None => return "the node closed the channel".into(),
                },
            }
        }
    }
}

#[async_trait]
impl NetKernel<StackedRatchet> for SessionKernel {
    fn load_remote(&mut self, node_remote: NodeRemote<StackedRatchet>) -> Result<(), NetworkError> {
        self.remote = Some(node_remote);
        Ok(())
    }

    async fn on_start(&self) -> Result<(), NetworkError> {
        let Some(remote) = self.remote.clone() else {
            return Err(NetworkError::msg(
                "the SDK started the kernel without a remote",
            ));
        };
        loop {
            self.shared.set(SessionState::Connecting);
            let reason = match self.login(&remote).await {
                Ok(conn) => self.pump(conn).await,
                Err(e) => format!("login failed: {e}"),
            };
            self.shared.set(SessionState::Disconnected {
                reason: reason.clone(),
            });
            if self.inbound.is_closed() {
                // The daemon is gone: leave the node now rather than hold a session open.
                return remote.shutdown().await;
            }
            tokio::time::sleep(self.settings.reconnect_delay).await;
        }
    }

    async fn on_node_event_received(
        &self,
        _message: NodeResult<StackedRatchet>,
    ) -> Result<(), NetworkError> {
        Ok(())
    }

    async fn on_stop(&mut self) -> Result<(), NetworkError> {
        Ok(())
    }
}
