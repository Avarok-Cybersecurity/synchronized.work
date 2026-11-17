//! `DoLatticeStorage` passes lattice-db's exported `LatticeStorage` conformance suite, and the
//! suite's own negative control — a store that drops the last page on `sync` — fails it.
//!
//! The port under the backend keeps a Durable Object's storage semantics in memory: one map
//! that outlives every handle (each `open` is the object restarting), every `put` atomic per key
//! and durable on return. What runs is the production backend, key encoding and size refusal
//! included; only the object's storage is modelled.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use lattice_core::storage::StorageResult;
use lattice_test_harness::conformance::controls::DropsLastPageOnSync;
use lattice_test_harness::{StorageFixture, run_conformance};
use sw_store_do::{DoLatticeStorage, KvError, KvPort};

/// The SQLite-backed object's per-value limit as of 2026-11 (2 MiB of key and value): a test
/// figure, since production states its own in wrangler.toml.
const TEST_MAX_VALUE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Default)]
struct ObjectStorage(Arc<Mutex<BTreeMap<String, Vec<u8>>>>);

impl ObjectStorage {
    fn map(&self) -> Result<std::sync::MutexGuard<'_, BTreeMap<String, Vec<u8>>>, KvError> {
        self.0.lock().map_err(|_| KvError("poisoned".into()))
    }
}

#[async_trait::async_trait]
impl KvPort for ObjectStorage {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, KvError> {
        Ok(self.map()?.get(key).cloned())
    }
    async fn put(&self, key: &str, value: &[u8]) -> Result<(), KvError> {
        self.map()?.insert(key.to_owned(), value.to_vec());
        Ok(())
    }
    async fn delete(&self, key: &str) -> Result<(), KvError> {
        self.map()?.remove(key);
        Ok(())
    }
    async fn sync(&self) -> Result<(), KvError> {
        Ok(())
    }
}

#[derive(Default)]
struct ObjectFixture(ObjectStorage);

#[async_trait::async_trait(?Send)]
impl StorageFixture for ObjectFixture {
    type Storage = DoLatticeStorage<ObjectStorage>;

    async fn open(&self) -> StorageResult<Self::Storage> {
        Ok(DoLatticeStorage::new(self.0.clone(), TEST_MAX_VALUE_BYTES))
    }
}

#[tokio::test]
async fn the_durable_object_backend_conforms() {
    run_conformance(ObjectFixture::default)
        .await
        .assert_conformant();
}

#[tokio::test]
async fn a_backend_that_drops_the_last_page_on_sync_fails_the_crash_check() {
    let report = run_conformance(|| DropsLastPageOnSync(ObjectFixture::default())).await;
    assert!(
        report.failed_check("crash::acknowledged_commits_survive_reopen"),
        "a store that lost an acknowledged page passed: {report:?}"
    );
}

#[tokio::test]
async fn a_value_over_the_object_limit_is_refused_and_writes_nothing() {
    use lattice_core::LatticeStorage;
    let object = ObjectStorage::default();
    let storage = DoLatticeStorage::new(object.clone(), 8);
    assert!(storage.write_page(1, &[0; 9]).await.is_err());
    assert!(storage.set_meta("k", &[0; 9]).await.is_err());
    assert!(object.map().map(|m| m.is_empty()).unwrap_or(false));
    assert!(storage.write_page(1, &[0; 8]).await.is_ok());
}
