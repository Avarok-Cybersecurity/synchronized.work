use sw_model::Cid;

/// The Citadel C2S session to the synchronized.work node, as the client logic sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    /// Signing in or reconnecting; nothing can be sent.
    Connecting,
    /// Connected as the Endpoint account with this permanent CID.
    Connected { cid: Cid },
    /// Closed; `reason` is for display and logs only.
    Disconnected { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// `send` while not [`SessionState::Connected`]; the caller keeps the message in its
    /// outbox and replays it after reconnecting (`seq` makes the replay idempotent).
    NotConnected,
    /// The message exceeds what the transport carries (e.g. the App's
    /// `limits.session.max_msg_bytes`).
    TooLarge { max: usize },
    /// Any other transport failure, never swallowed.
    Transport(String),
}

/// Bytes to and from the node: each `send` is one encoded `SwMessage`, each received item is
/// one encoded `SwMessage`. Framing, encryption and reconnection are the SDK's.
pub trait SessionPort {
    fn state(&self) -> SessionState;

    /// Enqueues one message for the node.
    fn send(&mut self, sw_message: Vec<u8>) -> Result<(), SessionError>;

    /// The next message from the node, if one has arrived; never blocks.
    fn poll_recv(&mut self) -> Result<Option<Vec<u8>>, SessionError>;
}
