//! The port the node stores through (SBIO). The Durable Object implements it over its own
//! storage (one `put` per key, `sw-worker`); tests implement it in memory.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError(pub String);

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Every write is one key, atomic; a call that returns `Err` changed nothing. Futures are
/// `Send` because the Citadel kernel that awaits them requires it (a Durable Object adapter
/// on single-threaded wasm32 wraps its handle accordingly).
#[async_trait::async_trait]
pub trait NodeStore: Send + Sync + 'static {
    /// Every key the node wrote and has not deleted.
    async fn load_all(&self) -> Result<Vec<(String, Vec<u8>)>, StoreError>;
    async fn put(&self, key: &str, value: &[u8]) -> Result<(), StoreError>;
    async fn delete(&self, key: &str) -> Result<(), StoreError>;
}
