//! Typed values to and from CBOR maps with integer keys. Unknown keys are refused
//! (the wire's `deny_unknown_fields`), optional fields are absent rather than `null`.

use ciborium::value::{Integer, Value};

use crate::WireError;

mod macros;
mod map;

pub(crate) use macros::{wire_map, wire_str, wire_text};
pub use map::{Field, Key, MapIn, MapOut};

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

pub(crate) fn uint(value: Value) -> Result<u64, WireError> {
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

/// An already-decoded CBOR item carried through unchanged (a sealed inner payload, whose
/// type the outer `typ` decides).
#[derive(Debug, Clone, PartialEq)]
pub struct Raw(pub Value);

/// The decoder refuses floats, so a decoded `Raw` never holds a NaN and equality is total.
impl Eq for Raw {}

impl Raw {
    /// Refuses anything but a map (the contract's `fields: map`, `body: map`).
    pub fn expect_map(&self) -> Result<(), WireError> {
        match self.0 {
            Value::Map(_) => Ok(()),
            _ => Err(WireError::Type("map")),
        }
    }
}

impl Wire for Raw {
    fn to_value(&self) -> Value {
        self.0.clone()
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        Ok(Raw(value))
    }
}
