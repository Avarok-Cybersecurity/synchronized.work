//! Join events in the Device's event log: what the node's `NOTIFY` said, kept as data and
//! handed to one waiter, like an ack. The node's text names people only by `id_fp`.

use super::state::{EventKind, PadState};

impl PadState {
    /// Records a join event once per `NOTIFY` id; returns its cursor, or `None` if seen.
    pub fn notice_in(
        &mut self,
        kind: EventKind,
        event_id: &str,
        subject: &str,
        text: &str,
        exp: u64,
    ) -> Option<u64> {
        let key = format!("notify/{event_id}");
        if self.seen.contains_key(&key) {
            return None;
        }
        self.seen.insert(key, exp);
        let cursor = self.append(kind, event_id, Some(text.to_owned()), exp);
        if let Some(e) = self.events.last_mut() {
            e.subject = Some(subject.to_owned());
        }
        Some(cursor)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_join_event_wakes_one_waiter_once_and_a_repeat_notify_is_dropped() {
        let mut s = PadState::default();
        let c = s
            .notice_in(
                EventKind::JoinRequested,
                "ab",
                "Acme",
                "x asked to join Acme",
                10,
            )
            .unwrap();
        assert!(
            s.notice_in(EventKind::JoinRequested, "ab", "Acme", "x", 10)
                .is_none()
        );
        let e = s.next_event(0).unwrap();
        assert_eq!((e.cursor, e.kind), (c, EventKind::JoinRequested));
        assert_eq!(e.subject.as_deref(), Some("Acme"));
        // Handed out once: the next waiter after 0 gets nothing.
        assert!(s.next_event(0).is_none());
    }
}
