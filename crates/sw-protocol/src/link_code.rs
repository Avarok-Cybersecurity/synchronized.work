//! The link code (design §5.3 "ships first"): eight characters a signed-in Endpoint of a Person
//! mints with `LINK_CODE` and a Citadel App types into `LINK_REDEEM`, valid ten minutes, once.
//! The wire carries the code itself (`LINK{op: "code", code}`), so the minting Endpoint draws
//! it; the node keeps only [`code_hash`] of the canonical form and compares that.
//!
//! The alphabet is Crockford's base32: no `I`, `L`, `O` or `U`, so a code read aloud or off a
//! screen survives; what a person types is folded to it by [`canonical`].

use sw_crypto::Entropy;

use crate::payload::Link;

/// 32 symbols: one random byte's low five bits pick one with no bias (256 is a multiple of 32).
pub const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Rules of the code itself (protocol rules, not tunables).
pub mod link_limits {
    /// "10 min, one use" (§5.3).
    pub const LIFETIME_MS: u64 = 10 * 60 * 1000;
    /// Unspent, unexpired codes one Person may hold at once.
    pub const LIVE_PER_PERSON: usize = 4;
    /// How long a spent or expired code is remembered, so a second try is told which it was.
    pub const KEPT_AFTER_EXPIRY_MS: u64 = LIFETIME_MS;
}

/// Domain separation for the stored hash.
const HASH_DOMAIN: &[u8] = b"sw-link-code-v1\0";

/// A fresh code from `entropy`.
pub fn generate(entropy: &mut dyn Entropy) -> String {
    let mut bytes = [0u8; Link::CODE_LEN];
    entropy.fill(&mut bytes);
    bytes
        .iter()
        .map(|b| char::from(ALPHABET[usize::from(b & 0x1F)]))
        .collect()
}

/// The canonical form of a typed code: upper case, `O` read as `0`, `I` and `L` as `1`;
/// `None` unless that is exactly [`Link::CODE_LEN`] symbols of [`ALPHABET`].
pub fn canonical(typed: &str) -> Option<String> {
    let code: String = typed
        .chars()
        .map(|c| match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        })
        .collect();
    let ok = code.chars().count() == Link::CODE_LEN && code.bytes().all(|b| ALPHABET.contains(&b));
    ok.then_some(code)
}

/// What the node stores and compares: `SHA3-256("sw-link-code-v1\0" || canonical code)`.
pub fn code_hash(canonical: &str) -> [u8; 32] {
    let mut input = HASH_DOMAIN.to_vec();
    input.extend_from_slice(canonical.as_bytes());
    sw_crypto::sha3_256(&input)
}
