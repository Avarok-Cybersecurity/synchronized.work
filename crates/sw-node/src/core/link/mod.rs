//! Linking a Citadel App's Endpoint to a Person with a link code (design §5.3 "ships first",
//! §7.5). A signed-in Endpoint of the Person mints a code with a signed `LINK_CODE`; the App's
//! agent-held account redeems it, unsigned, with `LINK_REDEEM{code, acting_username,
//! install_id, node_id}`. That enrols the account as a **viewer** of the Person for the code's
//! Project, *pending root signature*: it has no signing key and is in no Person record, so it
//! can subscribe to that Project and receive the Person's `NOTIFY`s about it, and nothing else
//! (§5.2: an account password gives viewing, not control).
//!
//! The frozen `LINK_CODE` names no Project, so a code is bound to the Project its minting
//! session subscribed to last; the Person must be a member of it. The node keeps only the
//! code's hash; the answer to both ops is a `PAD_RECEIPT` with `typ: 16`.

mod codec;
mod flow;

pub(super) use codec::{code_from_bytes, code_to_bytes, viewer_from_bytes, viewer_to_bytes};

use std::collections::{BTreeMap, VecDeque};

use sw_model::{Cid, IdFp};

use super::pad::ledger::hex;
use crate::limits::Limits;

/// Why a mint or a redeem was refused, in words the person can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkRefusal {
    NotACode,
    Unknown,
    Expired,
    Spent,
    /// The minting session follows no Project.
    NoProject,
    /// A live code with the same hash exists; the minter draws another.
    Taken,
    TooManyCodes,
    OtherPerson,
    NotAMember,
    TooManyAttempts,
}

impl LinkRefusal {
    pub fn reason(self) -> &'static str {
        match self {
            Self::NotACode => "That is not a link code: a code is 8 letters and digits.",
            Self::Unknown => {
                "That code is not known. Check it, or get a new one with sw-mcp link-code."
            }
            Self::Expired => {
                "That code has expired (codes last 10 minutes). Get a new one with sw-mcp link-code."
            }
            Self::Spent => "That code was already used. Get a new one with sw-mcp link-code.",
            Self::NoProject => {
                "No project for the code: this session follows none you are a member of."
            }
            Self::Taken => "That code is in use; draw another.",
            Self::TooManyCodes => {
                "This person already has the most unused link codes allowed; use or let one expire."
            }
            Self::OtherPerson => "This app is already linked to another person.",
            Self::NotAMember => "The code's person is not a member of its project.",
            Self::TooManyAttempts => "Too many link attempts. Wait a minute, then try again.",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Code {
    pub person: IdFp,
    pub project: String,
    pub exp: u64,
    pub spent: bool,
}

/// One `(install, node)` of a linked account: the Project that node shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewerLink {
    pub person: IdFp,
    pub project: String,
    pub node_id: String,
    pub install_id: String,
    pub acting_username: String,
    pub at: u64,
}

pub fn code_key(hash: &[u8; 32]) -> String {
    format!("link/{}", hex(hash))
}

pub fn viewer_key(cid: Cid, node_id: &str) -> String {
    format!("viewer/{cid}/{}", hex(node_id.as_bytes()))
}

const MINUTE_MS: u64 = 60_000;

#[derive(Debug, Default)]
pub struct Links {
    codes: BTreeMap<[u8; 32], Code>,
    /// Linked accounts: CID → node id → link.
    viewers: BTreeMap<Cid, BTreeMap<String, ViewerLink>>,
    /// Redeem attempts in the last minute, per account (a CID is permanent per account) and
    /// for the node.
    attempts: BTreeMap<Cid, VecDeque<u64>>,
    node_attempts: VecDeque<u64>,
}

fn within(q: &mut VecDeque<u64>, now_ms: u64, per_min: usize) -> bool {
    while q
        .front()
        .is_some_and(|at| now_ms.saturating_sub(*at) >= MINUTE_MS)
    {
        q.pop_front();
    }
    q.len() < per_min
}

impl Links {
    /// Counts one redeem attempt, or refuses it past either limit.
    fn take_attempt(&mut self, cid: Cid, now_ms: u64, l: &Limits) -> bool {
        let mine = self.attempts.entry(cid).or_default();
        if !within(mine, now_ms, l.link_redeem_per_min)
            || !within(&mut self.node_attempts, now_ms, l.link_redeem_node_per_min)
        {
            return false;
        }
        mine.push_back(now_ms);
        self.node_attempts.push_back(now_ms);
        true
    }

    /// The Person a linked account views for.
    pub fn person_of(&self, cid: Cid) -> Option<&IdFp> {
        self.viewers.get(&cid)?.values().next().map(|l| &l.person)
    }

    /// Whether `cid` is linked to `project` (for any of its nodes).
    pub fn links_project(&self, cid: Cid, project: &str) -> bool {
        self.viewers
            .get(&cid)
            .is_some_and(|ls| ls.values().any(|l| l.project == project))
    }

    /// The accounts linked to `person` for `project`.
    pub fn linked(&self, person: &IdFp, project: &str) -> Vec<Cid> {
        self.viewers
            .iter()
            .filter(|(_, ls)| {
                ls.values()
                    .any(|l| l.person == *person && l.project == project)
            })
            .map(|(cid, _)| *cid)
            .collect()
    }

    pub(super) fn load_code(&mut self, hash: [u8; 32], code: Code) {
        self.codes.insert(hash, code);
    }

    pub(super) fn load_viewer(&mut self, cid: Cid, link: ViewerLink) {
        self.viewers
            .entry(cid)
            .or_default()
            .insert(link.node_id.clone(), link);
    }
}
