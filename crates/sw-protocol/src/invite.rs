//! The invite a project owner sends a person out of band, and the one hash the node keeps of
//! it. The code carries the project, its owner and name (so the person sees what they are
//! joining) and a one-time token; nothing in it is secret beyond the token, which the node
//! stores only as [`token_hash`] and forgets when it is spent or expires.
//!
//! ```text
//! swi1.<base64url(CBOR {project, owner, name, exp, token})>
//! ```

use data_encoding::BASE64URL_NOPAD;
use sw_model::IdFp;

use crate::WireError;
use crate::cbor;
use crate::payload::limits::TASK_TITLE_BYTES;
use crate::wire::{Wire, wire_text};

/// The code's prefix, which names its format version.
pub const PREFIX: &str = "swi1.";

/// Domain separation for the token's hash.
const TOKEN_DOMAIN: &[u8] = b"sw-invite-v1\0";

/// What the node stores and compares: `SHA3-256("sw-invite-v1\0" || token)`.
pub fn token_hash(token: &[u8; 32]) -> [u8; 32] {
    let mut input = TOKEN_DOMAIN.to_vec();
    input.extend_from_slice(token);
    sw_crypto::sha3_256(&input)
}

wire_text! {
    /// The decoded invite code.
    pub struct InviteCode {
        project: String,
        owner: IdFp,
        name: String,
        exp: u64,
        token: [u8; 32],
    }
    check = InviteCode::check;
}

impl InviteCode {
    fn check(&self) -> Result<(), WireError> {
        let ok = !self.project.is_empty()
            && self.project.len() <= TASK_TITLE_BYTES
            && self.name.len() <= TASK_TITLE_BYTES;
        if ok {
            Ok(())
        } else {
            Err(WireError::Length("InviteCode project or name"))
        }
    }

    pub fn encode(&self) -> Result<String, WireError> {
        let bytes = cbor::encode(&self.to_value())?;
        Ok(format!("{PREFIX}{}", BASE64URL_NOPAD.encode(&bytes)))
    }

    /// Parses a pasted code; surrounding whitespace is ignored.
    pub fn decode(text: &str) -> Result<Self, WireError> {
        let body = text
            .trim()
            .strip_prefix(PREFIX)
            .ok_or(WireError::Shape("not an swi1. invite code"))?;
        let bytes = BASE64URL_NOPAD
            .decode(body.as_bytes())
            .map_err(|_| WireError::Malformed)?;
        Self::from_value(cbor::decode(&bytes)?)
    }

    pub fn token_hash(&self) -> [u8; 32] {
        token_hash(&self.token)
    }
}
