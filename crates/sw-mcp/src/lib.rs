//! `sw-mcp` (design §9): the synchronized.work MCP on a person's machine.
//!
//! Pure decisions — the gate, the provenance frame, the status projection, sign-in, the
//! allowlist format and every outbound message — sit beside the adapters that do I/O: the
//! Citadel session, the issuer over HTTPS, `~/.synchronized/`, the Unix socket and stdio.

pub mod adapters;
pub mod cli;
pub mod config;
pub mod daemon;
pub mod frame;
pub mod gate;
pub mod home;
pub mod hook;
pub mod ipc;
pub mod keys;
pub mod logging;
pub mod outbound;
pub mod serve;
pub mod session;
pub mod signin;
pub mod status;
pub mod store;
pub mod trust;
