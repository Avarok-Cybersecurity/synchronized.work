//! The synchronized.work Worker (design §3.1): stateless routing in front of one ServerDO.
//!
//! - `GET /healthz` — the deployed commit;
//! - `GET /.well-known/citadel` — the node's signed identity document;
//! - `/citadel` — a WebSocket upgrade, forwarded to the ServerDO, which runs the Citadel node.
//!
//! There is no HTTP API: every client is a Citadel client. Natively the crate is empty.

#[cfg(target_arch = "wasm32")]
mod clock;
#[cfg(target_arch = "wasm32")]
mod console;
#[cfg(target_arch = "wasm32")]
mod env_source;
#[cfg(target_arch = "wasm32")]
mod node_store;
#[cfg(target_arch = "wasm32")]
mod routes;
#[cfg(target_arch = "wasm32")]
mod server_do;
#[cfg(target_arch = "wasm32")]
mod sql_host;
