//! The two frames of the post-connect server identity challenge, as the workspace protocol
//! encodes them (`server-identity-challenge.md` §1): `WorkspaceProtocolPayload` as JSON, the
//! encoding a Citadel agent sends after every connect and reads the answer in.
//!
//! The types below mirror the workspace types variant for variant and field for field, with
//! the same serde derives (externally tagged enums, `deny_unknown_fields`, a `[u8; 32]`
//! nonce), so a frame is a challenge here exactly when it is one there. Only the shapes this
//! node takes part in are mirrored: every other frame is not a challenge and goes on to the
//! node's own wire. `fixtures/server-identity-challenge/frames.json`, produced with the
//! workspace types themselves, holds both encodings to these bytes.

use serde::{Deserialize, Serialize};

/// The nonce's length (`citadel_identity::NONCE_LEN`).
pub const NONCE_LEN: usize = citadel_identity::NONCE_LEN;

#[derive(Deserialize)]
enum RequestPayload {
    Request(Request),
}

#[derive(Deserialize)]
enum Request {
    ServerIdentityChallenge(Challenge),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Challenge {
    nonce: [u8; NONCE_LEN],
}

#[derive(Serialize)]
enum ResponsePayload {
    Response(Box<Response>),
}

#[derive(Serialize)]
enum Response {
    ServerIdentity(Answer),
}

#[derive(Serialize)]
struct Answer {
    domain: String,
    pk: Vec<u8>,
    sig: Vec<u8>,
}

/// The nonce of a `ServerIdentityChallenge` request, or `None` for any other frame.
pub fn challenge_nonce(frame: &[u8]) -> Option<[u8; NONCE_LEN]> {
    match serde_json::from_slice::<RequestPayload>(frame) {
        Ok(RequestPayload::Request(Request::ServerIdentityChallenge(c))) => Some(c.nonce),
        Err(_) => None,
    }
}

/// The `ServerIdentity` answer frame.
pub fn answer_frame(domain: &str, pk: &[u8], sig: &[u8]) -> Result<Vec<u8>, String> {
    let answer = ResponsePayload::Response(Box::new(Response::ServerIdentity(Answer {
        domain: domain.to_owned(),
        pk: pk.to_vec(),
        sig: sig.to_vec(),
    })));
    serde_json::to_vec(&answer).map_err(|e| e.to_string())
}
