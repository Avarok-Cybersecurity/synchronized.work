//! The P2P envelope (design §4.8): a 2-entry CBOR map `{"type": "sw/1", "payload": bytes}`
//! whose payload is an ordinary signed `SwMessage`, so the receiver runs the same gate.

use ciborium::value::Value;

use crate::{WireError, cbor};

pub const P2P_TYPE: &str = "sw/1";

pub fn encode_p2p(sw_message: &[u8]) -> Result<Vec<u8>, WireError> {
    cbor::encode(&Value::Map(vec![
        (Value::Text("type".into()), Value::Text(P2P_TYPE.into())),
        (
            Value::Text("payload".into()),
            Value::Bytes(sw_message.to_vec()),
        ),
    ]))
}

/// The inner `SwMessage` bytes, refusing any other shape or type tag.
pub fn decode_p2p(bytes: &[u8]) -> Result<Vec<u8>, WireError> {
    let Value::Map(entries) = cbor::decode(bytes)? else {
        return Err(WireError::Type("map"));
    };
    match entries.as_slice() {
        [
            (Value::Text(k1), Value::Text(t)),
            (Value::Text(k2), Value::Bytes(p)),
        ] if k1 == "type" && k2 == "payload" => {
            if t == P2P_TYPE {
                Ok(p.clone())
            } else {
                Err(WireError::Range("p2p type"))
            }
        }
        _ => Err(WireError::Shape("p2p envelope is {type, payload}")),
    }
}
