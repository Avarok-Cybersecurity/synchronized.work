//! The synchronized.work wire, conforming to the frozen contract
//! `docs/contracts/batch-1116/sw-wire.md` (C0, 2026-11-16). Bytes in, bytes out: no Citadel,
//! no I/O.
//!
//! ```text
//! SwMessage {1: typ, 2: payload}                unsigned: reads, presence, node data
//! SwMessage {1: typ, 3: Signed}                 everything that writes
//! Signed    {1: "ML-DSA-65", 2: kid, 3: Body bytes, 4: sig}
//! Body      {1: typ, 2: iss, 3: from, 4: aud, 5: iat, 6: exp, 7: nonce, 8: seq, 9: payload}
//! Sealed    {1: "ML-KEM-1024", 2: rcpt, 3: ct, 4: "AES-256-GCM", 5: n, 6: enc}
//! payloads  text-keyed maps
//! ```

pub mod body;
pub mod cbor;
mod error;
pub mod gate;
pub mod invite;
pub mod link_code;
mod model_wire;
pub mod p2p;
pub mod pad;
pub mod payload;
pub mod ports;
pub mod record;
pub mod replay;
pub mod sealed;
pub mod signed;
pub mod wire;

pub use error::WireError;
