//! `Body`: what a `Signed` envelope signs (design §4.2).

use ciborium::value::Value;
use sw_model::{Cid, IdFp};

use crate::payload::Payload;
use crate::wire::{MapIn, MapOut, Raw, Wire};
use crate::{WireError, cbor};

/// The node's audience string; every other audience is a Device's CID.
pub const NODE_AUDIENCE: &str = "synchronized.work";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    Node,
    Device(Cid),
}

impl Wire for Audience {
    fn to_value(&self) -> Value {
        match self {
            Self::Node => Value::Text(NODE_AUDIENCE.to_owned()),
            Self::Device(cid) => cid.to_value(),
        }
    }
    fn from_value(value: Value) -> Result<Self, WireError> {
        match value {
            Value::Text(t) if t == NODE_AUDIENCE => Ok(Self::Node),
            Value::Text(_) => Err(WireError::Range("audience")),
            other => Ok(Self::Device(u64::from_value(other)?)),
        }
    }
}

/// `typ` is not stored: it is derived from the payload, so the two cannot disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body {
    /// The issuing Person's `id_fp` (rev 3: the portable identity).
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
        out.field(0, &self.payload.typ());
        out.field(1, &self.iss);
        out.field(2, &self.from);
        out.field(3, &self.aud);
        out.field(4, &self.iat);
        out.field(5, &self.exp);
        out.field(6, &self.nonce);
        out.field(7, &self.seq);
        out.field(8, &Raw(self.payload.to_value()));
        cbor::encode(&out.build())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WireError> {
        let mut map = MapIn::new(cbor::decode(bytes)?)?;
        let typ: u8 = map.field(0)?;
        let body = Self {
            iss: map.field(1)?,
            from: map.field(2)?,
            aud: map.field(3)?,
            iat: map.field(4)?,
            exp: map.field(5)?,
            nonce: map.field(6)?,
            seq: map.field(7)?,
            payload: Payload::from_value(typ, map.field::<Raw>(8)?.0)?,
        };
        map.finish()?;
        Ok(body)
    }
}
