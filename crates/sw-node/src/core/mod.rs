//! The node's business rules (SBIO): bytes from a session in, [`Effect`]s out. No Citadel, no
//! storage, no clock here; the kernel adapter (`crate::kernel`) runs the effects. Every
//! signed message passes `sw_protocol::gate::verify` — the one gate every party shares —
//! before anything below looks at it.

mod challenge;
mod codec;
mod coop;
mod directory;
mod dispatch;
mod enrol;
mod fanout;
mod feed;
mod inbox;
mod invite;
mod link;
pub mod load;
mod notify;
mod pad;
mod receipt;
mod refusal;
mod significant;
mod task;
mod tree;

pub use challenge::{ServerIdentity, frame as challenge_frame};
pub use codec::{task_from_bytes, task_to_bytes};
pub use directory::Directory;
pub use feed::serve::UNRECORDED_SENDER;
pub use invite::invite_key;
pub use link::LinkRefusal;
pub use notify::{DEDUP_WINDOW_MS, event_id};
pub use pad::PadRefusal;
pub use receipt::RECEIPTED;
pub use refusal::NodeRefusal;
pub use significant::{Kind, Significant};
pub use task::{AgentState, TaskState};

use std::collections::BTreeSet;

use sw_model::{Cid, PersonPolicy};
use sw_protocol::replay::ReplayGuard;

use crate::limits::Limits;
use inbox::Inbox;
use notify::Notifier;
use tree::Tree;

/// What the adapter must do for the core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Send `bytes` (an `SwMessage`) on the live session `to`.
    Send {
        to: Cid,
        bytes: Vec<u8>,
    },
    /// Store `value` under `key` (one `put`, atomic).
    Put {
        key: String,
        value: Vec<u8>,
    },
    Delete {
        key: String,
    },
}

/// The rules the node is configured with (from `Provisioning`).
#[derive(Debug, Clone)]
pub struct Rules {
    /// work.avarok.net's pinned server key (ML-DSA-65, 1952 bytes).
    pub sign_in_issuer: Vec<u8>,
    pub limits: Limits,
    pub new_person_policy: PersonPolicy,
    /// What a server identity challenge is answered with; a core without one refuses it.
    pub server_identity: Option<ServerIdentity>,
}

#[derive(Debug)]
pub struct NodeCore {
    rules: Rules,
    directory: Directory,
    replay: ReplayGuard,
    tree: Tree,
    inbox: Inbox,
    notifier: Notifier,
    pad: pad::ledger::Ledger,
    invites: invite::Invites,
    feed: feed::Feed,
    links: link::Links,
    online: BTreeSet<Cid>,
    /// Sessions that sent `HELLO`, the first message of every session.
    greeted: BTreeSet<Cid>,
    answered: challenge::Answered,
}

impl NodeCore {
    pub fn new(rules: Rules) -> Self {
        Self {
            rules,
            directory: Directory::default(),
            replay: ReplayGuard::empty(),
            tree: Tree::default(),
            inbox: Inbox::default(),
            notifier: Notifier::default(),
            pad: pad::ledger::Ledger::default(),
            invites: invite::Invites::default(),
            feed: feed::Feed::default(),
            links: link::Links::default(),
            online: BTreeSet::new(),
            greeted: BTreeSet::new(),
            answered: challenge::Answered::default(),
        }
    }

    pub fn directory(&self) -> &Directory {
        &self.directory
    }

    pub fn task(&self, id: &str) -> Option<&TaskState> {
        self.tree.get(id)
    }

    pub fn tasks(&self) -> usize {
        self.tree.len()
    }

    pub fn is_online(&self, cid: Cid) -> bool {
        self.online.contains(&cid)
    }

    /// A session opened: deliver what waited for it.
    pub fn connected(&mut self, cid: Cid, now_ms: u64) -> Vec<Effect> {
        self.online.insert(cid);
        let (send, gone) = self.inbox.drain_for(cid, now_ms);
        let mut effects: Vec<Effect> = send
            .into_iter()
            .map(|bytes| Effect::Send { to: cid, bytes })
            .collect();
        effects.extend(gone.iter().map(|id| Effect::Delete {
            key: inbox::key(cid, id),
        }));
        effects.extend(self.pad_connected(cid, now_ms));
        effects
    }

    /// A session ended: it is offline, and its subscriptions end with it.
    pub fn disconnected(&mut self, cid: Cid) {
        self.online.remove(&cid);
        self.greeted.remove(&cid);
        self.answered.forget(cid);
        self.feed.drop_session(cid);
    }

    /// Live sessions of a Person for an event of `project`: every enrolled Endpoint, and the
    /// accounts linked to the Person for that Project.
    fn sessions_of(&self, person: &sw_model::IdFp, project: &str) -> Vec<Cid> {
        let mut cids = self.directory.endpoints_of(person);
        cids.extend(self.links.linked(person, project));
        cids.retain(|cid| self.online.contains(cid));
        cids.sort_unstable();
        cids.dedup();
        cids
    }
}
