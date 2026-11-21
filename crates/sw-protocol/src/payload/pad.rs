//! The developer scratchpad (types 18–22): notes one member leaves for others in a project,
//! and their acknowledgements. Bodies and replies travel only inside [`PadSealed`]; the node
//! stores and relays them unread. Text keys, like every payload.

use sw_model::{Cid, IdFp};

use crate::WireError;
use crate::sealed::Sealed;
use crate::wire::{Bytes, wire_str, wire_text};

/// Size limits of the scratchpad (SSOT for every party).
pub mod pad_limits {
    /// A note's body or an ack's reply, in bytes, before sealing.
    pub const TEXT_BYTES: usize = 4096;
    pub const REFS: usize = 16;
    pub const REF_BYTES: usize = 512;
    /// Explicit targets of one note.
    pub const TARGETS: usize = 64;
    /// Recipient Devices one note may be sealed to (keeps one stored entry under 128 KiB).
    pub const KEYS: usize = 64;
    /// Verified Devices listed for one member in a `PAD_DIRECTORY` answer.
    pub const PROOFS_PER_MEMBER: usize = 16;
    /// The longest `exp - iat` of a note or ack; a project's retention may only shorten it.
    pub const LIFETIME_MS: u64 = 7 * 24 * 60 * 60 * 1000;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadScope {
    /// Every member but the sender; `to` is empty.
    All,
    /// Exactly the members in `to`.
    Members,
}
wire_str!(PadScope { "all" => PadScope::All, "members" => PadScope::Members });

wire_text! {
    /// One recipient Device's copy of the content key, sealed to its ML-KEM key.
    pub struct KeyWrap {
        cid: Cid,
        wrap: Sealed,
    }
}

wire_text! {
    /// `enc = AES-256-GCM(K, n, "sw/pad/1", CBOR(PadText))`; `K` is in every `keys[].wrap`.
    pub struct PadSealed {
        keys: Vec<KeyWrap>,
        n: [u8; 12],
        enc: Bytes,
    }
    check = PadSealed::check;
}

impl PadSealed {
    fn check(&self) -> Result<(), WireError> {
        let shapes = self.keys.iter().all(|k| k.wrap.check_shape().is_ok());
        if !self.keys.is_empty() && self.keys.len() <= pad_limits::KEYS && shapes {
            Ok(())
        } else {
            Err(WireError::Shape("PadSealed keys"))
        }
    }
}

wire_text! {
    /// What a [`PadSealed`] opens to.
    pub struct PadText {
        body: String,
        refs: Vec<String>,
    }
    check = PadText::check;
}

impl PadText {
    fn check(&self) -> Result<(), WireError> {
        let ok = self.body.len() <= pad_limits::TEXT_BYTES
            && self.refs.len() <= pad_limits::REFS
            && self.refs.iter().all(|r| r.len() <= pad_limits::REF_BYTES);
        if ok {
            Ok(())
        } else {
            Err(WireError::Length("PadText limit"))
        }
    }
}

wire_text! {
    /// The inner map of one [`KeyWrap`]: the Device it is for, and the content key.
    pub struct ContentKey {
        aud: Cid,
        k: [u8; 32],
    }
}

wire_text! {
    /// 18 `PAD_NOTE`, Device-signed, addressed to the node. `proof` is the sender Device's
    /// own ENROL-by-assertion message, so a reader can verify the sender without the node.
    pub struct PadNote {
        project: String,
        scope: PadScope,
        to: Vec<IdFp>,
        body: PadSealed,
        proof: Bytes,
    }
    check = PadNote::check;
}

impl PadNote {
    fn check(&self) -> Result<(), WireError> {
        let ok = match self.scope {
            PadScope::All => self.to.is_empty(),
            PadScope::Members => !self.to.is_empty() && self.to.len() <= pad_limits::TARGETS,
        };
        if ok {
            Ok(())
        } else {
            Err(WireError::Shape("PAD_NOTE to for its scope"))
        }
    }
}

wire_text! {
    /// 19 `PAD_ACK`, Device-signed: `note_id` is the note's `signal_id`; `reply` is sealed to
    /// the note's sender Device alone.
    pub struct PadAck {
        project: String,
        note_id: [u8; 32],
        reply: Option<PadSealed>,
        proof: Bytes,
    }
}

wire_text! {
    /// 20 `PAD_POLICY`: the project owner sets how long notes live (≤ `LIFETIME_MS`).
    pub struct PadPolicy {
        project: String,
        retention_ms: u64,
    }
    check = PadPolicy::check;
}

impl PadPolicy {
    fn check(&self) -> Result<(), WireError> {
        if (1..=pad_limits::LIFETIME_MS).contains(&self.retention_ms) {
            Ok(())
        } else {
            Err(WireError::Range("retention_ms"))
        }
    }
}

wire_text! {
    pub struct PadMember {
        id_fp: IdFp,
        display: String,
        /// Each Device's ENROL-by-assertion message; readers verify every one.
        proofs: Vec<Bytes>,
    }
}

wire_text! {
    /// 21 `PAD_DIRECTORY`, unsigned: an Endpoint asks (`members` absent), the node answers.
    pub struct PadDirectory {
        project: String,
        members: Option<Vec<PadMember>>,
    }
}

wire_text! {
    /// 22 `PAD_RECEIPT`, unsigned, node → sender: whether a 18/19 was stored.
    pub struct PadReceipt {
        project: String,
        id: [u8; 32],
        typ: u8,
        stored: bool,
        reason: Option<String>,
    }
}
