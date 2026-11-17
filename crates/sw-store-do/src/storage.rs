//! `LatticeStorage` over a [`KvPort`].

use lattice_core::LatticeStorage;
use lattice_core::storage::{Page, StorageError, StorageResult};

use crate::keys::{meta_key, page_key};
use crate::kv::{KvError, KvPort};

/// The backend. `max_value_bytes` is the object's per-value limit, stated by the caller
/// (PCND): there is no built-in figure, because it is the platform's and can change.
#[derive(Debug, Clone)]
pub struct DoLatticeStorage<K> {
    kv: K,
    max_value_bytes: usize,
}

impl<K: KvPort> DoLatticeStorage<K> {
    pub fn new(kv: K, max_value_bytes: usize) -> Self {
        Self {
            kv,
            max_value_bytes,
        }
    }

    async fn put(&self, key: String, value: &[u8]) -> StorageResult<()> {
        if value.len() > self.max_value_bytes {
            return Err(StorageError::CapacityExceeded);
        }
        self.kv.put(&key, value).await.map_err(io)
    }
}

fn io(e: KvError) -> StorageError {
    StorageError::Io { message: e.0 }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
impl<K: KvPort> LatticeStorage for DoLatticeStorage<K> {
    async fn get_meta(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        self.kv.get(&meta_key(key)).await.map_err(io)
    }
    async fn set_meta(&self, key: &str, value: &[u8]) -> StorageResult<()> {
        self.put(meta_key(key), value).await
    }
    async fn delete_meta(&self, key: &str) -> StorageResult<()> {
        self.kv.delete(&meta_key(key)).await.map_err(io)
    }
    async fn read_page(&self, page_id: u64) -> StorageResult<Page> {
        self.kv
            .get(&page_key(page_id))
            .await
            .map_err(io)?
            .ok_or(StorageError::PageNotFound { page_id })
    }
    async fn write_page(&self, page_id: u64, data: &[u8]) -> StorageResult<()> {
        self.put(page_key(page_id), data).await
    }
    async fn page_exists(&self, page_id: u64) -> StorageResult<bool> {
        Ok(self.kv.get(&page_key(page_id)).await.map_err(io)?.is_some())
    }
    async fn delete_page(&self, page_id: u64) -> StorageResult<()> {
        self.kv.delete(&page_key(page_id)).await.map_err(io)
    }
    async fn sync(&self) -> StorageResult<()> {
        self.kv.sync().await.map_err(io)
    }
}
