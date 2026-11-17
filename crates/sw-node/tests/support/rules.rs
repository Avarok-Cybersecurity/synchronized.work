//! The node's test rules, and what a test reads back from what it sent.

use sw_crypto::SigningKey;
use sw_model::{Approval, Cid, PersonPolicy};
use sw_node::core::Rules;
use sw_node::limits::Limits;
use sw_protocol::payload::Payload;
use sw_protocol::signed::SwMessage;

/// Test limits: small enough that a test can reach each one.
pub fn rules(issuer: &SigningKey) -> Rules {
    Rules {
        sign_in_issuer: issuer.public_key().to_vec(),
        limits: Limits {
            inbox_max_signals: 4,
            inbox_max_bytes: 64 * 1024,
            max_persons: 8,
            max_tasks: 16,
        },
        new_person_policy: PersonPolicy {
            approval: Approval::Manual,
            rate_per_min: 10,
        },
    }
}

/// The `NOTIFY` kinds in what the node sent, per recipient CID.
pub fn notified(sent: &[(Cid, Vec<u8>)]) -> Vec<(Cid, String)> {
    sent.iter()
        .filter_map(|(to, bytes)| match SwMessage::from_bytes(bytes).ok()? {
            SwMessage::Unsigned(Payload::Notify(n)) => Some((*to, n.kind)),
            _ => None,
        })
        .collect()
}
