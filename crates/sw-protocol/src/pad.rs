//! The scratchpad's sealed text ([`PadSealed`]): one random content key `K` per note or ack,
//! the text encrypted once under it, and `K` wrapped to each recipient Device with the
//! sw-wire [`Sealed`] envelope (ML-KEM-768, HKDF-SHA3-256, AES-256-GCM). The node relays
//! it unread; a Device not among `keys` cannot open it.

use sw_crypto::{AEAD_NONCE_LEN, Entropy, KemSecretKey, aead_open, aead_seal};
use sw_model::Cid;
use zeroize::Zeroizing;

use crate::payload::{ContentKey, KeyWrap, PadSealed, PadText};
use crate::sealed::{SealError, Sealed};
use crate::wire::{Bytes, Wire};
use crate::{WireError, cbor};

/// Associated data of every scratchpad text ciphertext.
pub const PAD_AAD: &[u8] = b"sw/pad/1";

/// A Device to seal to: its CID and its proof-verified ML-KEM encapsulation key.
#[derive(Debug, Clone, Copy)]
pub struct Recipient<'a> {
    pub cid: Cid,
    pub seal_pk: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PadOpenError {
    /// This Device is not among the recipients.
    NotForMe,
    /// The wrap opened to a key for another CID.
    WrongAudience,
    Seal(SealError),
    /// The text did not authenticate under the content key.
    OpenFailed,
    Wire(WireError),
}

/// Seals `text` to every recipient. Entropy order: `K`, the text nonce, then each wrap's.
pub fn seal_text(
    text: &PadText,
    to: &[Recipient<'_>],
    entropy: &mut dyn Entropy,
) -> Result<PadSealed, SealError> {
    let plaintext = cbor::encode(&text.to_value()).map_err(SealError::Wire)?;
    PadText::from_value(text.to_value()).map_err(SealError::Wire)?;
    let mut k = Zeroizing::new([0u8; 32]);
    entropy.fill(k.as_mut());
    let mut n = [0u8; AEAD_NONCE_LEN];
    entropy.fill(&mut n);
    let enc = aead_seal(&k, &n, &plaintext, PAD_AAD).map_err(|_| SealError::Malformed)?;
    let keys = to
        .iter()
        .map(|r| {
            let inner = ContentKey { aud: r.cid, k: *k };
            Ok(KeyWrap {
                cid: r.cid,
                wrap: Sealed::seal(r.seal_pk, &inner, entropy)?,
            })
        })
        .collect::<Result<Vec<_>, SealError>>()?;
    let sealed = PadSealed {
        keys,
        n,
        enc: Bytes(enc),
    };
    PadSealed::from_value(sealed.to_value()).map_err(SealError::Wire)
}

/// Opens the text with this Device's key, checking the wrap names this Device's CID.
pub fn open_text(
    sealed: &PadSealed,
    own_cid: Cid,
    key: &KemSecretKey,
) -> Result<PadText, PadOpenError> {
    let wrap = sealed
        .keys
        .iter()
        .find(|w| w.cid == own_cid)
        .ok_or(PadOpenError::NotForMe)?;
    let ck: ContentKey = wrap.wrap.open(key).map_err(PadOpenError::Seal)?;
    let k = Zeroizing::new(ck.k);
    if ck.aud != own_cid {
        return Err(PadOpenError::WrongAudience);
    }
    let plaintext =
        aead_open(&k, &sealed.n, &sealed.enc.0, PAD_AAD).map_err(|_| PadOpenError::OpenFailed)?;
    let value = cbor::decode(&plaintext).map_err(PadOpenError::Wire)?;
    PadText::from_value(value).map_err(PadOpenError::Wire)
}
