//! Wire forms of the `sw-model` types that travel as they are.

use ciborium::value::Value;
use sw_model::{EndpointKind, IdFp};

use crate::WireError;
use crate::wire::{Wire, wire_str};

wire_str!(EndpointKind { "device" => EndpointKind::Device, "controller" => EndpointKind::Controller });

/// `tstr(16)`: the issuing Person's `id_fp`.
impl Wire for IdFp {
    fn to_value(&self) -> Value {
        Value::Text(self.as_str().to_owned())
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        IdFp::parse(&String::from_value(value)?).map_err(|_| WireError::Range("id_fp"))
    }
}
