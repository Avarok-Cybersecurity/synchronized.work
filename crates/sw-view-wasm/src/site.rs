//! The site's entry in the issuer's allowlist (`sign-in-with-citadel.md` §5; the allowlist's
//! `synchronized.work` client). One place: the link builder and the return route read these.

use sw_client::sign_in::{SignInClient, SignInError, sign_in_link};

pub const SIGN_IN: SignInClient<'static> = SignInClient {
    issuer: "work.avarok.net",
    client: "synchronized.work",
    return_url: "https://synchronized.work/auth/return",
};

/// The link for this browser's attempt; `verifier` and `state` come from web crypto and stay
/// in `sessionStorage` until the return.
pub fn link(verifier: &[u8; 32], state: &[u8]) -> Result<String, SignInError> {
    sign_in_link(&SIGN_IN, verifier, state)
}
