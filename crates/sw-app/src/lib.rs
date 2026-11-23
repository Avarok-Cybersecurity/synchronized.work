//! The synchronized.work Citadel App (design §7): [`SwApp`] implements `citadel-app-sdk`'s `App`
//! over `sw-app-core`, which makes every decision. This crate only translates: host events and
//! replies into the core's `Input`s, the core's `Effect`s into SDK calls ([`map`],
//! [`context`], [`theme`]), with every value of its own in [`config`].
//!
//! A library, not the module: citadel-workspace's `apps/first-party/synchronized-work/app`
//! exports it with `citadel_app_sdk::export_app!(sw_app::SwApp)` and builds the committed wasm.
//! The SDK builds for `wasm32-unknown-unknown` only, so [`SwApp`] exists there alone; the
//! mapping, context and theme functions build everywhere and are tested natively.

pub mod config;
pub mod context;
pub mod map;
pub mod theme;

#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod fetch;

#[cfg(target_arch = "wasm32")]
pub use app::{HostMeasure, SwApp};
