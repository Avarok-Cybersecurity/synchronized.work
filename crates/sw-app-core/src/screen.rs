//! The first-run states (design §7.5) and, for each, its copy and its next action.

use crate::copy;
use crate::link::{CodeEntry, Refused};
use crate::painter::FrameError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    /// Opening the transport (or reading the saved link first).
    Connecting,
    /// No link for this node in `storage.user`: the link-code entry.
    NeedsLink(LinkScreen),
    /// Linked, nothing to draw.
    Empty(EmptyReason),
    /// The live tree.
    Live,
    Error(AppError),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkScreen {
    pub entry: CodeEntry,
    /// The receipt id of the redeem in flight.
    pub pending: Option<[u8; 32]>,
    /// The node refused the last code.
    pub refused: Option<Refused>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyReason {
    /// The link bound no Project to this node.
    NoProject,
    /// Subscribed; no Task is visible yet.
    NothingYet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    /// Neither `citadel.session` nor `net.host` is granted.
    NoTransport,
    /// The open answered `no_identity` and the fallback cannot stand in.
    NoIdentity,
    /// The fallback was chosen, but this node was never linked.
    NotLinkedOnFallback,
    SessionClosed {
        reason: String,
    },
    OpenFailed {
        reason: String,
    },
    Wire,
    Send {
        reason: String,
    },
    Receive {
        reason: String,
    },
    /// The fold could not be made into a tree (a defect, shown rather than hidden).
    Tree {
        reason: String,
    },
    Http {
        status: u16,
    },
    HttpFailed {
        reason: String,
    },
    Storage {
        reason: String,
    },
    Frame(FrameError),
}

/// What the screen's button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextAction {
    /// Reopen the transport; the fold resumes from its cursor.
    Retry,
    EnterLinkCode,
    SubmitCode,
    /// Hand the member the site's address (the host opens internal links only).
    OpenSite,
}

impl NextAction {
    /// The region id of its button (in the contract's id grammar).
    pub fn id(self) -> &'static str {
        match self {
            Self::Retry => "retry",
            Self::EnterLinkCode => "enter-code",
            Self::SubmitCode => "submit-code",
            Self::OpenSite => "open-site",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        [
            Self::Retry,
            Self::EnterLinkCode,
            Self::SubmitCode,
            Self::OpenSite,
        ]
        .into_iter()
        .find(|a| a.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Retry => copy::ACTION_RETRY,
            Self::EnterLinkCode => copy::ACTION_ENTER_CODE,
            Self::SubmitCode => copy::ACTION_SUBMIT_CODE,
            Self::OpenSite => copy::ACTION_OPEN_SITE,
        }
    }
}

impl AppError {
    pub fn next(&self) -> NextAction {
        match self {
            Self::NoTransport | Self::NoIdentity | Self::NotLinkedOnFallback => {
                NextAction::OpenSite
            }
            _ => NextAction::Retry,
        }
    }

    /// The lines under the error title.
    pub fn lines(&self) -> Vec<String> {
        let own = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect();
        let with = |head: &str, tail: String| {
            vec![head.to_owned(), tail, copy::ERROR_NEXT_RETRY.to_owned()]
        };
        match self {
            Self::NoTransport => own(copy::ERROR_NO_TRANSPORT),
            Self::NoIdentity => own(copy::ERROR_NO_IDENTITY),
            Self::NotLinkedOnFallback => own(copy::ERROR_NOT_LINKED_FALLBACK),
            Self::SessionClosed { reason } => with(copy::ERROR_SESSION_CLOSED, reason.clone()),
            Self::OpenFailed { reason } => with(copy::ERROR_OPEN_FAILED, reason.clone()),
            Self::Wire => own(&[copy::ERROR_WIRE, copy::ERROR_NEXT_RETRY]),
            Self::Send { reason } => with(copy::ERROR_SEND, reason.clone()),
            Self::Receive { reason } => with(copy::ERROR_RECEIVE, reason.clone()),
            Self::Tree { reason } => with(copy::ERROR_TREE, reason.clone()),
            Self::Http { status } => with(copy::ERROR_HTTP, status.to_string()),
            Self::HttpFailed { reason } => with(copy::ERROR_HTTP_FAILED, reason.clone()),
            Self::Storage { reason } => with(copy::ERROR_STORAGE, reason.clone()),
            Self::Frame(e) => with(copy::ERROR_FRAME, format!("{e:?}")),
        }
    }
}

impl Screen {
    /// The actions the screen offers, primary first (Enter triggers the primary one).
    pub fn actions(&self) -> Vec<NextAction> {
        match self {
            Self::Connecting | Self::Live => Vec::new(),
            Self::NeedsLink(l) if l.pending.is_some() => Vec::new(),
            Self::NeedsLink(_) => vec![NextAction::SubmitCode, NextAction::OpenSite],
            Self::Empty(EmptyReason::NoProject) => {
                vec![NextAction::EnterLinkCode, NextAction::OpenSite]
            }
            Self::Empty(EmptyReason::NothingYet) => vec![NextAction::OpenSite],
            Self::Error(e) => vec![e.next()],
        }
    }
}
