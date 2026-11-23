//! Linking the install's Endpoint to a Person with a link code (design §5.3 "ships first",
//! §7.5). The member types the 8-character code `sw-mcp link-code` minted; the App sends
//! `LINK_REDEEM{code, acting_username, install_id, node_id}` (unsigned, slot 2) over its
//! session. The node answers with a `PAD_RECEIPT` for `typ 16` whose `id` is the SHA3-256 of
//! the redeem's bytes and whose `project` is the Project bound to this node (empty: none yet).
//! The outcome is remembered in `storage.user` under [`storage_key`].

use sw_protocol::WireError;
use sw_protocol::payload::{Link, LinkOp, PadReceipt, Payload};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::{MapIn, MapOut};

use crate::ctx::AppCtx;

/// The code as typed so far.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeEntry {
    code: String,
}

impl CodeEntry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn is_complete(&self) -> bool {
        self.code.chars().count() == Link::CODE_LEN
    }

    /// One `KeyEvent.key`: an ASCII letter or digit is appended (up to the code's length),
    /// `Backspace` removes the last one. Returns whether the entry changed.
    pub fn key(&mut self, key: &str) -> bool {
        if key == "Backspace" {
            return self.code.pop().is_some();
        }
        let mut chars = key.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) if c.is_ascii_alphanumeric() && !self.is_complete() => {
                self.code.push(c);
                true
            }
            _ => false,
        }
    }
}

/// The `LINK_REDEEM` for `code` in this context, and the receipt id that answers it.
pub fn redeem(code: &str, ctx: &AppCtx) -> Result<(Vec<u8>, [u8; 32]), WireError> {
    let bytes = SwMessage::Unsigned(Payload::Link(Link {
        op: LinkOp::Redeem,
        code: code.to_owned(),
        acting_username: Some(ctx.acting_username.clone()),
        install_id: Some(ctx.install_id.clone()),
        node_id: Some(ctx.node_id.clone()),
    }))
    .to_bytes()?;
    let id = sw_crypto::sha3_256(&bytes);
    Ok((bytes, id))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkOutcome {
    /// Linked; the Project this node shows, if one is bound yet.
    Linked {
        project: Option<String>,
    },
    Refused(Refused),
}

/// The node refused the code; `reason` is its own, when it gave one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub reason: Option<String>,
}

/// What `receipt` says about the redeem `pending`, if it is the answer to it.
pub fn outcome(receipt: &PadReceipt, pending: &[u8; 32]) -> Option<LinkOutcome> {
    if receipt.typ != 16 || receipt.id != *pending {
        return None;
    }
    Some(if receipt.stored {
        LinkOutcome::Linked {
            project: Some(receipt.project.clone()).filter(|p| !p.is_empty()),
        }
    } else {
        LinkOutcome::Refused(Refused {
            reason: receipt.reason.clone(),
        })
    })
}

/// What `storage.user` keeps for one node: that the install is linked, and its Project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRecord {
    pub project: Option<String>,
}

pub const RECORD_VERSION: u64 = 1;

/// One record per node the install is shown on.
pub fn storage_key(node_id: &str) -> String {
    format!("sw-link/{node_id}")
}

impl LinkRecord {
    pub fn to_bytes(&self) -> Result<Vec<u8>, WireError> {
        let mut map = MapOut::default();
        map.field("v", &RECORD_VERSION);
        map.field("project", &self.project);
        sw_protocol::cbor::encode(&map.build())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WireError> {
        let mut map = MapIn::new(sw_protocol::cbor::decode(bytes)?)?;
        let v: u64 = map.field("v")?;
        let project: Option<String> = map.field("project")?;
        map.finish()?;
        if v != RECORD_VERSION {
            return Err(WireError::UnknownCode("LinkRecord.v"));
        }
        Ok(Self { project })
    }
}
