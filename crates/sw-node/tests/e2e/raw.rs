//! A session before `HELLO`, and frames that are not `SwMessage`s: what a Citadel agent sends
//! right after it connects (the server identity challenge) and reads back.

use super::*;

impl Account {
    /// An app session's account name (`citadel-session-abi.md` §4):
    /// `<id_fp>.app-<install_id[..8]>@synchronized.work`, 47 bytes.
    pub fn app_session() -> Self {
        Self::of(0x03, "app-0123abcd")
    }
}

impl Client {
    /// Registers and signs in; sends nothing.
    pub async fn open(
        remote: &NodeRemote<StackedRatchet>,
        addr: SocketAddr,
        account: Account,
    ) -> Result<Self, NetworkError> {
        let user = account.username.clone();
        remote
            .register_with_defaults(addr, user.as_str(), user.as_str(), PASSWORD, LOOPBACK_CERTS)
            .await?;
        let mut conn = remote
            .connect_with_defaults(
                AuthenticationRequest::credentialed(user, PASSWORD),
                LOOPBACK_CERTS,
            )
            .await?;
        let (tx, rx) = conn.take_channel().expect("a C2S channel").split();
        Ok(Self {
            account,
            cid: conn.cid,
            tx,
            rx,
            seq: 0,
        })
    }

    pub async fn send_raw(&mut self, frame: Vec<u8>) -> Result<(), NetworkError> {
        self.tx.send(frame).await
    }

    pub async fn next_frame(&mut self) -> Result<Vec<u8>, NetworkError> {
        match self.rx.next().await {
            Some(bytes) => Ok(bytes.as_ref().to_vec()),
            None => Err(NetworkError::msg("the session closed")),
        }
    }
}
