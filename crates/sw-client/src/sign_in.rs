//! The start of "Sign in with Citadel" (frozen contract `sign-in-with-citadel.md` §1, §5): the
//! link to the issuer's page. It carries exactly `client`, `challenge`, `state` and `return`;
//! `challenge = SHA3-256(verifier)` and the verifier never leaves the client.

use data_encoding::BASE64URL_NOPAD;
use sw_crypto::sha3_256;

/// Who is signing in, where the issuer sends the person back, and which issuer: all three
/// come from the issuer's allowlist (`/sign-in/clients.json`), never from user input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignInClient<'a> {
    pub issuer: &'a str,
    pub client: &'a str,
    pub return_url: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignInError {
    /// `state` must be 16..=32 bytes (contract §1).
    StateLength,
}

/// `state` length bounds (contract §1 step 1).
pub const STATE_LEN: core::ops::RangeInclusive<usize> = 16..=32;

/// The issuer link for one attempt. `verifier` is 32 fresh random bytes the caller keeps
/// (memory or `sessionStorage`) until it completes the sign-in.
pub fn sign_in_link(
    who: &SignInClient<'_>,
    verifier: &[u8; 32],
    state: &[u8],
) -> Result<String, SignInError> {
    if !STATE_LEN.contains(&state.len()) {
        return Err(SignInError::StateLength);
    }
    Ok(format!(
        "https://{}/sign-in?client={}&challenge={}&state={}&return={}",
        who.issuer,
        who.client,
        BASE64URL_NOPAD.encode(&sha3_256(verifier)),
        BASE64URL_NOPAD.encode(state),
        percent_encode(who.return_url),
    ))
}

/// RFC 3986 percent-encoding of everything but the unreserved set.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
