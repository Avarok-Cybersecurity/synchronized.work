//! RFC 8949 §4.2.1 core deterministic encoding: shortest-form integers and lengths, definite
//! lengths only, map keys sorted by their encoded bytes, no duplicate keys. Floats, tags,
//! `null` and `undefined` never appear on the synchronized.work wire and are refused.

use ciborium::value::Value;

use crate::WireError;

/// Encodes `value` canonically (maps are sorted here, whatever order they were built in).
pub fn encode(value: &Value) -> Result<Vec<u8>, WireError> {
    let canonical = canonicalize(value.clone())?;
    let mut out = Vec::new();
    ciborium::ser::into_writer(&canonical, &mut out).map_err(|_| WireError::Encode)?;
    Ok(out)
}

/// Decodes exactly one canonical item spanning all of `bytes`. Anything that would encode
/// differently — a long-form integer, an indefinite length, unsorted or duplicate keys,
/// trailing bytes — is [`WireError::NonCanonical`].
pub fn decode(bytes: &[u8]) -> Result<Value, WireError> {
    let value: Value = ciborium::de::from_reader(bytes).map_err(|_| WireError::Malformed)?;
    if encode(&value)? != bytes {
        return Err(WireError::NonCanonical);
    }
    Ok(value)
}

fn canonicalize(value: Value) -> Result<Value, WireError> {
    Ok(match value {
        Value::Integer(_) | Value::Bytes(_) | Value::Text(_) | Value::Bool(_) => value,
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(canonicalize)
                .collect::<Result<_, _>>()?,
        ),
        Value::Map(entries) => {
            let mut keyed = Vec::with_capacity(entries.len());
            for (k, v) in entries {
                let k = canonicalize(k)?;
                let mut key_bytes = Vec::new();
                ciborium::ser::into_writer(&k, &mut key_bytes).map_err(|_| WireError::Encode)?;
                keyed.push((key_bytes, k, canonicalize(v)?));
            }
            keyed.sort_by(|a, b| a.0.cmp(&b.0));
            if keyed.windows(2).any(|w| w[0].0 == w[1].0) {
                return Err(WireError::DuplicateKey);
            }
            Value::Map(keyed.into_iter().map(|(_, k, v)| (k, v)).collect())
        }
        Value::Float(_) | Value::Tag(..) | Value::Null => return Err(WireError::ForbiddenType),
        _ => return Err(WireError::ForbiddenType),
    })
}

/// RFC 8949 §8 diagnostic notation, the language-neutral text every party's decode of a
/// fixture is compared against.
pub fn diagnostic(value: &Value) -> String {
    let mut out = String::new();
    write_diag(value, &mut out);
    out
}

fn write_diag(value: &Value, out: &mut String) {
    match value {
        Value::Integer(i) => out.push_str(&i128::from(*i).to_string()),
        Value::Bytes(b) => {
            out.push_str("h'");
            for byte in b {
                out.push_str(&format!("{byte:02x}"));
            }
            out.push('\'');
        }
        Value::Text(t) => out.push_str(&format!("{t:?}")),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_diag(item, out);
            }
            out.push(']');
        }
        Value::Map(entries) => {
            out.push('{');
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_diag(k, out);
                out.push_str(": ");
                write_diag(v, out);
            }
            out.push('}');
        }
        _ => out.push_str("<forbidden>"),
    }
}
