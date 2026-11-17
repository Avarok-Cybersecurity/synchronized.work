//! The synchronized.work node (design rev 3 §3): a Citadel server whose `[identity]` is
//! `synchronized.work`. It refuses to start unprovisioned ([`provision`]), verifies every
//! signed message through `sw-protocol`'s gate, runs the Task tree and the tier transitions of
//! `sw-model` ([`core`]), and pushes significant events to the Persons concerned. The Citadel
//! SDK is reached only from [`kernel`] and [`start`]; everything above them is pure.

pub mod core;
pub mod identity;
pub mod kernel;
pub mod limits;
pub mod provision;
pub mod start;
pub mod store;
pub mod well_known;

pub use provision::{ProvisionError, Provisioning, Source};
