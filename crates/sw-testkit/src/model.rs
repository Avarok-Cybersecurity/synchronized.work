//! The contract's fixture `model` notation (sw-wire contract §1): `{"$map": [[k, v], …]}` in
//! canonical order, `{"$hex": "…"}` for byte strings, `{"$u64": "decimal"}` for integers at
//! or above 2^53, plain numbers, strings, booleans and arrays.

use ciborium::value::Value;
use serde_json::{Value as Json, json};

const JSON_SAFE_LIMIT: u128 = 1 << 53;

pub fn to_model(value: &Value) -> Result<Json, String> {
    Ok(match value {
        Value::Integer(i) => {
            let n = i128::from(*i);
            if n < 0 {
                return Err("negative integers never appear on the sw wire".into());
            }
            if n as u128 >= JSON_SAFE_LIMIT {
                json!({ "$u64": n.to_string() })
            } else {
                json!(n as u64)
            }
        }
        Value::Bytes(b) => json!({ "$hex": hex(b) }),
        Value::Text(t) => json!(t),
        Value::Bool(b) => json!(b),
        Value::Array(items) => Json::Array(items.iter().map(to_model).collect::<Result<_, _>>()?),
        Value::Map(entries) => {
            let pairs = entries
                .iter()
                .map(|(k, v)| Ok(json!([to_model(k)?, to_model(v)?])))
                .collect::<Result<Vec<Json>, String>>()?;
            json!({ "$map": pairs })
        }
        _ => return Err("a float, tag or null is not in the model notation".into()),
    })
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn unhex(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err("odd-length hex".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}
