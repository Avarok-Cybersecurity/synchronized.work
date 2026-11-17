//! A stand-in for `citadel-identity`'s sign-in assertion (identity-api §7), checked byte for
//! byte against `fixtures/sign-in-kat.json`. Replace with the real crate's adapter.
//!
//! ```text
//! body = str16(aud) ‖ str16(device) ‖ u64be(exp_s) ‖ nonce[32] ‖ identity_pk[1952]
//! sig  = Sign(sk_issuer, "citadel-sign-in-assertion-v1" ‖ 0x00 ‖ body)
//! blob = 0x01 ‖ body ‖ sig[3309]
//! ```

use sw_crypto::{SIGN_PUBLIC_KEY_LEN, SIGNATURE_LEN, SigningKey, verify_signature};
use sw_protocol::ports::{IdentityError, SignInClaims};

const DOMAIN: &[u8] = b"citadel-sign-in-assertion-v1";
const VERSION: u8 = 0x01;

fn str16(out: &mut Vec<u8>, text: &str) {
    let len = u16::try_from(text.len()).expect("assertion fields are at most 255 bytes");
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(text.as_bytes());
}

fn message(body: &[u8]) -> Vec<u8> {
    [DOMAIN, &[0], body].concat()
}

/// Mints an assertion as work.avarok.net would.
pub fn sign_assertion(issuer: &SigningKey, claims: &SignInClaims, nonce: &[u8; 32]) -> Vec<u8> {
    let mut body = Vec::new();
    str16(&mut body, &claims.aud);
    str16(&mut body, &claims.device);
    body.extend_from_slice(&claims.exp_s.to_be_bytes());
    body.extend_from_slice(nonce);
    body.extend_from_slice(&claims.identity);
    let sig = issuer.sign(&message(&body));
    [&[VERSION][..], &body, &sig[..]].concat()
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], IdentityError> {
        if self.0.len() < n {
            return Err(IdentityError::Malformed);
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }
    fn str16(&mut self) -> Result<String, IdentityError> {
        let len = u16::from_be_bytes(self.take(2)?.try_into().expect("2 bytes"));
        let text = self.take(usize::from(len))?;
        if text.is_empty() || text.len() > 255 {
            return Err(IdentityError::Malformed);
        }
        String::from_utf8(text.to_vec()).map_err(|_| IdentityError::Malformed)
    }
}

pub(crate) fn open_assertion(blob: &[u8], issuer: &[u8]) -> Result<SignInClaims, IdentityError> {
    let mut r = Reader(blob);
    if r.take(1)? != [VERSION] {
        return Err(IdentityError::Malformed);
    }
    let aud = r.str16()?;
    let device = r.str16()?;
    let exp_s = u64::from_be_bytes(r.take(8)?.try_into().expect("8 bytes"));
    r.take(32)?;
    let identity = r.take(SIGN_PUBLIC_KEY_LEN)?.to_vec();
    let sig = r.take(SIGNATURE_LEN)?;
    if !r.0.is_empty() {
        return Err(IdentityError::Malformed);
    }
    let body = &blob[1..blob.len() - SIGNATURE_LEN];
    verify_signature(issuer, &message(body), sig).map_err(|_| IdentityError::BadSignature)?;
    Ok(SignInClaims {
        identity,
        device,
        aud,
        exp_s,
    })
}
