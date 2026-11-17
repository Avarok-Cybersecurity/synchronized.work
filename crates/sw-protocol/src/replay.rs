//! Replay state (design §4.5): `(kid, nonce)` kept until `exp`, and the last `seq` per kid.
//! Pure data; the node and each Device persist it through their own stores.

use std::collections::BTreeMap;

use sw_crypto::Kid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayRefusal {
    /// This `(kid, nonce)` was already accepted and has not expired.
    Replay,
    /// `seq` is not greater than the last accepted `seq` for this kid.
    SeqNotIncreasing,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplayGuard {
    seen: BTreeMap<(Kid, [u8; 16]), u64>,
    last_seq: BTreeMap<Kid, u64>,
}

impl ReplayGuard {
    /// An empty guard: what a fresh node or Device starts from (it is state, not a default).
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn check(&self, kid: &Kid, nonce: &[u8; 16], seq: u64) -> Result<(), ReplayRefusal> {
        if self.seen.contains_key(&(*kid, *nonce)) {
            return Err(ReplayRefusal::Replay);
        }
        if self.last_seq.get(kid).is_some_and(|last| seq <= *last) {
            return Err(ReplayRefusal::SeqNotIncreasing);
        }
        Ok(())
    }

    /// Records an accepted message. Call only after every other check passed, so a forged
    /// or expired message can never poison the state.
    pub fn record(&mut self, kid: &Kid, nonce: &[u8; 16], seq: u64, exp: u64) {
        self.seen.insert((*kid, *nonce), exp);
        self.last_seq.insert(*kid, seq);
    }

    /// Restores the last accepted `seq` for `kid` after a restart. Nonces need no restoring:
    /// `seq` is strictly increasing per kid, so a replayed message is refused by its `seq`.
    pub fn restore_seq(&mut self, kid: &Kid, seq: u64) {
        self.last_seq.insert(*kid, seq);
    }

    /// Drops nonces whose message has expired; an expired message is refused by `exp` anyway.
    pub fn prune(&mut self, now_ms: u64) {
        self.seen.retain(|_, exp| *exp > now_ms);
    }

    pub fn last_seq(&self, kid: &Kid) -> Option<u64> {
        self.last_seq.get(kid).copied()
    }

    pub fn tracked_nonces(&self) -> usize {
        self.seen.len()
    }
}
