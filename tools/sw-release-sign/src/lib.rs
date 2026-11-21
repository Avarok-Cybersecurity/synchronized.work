//! The release signature of one sw-mcp asset (pure): ML-DSA-65 over
//!
//! ```text
//! "sw-mcp-release-v1\0" || tag || "\0" || file name || "\0" || SHA3-256(file bytes)
//! ```
//!
//! so a signature binds the asset to its name and to the release tag it was published under:
//! a valid asset renamed, or moved to another release, does not verify. Keys are files: the
//! private key is its 32-byte seed as 64 hex characters; the public key is the 1952-byte
//! verifying key as base64url, committed at `release/sw-mcp-release.pub`.

use data_encoding::{BASE64URL_NOPAD, HEXLOWER_PERMISSIVE};
use sw_crypto::{SIGN_PUBLIC_KEY_LEN, SIGN_SEED_LEN, SigningKey, sha3_256, verify_signature};
use zeroize::Zeroizing;

const DOMAIN: &[u8] = b"sw-mcp-release-v1\0";

/// What the committed public key file says until the owner commits a real key.
pub const PLACEHOLDER: &str = "PLACEHOLDER: no release key has been committed yet";

pub fn message(tag: &str, name: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let ok = |s: &str| !s.is_empty() && !s.contains('\0') && !s.contains('/');
    if !ok(tag) || !ok(name) {
        return Err(format!(
            "tag {tag:?} and name {name:?} must be plain, non-empty names"
        ));
    }
    let mut m = DOMAIN.to_vec();
    m.extend_from_slice(tag.as_bytes());
    m.push(0);
    m.extend_from_slice(name.as_bytes());
    m.push(0);
    m.extend_from_slice(&sha3_256(bytes));
    Ok(m)
}

pub fn private_key(text: &str) -> Result<SigningKey, String> {
    let seed = Zeroizing::new(
        HEXLOWER_PERMISSIVE
            .decode(text.trim().as_bytes())
            .map_err(|_| "the private key is 64 hex characters".to_owned())?,
    );
    let seed: &[u8; SIGN_SEED_LEN] = seed
        .as_slice()
        .try_into()
        .map_err(|_| "the private key is a 32-byte seed".to_owned())?;
    Ok(SigningKey::from_seed(seed))
}

pub fn public_key(text: &str) -> Result<Vec<u8>, String> {
    let text = text.trim();
    if text == PLACEHOLDER {
        return Err("the release public key is still the placeholder".into());
    }
    let pk = BASE64URL_NOPAD
        .decode(text.as_bytes())
        .map_err(|_| "the public key is base64url".to_owned())?;
    if pk.len() != SIGN_PUBLIC_KEY_LEN {
        return Err(format!("the public key is {SIGN_PUBLIC_KEY_LEN} bytes"));
    }
    Ok(pk)
}

/// The `.mldsa.sig` file's text: the signature as base64url and a newline.
pub fn sign(key: &SigningKey, tag: &str, name: &str, bytes: &[u8]) -> Result<String, String> {
    let sig = key.sign(&message(tag, name, bytes)?);
    Ok(format!("{}\n", BASE64URL_NOPAD.encode(&sig)))
}

pub fn verify(pk: &[u8], tag: &str, name: &str, bytes: &[u8], sig: &str) -> Result<(), String> {
    let sig = BASE64URL_NOPAD
        .decode(sig.trim().as_bytes())
        .map_err(|_| "the signature is base64url".to_owned())?;
    verify_signature(pk, &message(tag, name, bytes)?, &sig)
        .map_err(|e| format!("{name}: the signature does not verify ({e:?})"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_signature_binds_the_bytes_the_name_and_the_tag() {
        let key = SigningKey::from_seed(&[5; 32]);
        let pk = key.public_key().to_vec();
        let sig = sign(&key, "sw-mcp-v0.1.0", "a.tar.gz", b"bytes").unwrap();
        verify(&pk, "sw-mcp-v0.1.0", "a.tar.gz", b"bytes", &sig).unwrap();
        assert!(verify(&pk, "sw-mcp-v0.1.0", "a.tar.gz", b"bytez", &sig).is_err());
        assert!(verify(&pk, "sw-mcp-v0.1.0", "b.tar.gz", b"bytes", &sig).is_err());
        assert!(verify(&pk, "sw-mcp-v0.1.1", "a.tar.gz", b"bytes", &sig).is_err());
        let other = SigningKey::from_seed(&[6; 32]).public_key().to_vec();
        assert!(verify(&other, "sw-mcp-v0.1.0", "a.tar.gz", b"bytes", &sig).is_err());
    }

    #[test]
    fn keys_parse_and_the_placeholder_is_refused() {
        let key = private_key(&"05".repeat(32)).unwrap();
        let text = BASE64URL_NOPAD.encode(key.public_key());
        assert_eq!(public_key(&text).unwrap(), key.public_key().to_vec());
        assert!(public_key(PLACEHOLDER).is_err());
        assert!(public_key("AAAA").is_err());
        assert!(private_key("05").is_err());
        assert!(message("t/x", "n", b"").is_err());
    }
}
