//! The SDK's account backend (`BackendType::HostSql`) over the object's SQLite: the SDK owns
//! the schema and the statements (its `host_sql` backend, run natively against SQLite by its
//! own suite); this file only executes a batch inside `transactionSync`, so a batch applies
//! whole or not at all, as `SqlHost::execute` requires.
//!
//! workers-rs 0.8.7 binds no `transactionSync`, so the one method is bound here (design §10:
//! "if either is missing ... exactly those calls").

use std::cell::RefCell;
use std::rc::Rc;

use citadel_sdk::prelude::{SqlHost, SqlRow, SqlStatement, SqlValue, StorageQuota};
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;
use worker::send::SendWrapper;
use worker::{SqlStorage, SqlStorageValue, Storage};

#[wasm_bindgen]
extern "C" {
    /// The object's `ctx.storage`, seen through the one method bound here.
    type TransactionalStorage;

    #[wasm_bindgen(method, catch, js_name = transactionSync)]
    fn transaction_sync(
        this: &TransactionalStorage,
        body: &js_sys::Function,
    ) -> Result<JsValue, JsValue>;
}

type Batch = Result<Vec<Vec<SqlRow>>, String>;

/// Largest integer a JS number carries exactly.
const MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;

pub struct DurableObjectSql(SendWrapper<Storage>);

impl DurableObjectSql {
    pub fn new(storage: Storage) -> Self {
        Self(SendWrapper::new(storage))
    }
}

fn param(value: &SqlValue) -> Result<SqlStorageValue, String> {
    Ok(match value {
        SqlValue::Null => SqlStorageValue::Null,
        SqlValue::Integer(n) if n.unsigned_abs() <= MAX_SAFE_INTEGER as u64 => {
            SqlStorageValue::Integer(*n)
        }
        SqlValue::Integer(n) => return Err(format!("integer {n} does not fit a JS number")),
        SqlValue::Text(t) => SqlStorageValue::String(t.clone()),
        SqlValue::Blob(b) => SqlStorageValue::Blob(b.clone()),
    })
}

fn column(value: SqlStorageValue) -> Result<SqlValue, String> {
    Ok(match value {
        SqlStorageValue::Null => SqlValue::Null,
        SqlStorageValue::Integer(n) => SqlValue::Integer(n),
        SqlStorageValue::Float(f) if f.fract() == 0.0 && f.abs() <= MAX_SAFE_INTEGER as f64 => {
            SqlValue::Integer(f as i64)
        }
        SqlStorageValue::String(t) => SqlValue::Text(t),
        SqlStorageValue::Blob(b) => SqlValue::Blob(b),
        other => return Err(format!("unsupported column value {other:?}")),
    })
}

fn run(sql: &SqlStorage, statements: &[SqlStatement]) -> Result<Vec<Vec<SqlRow>>, String> {
    let mut results = Vec::with_capacity(statements.len());
    for statement in statements {
        let params = statement
            .params
            .iter()
            .map(param)
            .collect::<Result<Vec<_>, _>>()?;
        let cursor = sql.exec(statement.sql, params).map_err(|e| e.to_string())?;
        let mut rows = Vec::new();
        for row in cursor.raw() {
            let row = row.map_err(|e| e.to_string())?;
            rows.push(
                row.into_iter()
                    .map(column)
                    .collect::<Result<SqlRow, String>>()?,
            );
        }
        results.push(rows);
    }
    Ok(results)
}

#[async_trait::async_trait]
impl SqlHost for DurableObjectSql {
    async fn execute(&self, statements: Vec<SqlStatement>) -> Result<Vec<Vec<SqlRow>>, String> {
        let storage: &Storage = &self.0.0;
        let sql = storage.sql();
        let outcome: Rc<RefCell<Option<Batch>>> = Rc::new(RefCell::new(None));
        let slot = outcome.clone();
        let body = Closure::once(move || -> Result<JsValue, JsValue> {
            let result = run(&sql, &statements);
            let failed = result.as_ref().err().cloned();
            *slot.borrow_mut() = Some(result);
            // Throwing rolls the whole batch back.
            failed.map_or(Ok(JsValue::UNDEFINED), |why| Err(JsValue::from_str(&why)))
        });
        let called = storage
            .as_raw()
            .unchecked_ref::<TransactionalStorage>()
            .transaction_sync(body.as_ref().unchecked_ref());
        drop(body);
        let ran = outcome.borrow_mut().take();
        match (called, ran) {
            (Ok(_), Some(Ok(rows))) => Ok(rows),
            (_, Some(Err(why))) => Err(why),
            (Err(e), _) => Err(format!("transactionSync: {e:?}")),
            (Ok(_), None) => Err("transactionSync did not run the batch".into()),
        }
    }

    /// The node keeps no RE-VFS objects: uploads are refused, explicitly.
    fn storage_quota(&self) -> Result<StorageQuota, String> {
        Ok(StorageQuota::Bytes(0))
    }
}
