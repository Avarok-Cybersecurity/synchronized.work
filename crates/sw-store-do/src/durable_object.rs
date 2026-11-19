//! [`KvPort`] over a Durable Object's storage (`ctx.storage`): values are stored as
//! `Uint8Array`s, which the object's structured clone keeps as bytes, and read back from
//! either `Uint8Array` or `ArrayBuffer`. Anything else under a key is an error, never bytes.

use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use worker::Storage;

use crate::kv::{KvError, KvPort};

fn error(what: &str, e: impl std::fmt::Debug) -> KvError {
    KvError(format!("durable object storage {what}: {e:?}"))
}

#[async_trait::async_trait(?Send)]
impl KvPort for Storage {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, KvError> {
        let promise = self.as_raw().get(key).map_err(|e| error("get", e))?;
        let value = JsFuture::from(promise).await.map_err(|e| error("get", e))?;
        if value.is_undefined() {
            return Ok(None);
        }
        if let Some(bytes) = value.dyn_ref::<Uint8Array>() {
            return Ok(Some(bytes.to_vec()));
        }
        if let Some(buffer) = value.dyn_ref::<ArrayBuffer>() {
            return Ok(Some(Uint8Array::new(buffer).to_vec()));
        }
        Err(error("get", format!("{key} does not hold bytes")))
    }

    async fn put(&self, key: &str, value: &[u8]) -> Result<(), KvError> {
        self.put_raw(key, Uint8Array::from(value))
            .await
            .map_err(|e| error("put", e))
    }

    async fn delete(&self, key: &str) -> Result<(), KvError> {
        Storage::delete(self, key)
            .await
            .map(|_| ())
            .map_err(|e| error("delete", e))
    }

    async fn sync(&self) -> Result<(), KvError> {
        Storage::sync(self).await.map_err(|e| error("sync", e))
    }
}
