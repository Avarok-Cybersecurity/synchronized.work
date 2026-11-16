//! The synchronized.work wire (design rev 3 §4). Bytes in, bytes out: no Citadel, no I/O.
//!
//! ```text
//! SwMessage {0: 1, 1: typ, 2: payload}          unsigned: reads, presence, node data
//! SwMessage {0: 1, 3: Signed}                   everything that writes
//! Signed    {0: "ML-DSA-65", 1: kid, 2: Body bytes, 3: sig}
//! Body      {0: typ, 1: iss, 2: from, 3: aud, 4: iat, 5: exp, 6: nonce, 7: seq, 8: payload}
//! Sealed    {0: "ML-KEM-768", 1: rcpt, 2: ct, 3: "AES-256-GCM", 4: n, 5: enc}
//! ```

pub mod body;
pub mod cbor;
mod error;
pub mod gate;
mod model_wire;
pub mod p2p;
pub mod payload;
pub mod ports;
pub mod replay;
pub mod sealed;
pub mod signed;
pub mod wire;

pub use error::WireError;
