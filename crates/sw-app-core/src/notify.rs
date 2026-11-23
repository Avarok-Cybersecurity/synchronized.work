//! `NOTIFY` → the host's `Notify` (design §4.7): each one becomes exactly one host notification
//! with the node's own text and a navigate target of the App's node, once per `event_id`. A
//! `NOTIFY` is information only: it changes no state, sends nothing and redraws nothing (the
//! graph redraws from `EVENT`s).

use std::collections::{BTreeSet, VecDeque};

use sw_protocol::payload::Notify;

/// One host notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostNotify {
    /// The `NOTIFY`'s `event_id`: the host's answer names it, so a refusal re-queues this one.
    pub event_id: [u8; 32],
    /// The visibility-filtered subject the node chose (a hidden title stays hidden).
    pub title: String,
    pub body: String,
    /// Where activating it navigates: the node hosting this App.
    pub node_id: String,
}

/// Remembers the last `capacity` `event_id`s; the oldest is forgotten first.
#[derive(Debug, Clone)]
pub struct NotifyMap {
    capacity: usize,
    order: VecDeque<[u8; 32]>,
    seen: BTreeSet<[u8; 32]>,
}

impl NotifyMap {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            order: VecDeque::new(),
            seen: BTreeSet::new(),
        }
    }

    /// The notification for `n`, or `None` when this `event_id` was already shown.
    pub fn map(&mut self, n: &Notify, node_id: &str) -> Option<HostNotify> {
        if !self.seen.insert(n.event_id) {
            return None;
        }
        self.order.push_back(n.event_id);
        while self.order.len() > self.capacity {
            if let Some(old) = self.order.pop_front() {
                self.seen.remove(&old);
            }
        }
        Some(HostNotify {
            event_id: n.event_id,
            title: n.subject.clone(),
            body: n.text.clone(),
            node_id: node_id.to_owned(),
        })
    }
}
