/// Where a peer stands from this Endpoint's side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    Unregistered,
    Registered,
    Connected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum P2pError {
    /// `send` to a peer whose channel is not up; the caller falls back to the inbox
    /// (design §4.8: "otherwise, or on any error, queue to the inbox").
    NotConnected,
    /// The SDK refused the registration or connection.
    Refused(String),
    Transport(String),
}

/// Citadel P2P between two Endpoints of the synchronized.work node (design §4.8). Peers are
/// addressed by their Citadel **username** (`<id_fp>.<slug>@synchronized.work`), never by
/// CID: the SDK's `propose_target` / `find_target` take usernames. Every payload is a P2P
/// envelope (`sw_protocol::p2p::encode_p2p`) around a signed `SwMessage`, so the receiver
/// runs the same gate as on the inbox path.
pub trait P2pPort {
    fn peer_state(&self, peer_username: &str) -> PeerState;

    /// `propose_target` for a new peer, `find_target` for one already registered.
    fn register(&mut self, peer_username: &str) -> Result<(), P2pError>;

    /// `connect_to_peer`; the channel is up once [`PeerState::Connected`] is reported.
    fn connect(&mut self, peer_username: &str) -> Result<(), P2pError>;

    /// Removes the registration (a co-worker's `WORKS_ON` edge went away).
    fn deregister(&mut self, peer_username: &str) -> Result<(), P2pError>;

    fn send(&mut self, peer_username: &str, envelope: Vec<u8>) -> Result<(), P2pError>;

    /// The next `(peer_username, envelope)` received, if any; never blocks.
    fn poll_recv(&mut self) -> Result<Option<(String, Vec<u8>)>, P2pError>;
}
