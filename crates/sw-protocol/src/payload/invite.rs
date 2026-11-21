//! Joining a project by invitation (types 23 and 24). An invite is a one-time token the owner
//! hands a person out of band; the node stores only its hash, bound to one project, until it
//! expires or a `JOIN_REQUEST` spends it. Admission is still the owner's signed `JOIN_ANSWER`.

use sw_model::IdFp;

use crate::WireError;
use crate::wire::wire_text;

/// Invitation limits (SSOT for every party): protocol rules, not tunables.
pub mod invite_limits {
    /// The longest an invite may live.
    pub const MAX_LIFETIME_MS: u64 = 7 * 24 * 60 * 60 * 1000;
    /// Unspent, unexpired invites one project may hold at once.
    pub const LIVE_PER_PROJECT: usize = 32;
    /// Pending join requests one `JOIN_PENDING` answer lists.
    pub const PENDING_LISTED: usize = 256;
}

wire_text! {
    /// 23 `INVITE`, Endpoint-signed by the project's owner: the hash of a fresh one-time token
    /// (see `crate::invite::token_hash`) and when it stops being accepted. Making the first
    /// invite opens the project to join requests (`off → aware`, manual admission).
    pub struct Invite {
        project: String,
        token_hash: [u8; 32],
        exp: u64,
    }
}

wire_text! {
    /// One join request waiting for its Task owner's answer.
    pub struct PendingJoin {
        /// The request's `signal_id`, which a `JOIN_ANSWER` names.
        request_id: [u8; 32],
        project: String,
        task_id: String,
        person: IdFp,
        /// The display handle the node holds for that Person (as the node reports it).
        display: String,
        at: u64,
    }
}

wire_text! {
    /// 24 `JOIN_PENDING`, unsigned: an Endpoint asks (`requests` absent) for the requests
    /// waiting on its Person's answer, in one project or (`project` absent) in all; the node
    /// answers with them.
    pub struct JoinPending {
        project: Option<String>,
        requests: Option<Vec<PendingJoin>>,
    }
    check = JoinPending::check;
}

impl JoinPending {
    fn check(&self) -> Result<(), WireError> {
        if self
            .requests
            .as_ref()
            .is_none_or(|r| r.len() <= invite_limits::PENDING_LISTED)
        {
            Ok(())
        } else {
            Err(WireError::Length("JOIN_PENDING requests"))
        }
    }
}
