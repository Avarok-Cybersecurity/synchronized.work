//! The synchronized.work Worker (design §3.1): stateless routing in front of one ServerDO.
//!
//! - `GET /healthz` — the deployed commit and whether the node is provisioned;
//! - `GET /.well-known/citadel` — the node's signed identity document;
//! - `/citadel` — a WebSocket upgrade, forwarded to the ServerDO, which runs the Citadel node.
//!
//! - `POST /auth/return` — where a browser sign-in returns (sign-in contract §5);
//! - everything else — the site's static assets, with the security headers of `security`;
//!   a miss gets the app shell, or 404 when the path names a file (`fallback`).
//!
//! There is no HTTP API: every client is a Citadel client. Natively the crate holds only the
//! pure `fallback` rules.

#[cfg(target_arch = "wasm32")]
mod clock;
#[cfg(target_arch = "wasm32")]
mod console;
#[cfg(target_arch = "wasm32")]
mod env_source;
pub mod fallback;
#[cfg(target_arch = "wasm32")]
mod node_store;
#[cfg(target_arch = "wasm32")]
mod routes;
#[cfg(target_arch = "wasm32")]
mod security;
#[cfg(target_arch = "wasm32")]
mod server_do;
#[cfg(target_arch = "wasm32")]
mod sql_host;
