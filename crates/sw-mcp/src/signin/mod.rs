//! "Sign in with Citadel", MCP variant (owner decision D-B; `sign-in-with-citadel.md` §1, §3,
//! §4). Pure: the verifier and state arrive from the caller's entropy, the clock as `now_s`.
//!
//! 1. `start`: `challenge = SHA3-256(verifier)`; the link carries exactly `client`,
//!    `challenge`, `state` and `return`, plus the three match words for the terminal.
//! 2. The person signs in on work.avarok.net and is shown a one-time code `XXXX-XXXX`.
//! 3. `complete_request`: `{v, client, code, verifier, device}` for `/sign-in/complete`.
//! 4. `accept`: the response's assertion must be the pinned issuer's, for `sw-mcp`, answering
//!    this challenge, unexpired, and naming this Device's signing key.

mod flow;
mod http;

pub use flow::{
    IssuerPort, IssuerReply, SignInError, complete_sign_in, load_assertion, pin_issuer,
};
pub use http::HttpIssuer;

use citadel_identity::{AssertionError, MlDsa65Pk, match_phrase, verify_assertion};
use data_encoding::BASE64URL_NOPAD;
use serde::{Deserialize, Serialize};
use sw_crypto::sha3_256;
use zeroize::Zeroizing;

/// This client's id in the issuer's allowlist; also the assertion's audience.
pub const CLIENT: &str = "sw-mcp";
/// The code alphabet: 32 symbols, no `I L O U` (contract §4).
pub const CODE_ALPHABET: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// A sign-in in progress. The verifier lives only here, in memory.
pub struct PendingSignIn {
    verifier: Zeroizing<[u8; 32]>,
    pub challenge: [u8; 32],
    pub state: [u8; 16],
}

impl std::fmt::Debug for PendingSignIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingSignIn")
            .field("challenge", &BASE64URL_NOPAD.encode(&self.challenge))
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignInLink {
    pub url: String,
    /// The three words the page shows for the same challenge (contract §3).
    pub match_words: [&'static str; 3],
}

/// RFC 3986 unreserved characters pass; everything else is percent-encoded.
fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The only return the allowlist lists for `sw-mcp`: the issuer's code page (contract §2).
pub fn return_url(issuer: &str) -> String {
    format!("https://{issuer}/sign-in/code")
}

pub fn start(verifier: [u8; 32], state: [u8; 16], issuer: &str) -> (PendingSignIn, SignInLink) {
    let challenge = sha3_256(&verifier);
    let url = format!(
        "https://{issuer}/sign-in?client={CLIENT}&challenge={}&state={}&return={}",
        BASE64URL_NOPAD.encode(&challenge),
        BASE64URL_NOPAD.encode(&state),
        percent_encode(&return_url(issuer)),
    );
    let pending = PendingSignIn {
        verifier: Zeroizing::new(verifier),
        challenge,
        state,
    };
    let link = SignInLink {
        url,
        match_words: match_phrase(&challenge),
    };
    (pending, link)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeError {
    Length,
    /// A symbol outside the alphabet (`I`, `L`, `O`, `U`, punctuation, …).
    Symbol(char),
}

/// Accepts what a person pastes: case-insensitive, dash optional, surrounding space ignored.
/// Answers the wire form, `XXXX-XXXX`.
pub fn normalize_code(input: &str) -> Result<String, CodeError> {
    let mut symbols = String::with_capacity(8);
    for (i, c) in input.trim().chars().enumerate() {
        if c == '-' && i == 4 && symbols.len() == 4 {
            continue;
        }
        let up = c.to_ascii_uppercase();
        if !CODE_ALPHABET.contains(up) {
            return Err(CodeError::Symbol(c));
        }
        symbols.push(up);
    }
    if symbols.len() != 8 {
        return Err(CodeError::Length);
    }
    Ok(format!("{}-{}", &symbols[..4], &symbols[4..]))
}

/// `SignInCompleteRequest` (schema): exactly these five members.
#[derive(Debug, Serialize)]
pub struct CompleteRequest {
    pub v: u8,
    pub client: &'static str,
    pub code: String,
    pub verifier: String,
    pub device: String,
}

pub fn complete_request(
    pending: &PendingSignIn,
    code: &str,
    device: &str,
) -> Result<CompleteRequest, CodeError> {
    Ok(CompleteRequest {
        v: 1,
        client: CLIENT,
        code: normalize_code(code)?,
        verifier: BASE64URL_NOPAD.encode(pending.verifier.as_ref()),
        device: device.to_owned(),
    })
}

/// `SignInCompleteResponse` (schema): exactly these three members.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompleteResponse {
    pub v: u8,
    pub issuer: String,
    pub assertion: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptError {
    Response(String),
    WrongIssuer {
        expected: String,
        found: String,
    },
    Assertion(AssertionError),
    /// The assertion binds another key than this Device's.
    WrongDevice {
        expected: String,
        found: String,
    },
}

/// What the MCP keeps: the blob (for `ENROL` by assertion) and the fields it uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedAssertion {
    pub blob: Vec<u8>,
    pub identity_pk: Vec<u8>,
    pub device: String,
    pub exp_s: u64,
}

/// Verifies a `/sign-in/complete` answer for `pending` (contract §1 step 5).
pub fn accept(
    pending: &PendingSignIn,
    body: &[u8],
    issuer: &str,
    issuer_pk: &MlDsa65Pk,
    device: &str,
    now_s: u64,
) -> Result<AcceptedAssertion, AcceptError> {
    let response: CompleteResponse =
        serde_json::from_slice(body).map_err(|e| AcceptError::Response(e.to_string()))?;
    if response.v != 1 {
        return Err(AcceptError::Response(format!("version {}", response.v)));
    }
    if response.issuer != issuer {
        return Err(AcceptError::WrongIssuer {
            expected: issuer.into(),
            found: response.issuer,
        });
    }
    let blob = BASE64URL_NOPAD
        .decode(response.assertion.as_bytes())
        .map_err(|e| AcceptError::Response(e.to_string()))?;
    accept_blob(pending, blob, issuer_pk, device, now_s)
}

/// The assertion checks alone: issuer signature, audience `sw-mcp`, nonce = this challenge,
/// expiry, and `device` = this Device's label.
pub fn accept_blob(
    pending: &PendingSignIn,
    blob: Vec<u8>,
    issuer_pk: &MlDsa65Pk,
    device: &str,
    now_s: u64,
) -> Result<AcceptedAssertion, AcceptError> {
    let a = verify_assertion(&blob, issuer_pk, CLIENT, &pending.challenge, now_s)
        .map_err(AcceptError::Assertion)?;
    if a.device != device {
        return Err(AcceptError::WrongDevice {
            expected: device.into(),
            found: a.device,
        });
    }
    Ok(AcceptedAssertion {
        identity_pk: a.identity.as_bytes().to_vec(),
        device: a.device,
        exp_s: a.exp_s,
        blob,
    })
}
