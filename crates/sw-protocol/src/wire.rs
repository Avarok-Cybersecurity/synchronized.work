//! Typed values to and from CBOR maps with integer keys. Unknown keys are refused
//! (the wire's `deny_unknown_fields`), optional fields are absent rather than `null`.

use std::collections::BTreeMap;

use ciborium::value::{Integer, Value};

use crate::WireError;

pub trait Wire: Sized {
    fn to_value(&self) -> Value;
    fn from_value(value: Value) -> Result<Self, WireError>;
}

/// A byte string (a plain `Vec<u8>` would be an array of integers).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Bytes(pub Vec<u8>);

impl core::fmt::Debug for Bytes {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Bytes({} bytes)", self.0.len())
    }
}

fn uint(value: Value) -> Result<u64, WireError> {
    match value {
        Value::Integer(i) => u64::try_from(i).map_err(|_| WireError::Type("unsigned integer")),
        _ => Err(WireError::Type("unsigned integer")),
    }
}

macro_rules! wire_uint {
    ($($t:ty),*) => {$(
        impl Wire for $t {
            fn to_value(&self) -> Value {
                Value::Integer(Integer::from(*self))
            }
            fn from_value(value: Value) -> Result<Self, WireError> {
                <$t>::try_from(uint(value)?).map_err(|_| WireError::Range(stringify!($t)))
            }
        }
    )*};
}
wire_uint!(u8, u16, u32, u64);

impl Wire for i32 {
    fn to_value(&self) -> Value {
        Value::Integer(Integer::from(*self))
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        match value {
            Value::Integer(i) => i32::try_from(i).map_err(|_| WireError::Range("i32")),
            _ => Err(WireError::Type("integer")),
        }
    }
}

impl Wire for bool {
    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        match value {
            Value::Bool(b) => Ok(b),
            _ => Err(WireError::Type("bool")),
        }
    }
}

impl Wire for String {
    fn to_value(&self) -> Value {
        Value::Text(self.clone())
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        match value {
            Value::Text(t) => Ok(t),
            _ => Err(WireError::Type("text")),
        }
    }
}

impl Wire for Bytes {
    fn to_value(&self) -> Value {
        Value::Bytes(self.0.clone())
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        match value {
            Value::Bytes(b) => Ok(Bytes(b)),
            _ => Err(WireError::Type("bytes")),
        }
    }
}

impl<const N: usize> Wire for [u8; N] {
    fn to_value(&self) -> Value {
        Value::Bytes(self.to_vec())
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        let Bytes(b) = Bytes::from_value(value)?;
        b.try_into()
            .map_err(|_| WireError::Length("fixed-size bytes"))
    }
}

impl<T: Wire> Wire for Vec<T> {
    fn to_value(&self) -> Value {
        Value::Array(self.iter().map(Wire::to_value).collect())
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        match value {
            Value::Array(items) => items.into_iter().map(T::from_value).collect(),
            _ => Err(WireError::Type("array")),
        }
    }
}

/// A map field: required for `T: Wire`, absent-when-`None` for `Option<T>`.
pub trait Field: Sized {
    fn put(&self, key: u64, out: &mut MapOut);
    fn take(key: u64, map: &mut MapIn) -> Result<Self, WireError>;
}

impl<T: Wire> Field for T {
    fn put(&self, key: u64, out: &mut MapOut) {
        out.0
            .push((Value::Integer(Integer::from(key)), self.to_value()));
    }
    fn take(key: u64, map: &mut MapIn) -> Result<Self, WireError> {
        T::from_value(map.0.remove(&key).ok_or(WireError::MissingKey(key))?)
    }
}

impl<T: Wire> Field for Option<T> {
    fn put(&self, key: u64, out: &mut MapOut) {
        if let Some(v) = self {
            v.put(key, out);
        }
    }
    fn take(key: u64, map: &mut MapIn) -> Result<Self, WireError> {
        map.0.remove(&key).map(T::from_value).transpose()
    }
}

#[derive(Debug, Default)]
pub struct MapOut(Vec<(Value, Value)>);

impl MapOut {
    pub fn field<F: Field>(&mut self, key: u64, value: &F) {
        value.put(key, self);
    }
    pub fn build(self) -> Value {
        Value::Map(self.0)
    }
}

#[derive(Debug)]
pub struct MapIn(BTreeMap<u64, Value>);

impl MapIn {
    pub fn new(value: Value) -> Result<Self, WireError> {
        let Value::Map(entries) = value else {
            return Err(WireError::Type("map"));
        };
        let mut map = BTreeMap::new();
        for (k, v) in entries {
            if map.insert(uint(k)?, v).is_some() {
                return Err(WireError::DuplicateKey);
            }
        }
        Ok(Self(map))
    }
    pub fn field<F: Field>(&mut self, key: u64) -> Result<F, WireError> {
        F::take(key, self)
    }
    /// Refuses any key the schema did not read.
    pub fn finish(self) -> Result<(), WireError> {
        match self.0.keys().next() {
            Some(k) => Err(WireError::UnknownKey(*k)),
            None => Ok(()),
        }
    }
}

/// Declares a struct carried as a CBOR map with the given integer keys.
macro_rules! wire_map {
    ($(#[$m:meta])* pub struct $name:ident { $($(#[$fm:meta])* $k:literal => $f:ident : $t:ty),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name { $($(#[$fm])* pub $f: $t),* }
        impl $crate::wire::Wire for $name {
            fn to_value(&self) -> ciborium::value::Value {
                let mut out = $crate::wire::MapOut::default();
                $( out.field($k, &self.$f); )*
                out.build()
            }
            fn from_value(value: ciborium::value::Value) -> Result<Self, $crate::WireError> {
                let mut map = $crate::wire::MapIn::new(value)?;
                let out = Self { $($f: map.field($k)?),* };
                map.finish()?;
                Ok(out)
            }
        }
    };
}
pub(crate) use wire_map;

/// An already-decoded CBOR item carried through unchanged (a sealed inner payload, whose
/// type the outer `typ` decides).
#[derive(Debug, Clone, PartialEq)]
pub struct Raw(pub Value);

impl Wire for Raw {
    fn to_value(&self) -> Value {
        self.0.clone()
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        Ok(Raw(value))
    }
}
