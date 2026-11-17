//! Ports only (design §10, master plan lane S1). The shared client state machines (HELLO,
//! subscribe/catch-up cursors, outbox replay, pairing) are written against these traits and
//! run unchanged over the three Citadel clients — the browser's wasm client (site), the
//! agent-held `citadel.session` (App) and the native client (`sw-mcp`) — and over the HTTP
//! fallback. The traits are synchronous and non-blocking so one definition serves async
//! runtimes and the browser's event loop alike: `send` enqueues, `poll_recv` never waits.

mod p2p;
mod session;

pub use p2p::{P2pError, P2pPort, PeerState};
pub use session::{SessionError, SessionPort, SessionState};
