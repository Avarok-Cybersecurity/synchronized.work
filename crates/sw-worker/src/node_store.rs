//! `sw_node::store::NodeStore` over the object's storage: one `put` of one key per write,
//! under the `n/` prefix so the node's keys never meet another store's in the same object.

use js_sys::{Array, Map, Uint8Array};
use sw_node::store::{NodeStore, StoreError};
use sw_store_do::KvPort;
use wasm_bindgen::JsCast;
use worker::send::{SendFuture, SendWrapper};
use worker::{ListOptions, Storage};

pub const PREFIX: &str = "n/";

pub struct DoNodeStore(SendWrapper<Storage>);

impl DoNodeStore {
    pub fn new(storage: Storage) -> Self {
        Self(SendWrapper::new(storage))
    }
}

fn error(e: impl std::fmt::Debug) -> StoreError {
    StoreError(format!("{e:?}"))
}

#[async_trait::async_trait]
impl NodeStore for DoNodeStore {
    async fn load_all(&self) -> Result<Vec<(String, Vec<u8>)>, StoreError> {
        let storage: &Storage = &self.0.0;
        SendFuture::new(async move {
            let map: Map = storage
                .list_with_options(ListOptions::new().prefix(PREFIX))
                .await
                .map_err(error)?;
            let mut out = Vec::new();
            for entry in map.entries() {
                let pair: Array = entry.map_err(error)?.unchecked_into();
                let key = pair
                    .get(0)
                    .as_string()
                    .ok_or_else(|| error("a key that is not text"))?;
                let bytes = pair
                    .get(1)
                    .dyn_into::<Uint8Array>()
                    .map_err(|_| error(format!("{key} does not hold bytes")))?;
                let Some(key) = key.strip_prefix(PREFIX) else {
                    return Err(error(format!("{key} listed outside {PREFIX}")));
                };
                out.push((key.to_owned(), bytes.to_vec()));
            }
            Ok(out)
        })
        .await
    }

    async fn put(&self, key: &str, value: &[u8]) -> Result<(), StoreError> {
        let storage: &Storage = &self.0.0;
        let key = format!("{PREFIX}{key}");
        SendFuture::new(async move { KvPort::put(storage, &key, value).await.map_err(error) }).await
    }

    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        let storage: &Storage = &self.0.0;
        let key = format!("{PREFIX}{key}");
        SendFuture::new(async move { KvPort::delete(storage, &key).await.map_err(error) }).await
    }
}
