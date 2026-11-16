//! `Signed {1: alg, 2: kid, 3: body, 4: sig}` and the `SwMessage` frame
//! `{1: typ, 2: unsigned payload | 3: Signed}` (sw-wire contract §2).

use ciborium::value::Value;
use sw_crypto::{Kid, kid_of, sha256};

use crate::body::Body;
use crate::payload::Payload;
use crate::ports::MessageSigner;
use crate::wire::{Bytes, MapIn, MapOut, Raw, Wire, wire_map};
use crate::{WireError, cbor};

pub const SIGN_ALG: &str = "ML-DSA-65";
/// Domain separation: `"sw-signed-v1" ‖ 0x00 ‖ body bytes`.
pub const SIGN_DOMAIN: &[u8] = b"sw-signed-v1\0";

wire_map! {
    pub struct Signed {
        1 => alg: String,
        /// `SHA-256` of the signer's verifying key.
        2 => kid: Kid,
        /// The canonical CBOR of the [`Body`], exactly as signed.
        3 => body: Bytes,
        4 => sig: Bytes,
    }
}

/// The bytes a signature covers.
pub fn signed_message(body_bytes: &[u8]) -> Vec<u8> {
    [SIGN_DOMAIN, body_bytes].concat()
}

impl Signed {
    pub fn sign(body: &Body, signer: &dyn MessageSigner) -> Result<Self, WireError> {
        let body_bytes = body.to_bytes()?;
        let sig = signer.sign(&signed_message(&body_bytes));
        Ok(Self {
            alg: SIGN_ALG.to_owned(),
            kid: kid_of(signer.public_key()),
            body: Bytes(body_bytes),
            sig: Bytes(sig),
        })
    }

    /// `signal_id = SHA-256(Signed bytes)`: the canonical encoding of this map.
    pub fn signal_id(&self) -> Result<[u8; 32], WireError> {
        Ok(sha256(&cbor::encode(&self.to_value())?))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SwMessage {
    /// Reads, presence, node-originated data, `LINK_REDEEM`: slot 2.
    Unsigned(Payload),
    /// Everything that writes: slot 3. `typ` is carried beside the envelope so a relay can
    /// route without opening it; the gate refuses it unless it equals `Body[1]`.
    Signed { typ: u8, signed: Signed },
}

impl SwMessage {
    /// A signed message whose outer `typ` is taken from the body it carries.
    pub fn signed(body: &Body, signer: &dyn MessageSigner) -> Result<Self, WireError> {
        Ok(Self::Signed {
            typ: body.payload.typ(),
            signed: Signed::sign(body, signer)?,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, WireError> {
        let mut out = MapOut::default();
        match self {
            Self::Unsigned(p) => {
                out.field(1u64, &p.typ());
                out.field(2u64, &Raw(p.to_value()));
            }
            Self::Signed { typ, signed } => {
                out.field(1u64, typ);
                out.field(3u64, signed);
            }
        }
        cbor::encode(&out.build())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WireError> {
        Self::from_value(cbor::decode(bytes)?)
    }

    fn from_value(value: Value) -> Result<Self, WireError> {
        let mut map = MapIn::new(value)?;
        let typ: u8 = map.field(1u64)?;
        let payload: Option<Raw> = map.field(2u64)?;
        let signed: Option<Signed> = map.field(3u64)?;
        map.finish()?;
        match (payload, signed) {
            (Some(Raw(p)), None) => Ok(Self::Unsigned(Payload::from_value(typ, p)?)),
            (None, Some(signed)) => Ok(Self::Signed { typ, signed }),
            _ => Err(WireError::Shape(
                "SwMessage holds exactly one of slot 2 and slot 3",
            )),
        }
    }
}
