//! Test and fixture scaffolding. Every value here is deterministic so fixtures are
//! reproducible byte for byte; none of it is fit for production, which is why the sbio gate
//! refuses `sw-testkit` as a normal dependency of any crate but `xtask`.

mod entropy;

pub use entropy::LabelledEntropy;
