//! Why the node refused a message. Each is logged by the adapter and nothing is applied.

use sw_protocol::WireError;
use sw_protocol::gate::Refusal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeRefusal {
    Wire(WireError),
    Gate(Refusal),
    Tier(sw_model::tier::Refusal),
    /// A type the node never accepts from a session (its own 7, 13, 14), or one that travels
    /// only peer to peer (17).
    NotForTheNode(u8),
    /// A type this node does not serve yet; refused loudly rather than dropped.
    Unsupported(u8),
    /// Anything but `HELLO` before `HELLO`.
    NoHello,
    UnknownPerson,
    StaleEpoch,
    NoSuchEndpoint,
    TakenByAnotherPerson,
    CapReached(&'static str),
    AlreadyExists,
    NoSuchTask,
    NoSuchRequest,
    HasChildren,
    NotAWorker,
    /// Writing under a project its signer is neither the owner nor an admitted member of.
    NotAMember,
    /// An invite names a Task that is not a project's root.
    NotAProject,
    /// A request to join a project spent no invite.
    NeedsInvite,
    /// The invite is unknown, spent or expired.
    InviteInvalid,
    /// An invite's `exp` is past, or further away than `invite_limits::MAX_LIFETIME_MS`.
    InviteLifetime,
    /// The relayed signal's audience is not an enrolled Endpoint of the kind it must reach.
    NoSuchRecipient,
    InboxFull,
    /// A scratchpad rule (sent back to the writer in a `PAD_RECEIPT`).
    Pad(super::PadRefusal),
    /// A link-code rule (sent back to the minter in a `PAD_RECEIPT`).
    Link(super::LinkRefusal),
    /// A `TASK_WRITE` carrying `embedding`: the wire's `uint ≤ 65535` form has no defined map
    /// from the signed buckets `sw-model` recomputes, so it cannot be checked (C0 gap).
    EmbeddingUnverifiable,
    /// A server identity challenge to a node provisioned with no server key.
    NoServerIdentity,
    /// A server identity challenge sooner than the provisioned interval after the last answer
    /// to the same session.
    ChallengeTooSoon,
    Malformed,
}

impl From<WireError> for NodeRefusal {
    fn from(e: WireError) -> Self {
        Self::Wire(e)
    }
}

impl From<Refusal> for NodeRefusal {
    fn from(e: Refusal) -> Self {
        Self::Gate(e)
    }
}
