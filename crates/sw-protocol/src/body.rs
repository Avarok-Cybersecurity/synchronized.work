//! `Body`: what a `Signed` envelope signs (sw-wire contract §2).

use ciborium::value::Value;
use sw_model::{Cid, IdFp};

use crate::payload::Payload;
use crate::wire::{MapIn, MapOut, Raw, Wire};
use crate::{WireError, cbor};

/// The node's audience string; every other audience is an Endpoint's CID.
pub const NODE_AUDIENCE: &str = "synchronized.work";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    Node,
    /// A Device (4, 5) or a Controller (6, 17).
    Endpoint(Cid),
}

impl Wire for Audience {
    fn to_value(&self) -> Value {
        match self {
            Self::Node => Value::Text(NODE_AUDIENCE.to_owned()),
            Self::Endpoint(cid) => cid.to_value(),
        }
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        match value {
            Value::Text(t) if t == NODE_AUDIENCE => Ok(Self::Node),
            Value::Text(_) => Err(WireError::Range("audience")),
            other => Ok(Self::Endpoint(u64::from_value(other)?)),
        }
    }
}

/// `{1: typ, 2: iss, 3: from, 4: aud, 5: iat, 6: exp, 7: nonce, 8: seq, 9: payload}`.
/// `typ` is not stored: it is derived from the payload, so the two cannot disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body {
    /// The issuing Person's `id_fp`.
    pub iss: IdFp,
    /// The sending Endpoint's CID.
    pub from: Cid,
    pub aud: Audience,
    pub iat: u64,
    pub exp: u64,
    pub nonce: [u8; 16],
    /// Per `kid`, strictly increasing.
    pub seq: u64,
    pub payload: Payload,
}

impl Body {
    pub fn to_bytes(&self) -> Result<Vec<u8>, WireError> {
        let mut out = MapOut::default();
        out.field(1u64, &self.payload.typ());
        out.field(2u64, &self.iss);
        out.field(3u64, &self.from);
        out.field(4u64, &self.aud);
        out.field(5u64, &self.iat);
        out.field(6u64, &self.exp);
        out.field(7u64, &self.nonce);
        out.field(8u64, &self.seq);
        out.field(9u64, &Raw(self.payload.to_value()));
        cbor::encode(&out.build())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WireError> {
        let mut map = MapIn::new(cbor::decode(bytes)?)?;
        let typ: u8 = map.field(1u64)?;
        let body = Self {
            iss: map.field(2u64)?,
            from: map.field(3u64)?,
            aud: map.field(4u64)?,
            iat: map.field(5u64)?,
            exp: map.field(6u64)?,
            nonce: map.field(7u64)?,
            seq: map.field(8u64)?,
            payload: Payload::from_value(typ, map.field::<Raw>(9u64)?.0)?,
        };
        map.finish()?;
        Ok(body)
    }
}
