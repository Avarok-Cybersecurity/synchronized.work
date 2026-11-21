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
    /// The relayed signal's audience is not an enrolled Endpoint of the kind it must reach.
    NoSuchRecipient,
    InboxFull,
    /// A scratchpad rule (sent back to the writer in a `PAD_RECEIPT`).
    Pad(super::PadRefusal),
    /// A `TASK_WRITE` carrying `embedding`: the wire's `uint ≤ 65535` form has no defined map
    /// from the signed buckets `sw-model` recomputes, so it cannot be checked (C0 gap).
    EmbeddingUnverifiable,
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
