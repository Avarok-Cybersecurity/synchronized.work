use std::collections::BTreeMap;

use ciborium::value::{Integer, Value};

use super::{Wire, uint};
use crate::WireError;

/// A map key: envelopes use small integers, payloads use text (sw-wire contract §1).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    Int(u64),
    Text(String),
}

impl Key {
    fn to_value(&self) -> Value {
        match self {
            Self::Int(k) => Value::Integer(Integer::from(*k)),
            Self::Text(k) => Value::Text(k.clone()),
        }
    }
}

impl From<u64> for Key {
    fn from(k: u64) -> Self {
        Self::Int(k)
    }
}

impl From<&str> for Key {
    fn from(k: &str) -> Self {
        Self::Text(k.to_owned())
    }
}

/// A map field: required for `T: Wire`, absent-when-`None` for `Option<T>`.
pub trait Field: Sized {
    fn put(&self, key: Key, out: &mut MapOut);
    fn take(key: Key, map: &mut MapIn) -> Result<Self, WireError>;
}

impl<T: Wire> Field for T {
    fn put(&self, key: Key, out: &mut MapOut) {
        out.0.push((key.to_value(), self.to_value()));
    }
    fn take(key: Key, map: &mut MapIn) -> Result<Self, WireError> {
        match map.0.remove(&key) {
            Some(v) => T::from_value(v),
            None => Err(WireError::MissingKey(key)),
        }
    }
}

impl<T: Wire> Field for Option<T> {
    fn put(&self, key: Key, out: &mut MapOut) {
        if let Some(v) = self {
            v.put(key, out);
        }
    }
    fn take(key: Key, map: &mut MapIn) -> Result<Self, WireError> {
        map.0.remove(&key).map(T::from_value).transpose()
    }
}

#[derive(Debug, Default)]
pub struct MapOut(Vec<(Value, Value)>);

impl MapOut {
    pub fn field<F: Field>(&mut self, key: impl Into<Key>, value: &F) {
        value.put(key.into(), self);
    }
    pub fn build(self) -> Value {
        Value::Map(self.0)
    }
}

#[derive(Debug)]
pub struct MapIn(BTreeMap<Key, Value>);

impl MapIn {
    pub fn new(value: Value) -> Result<Self, WireError> {
        let Value::Map(entries) = value else {
            return Err(WireError::Type("map"));
        };
        let mut map = BTreeMap::new();
        for (k, v) in entries {
            let key = match k {
                Value::Text(t) => Key::Text(t),
                other => Key::Int(uint(other)?),
            };
            if map.insert(key, v).is_some() {
                return Err(WireError::DuplicateKey);
            }
        }
        Ok(Self(map))
    }
    pub fn field<F: Field>(&mut self, key: impl Into<Key>) -> Result<F, WireError> {
        F::take(key.into(), self)
    }
    /// Refuses any key the schema did not read.
    pub fn finish(self) -> Result<(), WireError> {
        match self.0.into_keys().next() {
            Some(k) => Err(WireError::UnknownKey(k)),
            None => Ok(()),
        }
    }
}
