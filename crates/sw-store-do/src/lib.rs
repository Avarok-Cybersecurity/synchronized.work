//! lattice-db's [`LatticeStorage`] over a Durable Object's transactional storage (design §2.6,
//! lattice-core `storage.rs` "Cloudflare Durable Objects").
//!
//! The mapping is the one lattice-db documents for a SQLite-backed object:
//! - every metadata key and every page is **one `put` of one key**, which the object writes
//!   atomically, so `set_meta` is atomic per key as the WAL requires;
//! - page ids are encoded **losslessly** as fixed-width hex (`p:` + 16 digits): snapshot chunks
//!   sit above `2^32` and the trait allows any `u64`, past what a JavaScript number holds;
//! - a value larger than the object's per-value limit is refused before the `put`, so a refused
//!   write changed nothing;
//! - `sync` awaits the storage's own `sync()`.
//!
//! The storage itself is behind [`KvPort`] (SBIO): the Durable Object implements it in
//! `durable_object` (wasm32), and the tests run lattice-db's conformance suite against a port
//! that keeps the object's semantics in memory. Checkpoints are taken from the object's alarm
//! (`checkpoint`, wasm32: lattice-db's checkpointing engine exists only there).

mod keys;
mod kv;
mod storage;

#[cfg(target_arch = "wasm32")]
pub mod checkpoint;
#[cfg(target_arch = "wasm32")]
pub mod durable_object;

pub use keys::{META_PREFIX, PAGE_PREFIX, meta_key, page_key};
pub use kv::{KvError, KvPort};
pub use storage::DoLatticeStorage;
