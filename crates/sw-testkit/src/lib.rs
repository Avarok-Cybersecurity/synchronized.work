//! Test and fixture scaffolding. Every value here is deterministic so fixtures are
//! reproducible byte for byte; none of it is fit for production, which is why the sbio gate
//! refuses `sw-testkit` as a normal dependency of any crate but `xtask`.
//!
//! `expect` is allowed: a fixture that cannot be built is a broken test setup and must
//! stop the run loudly.
#![allow(clippy::expect_used)]

mod cast;
pub mod conformance;
mod entropy;
mod identity;
pub mod kat;
mod model;

pub use cast::{Cast, Directory, FIXTURE_NOW_MS};
pub use entropy::{LabelledEntropy, ReplayedEntropy};
pub use identity::{StandInIdentity, StandInIdentityPort};
