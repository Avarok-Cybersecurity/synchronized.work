//! The port the backend is written against: a Durable Object's key-value storage.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvError(pub String);

impl fmt::Display for KvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One key, one value, written atomically. A call that returns `Err` changed nothing (the
/// object's `put` either commits or fails), and once `sync` returns `Ok` every write before it
/// is durable.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
pub trait KvPort: Send + Sync + 'static {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, KvError>;
    async fn put(&self, key: &str, value: &[u8]) -> Result<(), KvError>;
    async fn delete(&self, key: &str) -> Result<(), KvError>;
    async fn sync(&self) -> Result<(), KvError>;
}

/// The wasm32 form: one thread, and the object's storage is a JavaScript handle.
#[cfg(target_arch = "wasm32")]
#[async_trait::async_trait(?Send)]
pub trait KvPort: 'static {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, KvError>;
    async fn put(&self, key: &str, value: &[u8]) -> Result<(), KvError>;
    async fn delete(&self, key: &str) -> Result<(), KvError>;
    async fn sync(&self) -> Result<(), KvError>;
}
