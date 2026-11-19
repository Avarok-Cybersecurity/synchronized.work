//! A native Citadel client of the synchronized.work node, for measurement and for the node's
//! end-to-end test: register, sign in, enrol by assertion, and send `SwMessage`s on the C2S
//! channel. The assertion is minted with `citadel-identity`'s own `ServerKey`, as
//! work.avarok.net mints it, from a test seed the node under test is provisioned to trust.

#![cfg(not(target_arch = "wasm32"))]

pub mod client;
pub mod kernel;
pub mod payloads;
pub mod probe;
