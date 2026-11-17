//! Ports and the shared client logic (design §10, master plan lane S1). The client state
//! machines (HELLO, subscribe/catch-up cursors, the viewer's fold of the node's messages) are
//! written against these traits and run unchanged over the three Citadel clients — the
//! browser's wasm client (site), the agent-held `citadel.session` (App) and the native client
//! (`sw-mcp`) — and over the HTTP fallback. The traits are synchronous and non-blocking so one
//! definition serves async runtimes and the browser's event loop alike: `send` enqueues,
//! `poll_recv` never waits.

pub mod live;
mod p2p;
mod replay;
mod session;
pub mod sign_in;

pub use p2p::{P2pError, P2pPort, PeerState};
pub use replay::ReplayPort;
pub use session::{SessionError, SessionPort, SessionState};
