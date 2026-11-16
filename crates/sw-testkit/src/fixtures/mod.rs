//! The conformance fixtures (design §11.1, plan §5.5): one canonical `SwMessage` per message
//! type, built deterministically from [`Cast`]. `xtask gen-fixtures` writes them; every
//! party's decoder must reproduce them byte for byte.

mod controller;
mod device;
mod plain;

use sw_model::{Cid, Id128, IdFp};
use sw_protocol::body::{Audience, Body};
use sw_protocol::payload::Payload;
use sw_protocol::ports::MessageSigner;
use sw_protocol::signed::{Signed, SwMessage};

use crate::{Cast, FIXTURE_NOW_MS, LabelledEntropy};

pub(crate) const TASK: Id128 = Id128([0xa1; 16]);
pub(crate) const AGENT: Id128 = Id128([0xb2; 16]);
pub(crate) const PROJECT: Id128 = Id128([0xc3; 16]);

pub(crate) type Fixture = (&'static str, Vec<u8>);

/// Signs with one key as one Person from one Endpoint, drawing nonces from one stream.
pub(crate) struct Issuer<'a> {
    pub key: &'a dyn MessageSigner,
    pub iss: IdFp,
    pub from: Cid,
}

impl Issuer<'_> {
    pub fn signed(
        &self,
        aud: Audience,
        seq: u64,
        payload: Payload,
        e: &mut LabelledEntropy,
    ) -> Vec<u8> {
        let body = Body {
            iss: self.iss.clone(),
            from: self.from,
            aud,
            iat: FIXTURE_NOW_MS,
            exp: FIXTURE_NOW_MS + payload.max_lifetime_ms(),
            nonce: e.bytes(),
            seq,
            payload,
        };
        let signed = Signed::sign(&body, self.key).expect("fixture body encodes");
        SwMessage::Signed(signed)
            .to_bytes()
            .expect("fixture message encodes")
    }
}

pub(crate) fn unsigned(payload: Payload) -> Vec<u8> {
    SwMessage::Unsigned(payload)
        .to_bytes()
        .expect("fixture message encodes")
}

/// Every fixture as `(file name, bytes)`, sorted by name. One entropy stream in a fixed
/// order, so the bytes never change unless a builder does.
pub fn all() -> Vec<Fixture> {
    let cast = Cast::new();
    let mut e = LabelledEntropy::new("sw-fixtures-v1");
    let mut out = Vec::new();
    out.extend(device::fixtures(&cast, &mut e));
    out.extend(controller::fixtures(&cast, &mut e));
    out.extend(plain::fixtures(&cast, &mut e));
    out.sort_by(|a, b| a.0.cmp(b.0));
    out
}
