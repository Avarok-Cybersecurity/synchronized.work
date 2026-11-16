//! Readiness and admission order (design §2.4). One ranking serves both of the places the
//! lanes model admits work: a Person's own `sw-mcp` admitting its waiting lanes, and the
//! ProjectDO ordering cross-person join requests under an `aware` Task.

use crate::Id128;

/// A declared reason a lane cannot proceed now.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Blocker {
    Lock,
    Disk,
    Human,
    Dependency(Id128),
    RateLimit,
    Other(String),
}

/// Readiness in thousandths: an exact integer so every party ranks identically on every
/// target (design §2.4 writes it as an f32 in [0, 1]; the values are the same).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Readiness(u16);

impl Readiness {
    pub const READY: Self = Self(1000);
    pub const HUMAN: Self = Self(0);
    pub const DEPENDENCY: Self = Self(200);
    pub const RESOURCE: Self = Self(500);

    pub fn from_milli(milli: u16) -> Option<Self> {
        (milli <= 1000).then_some(Self(milli))
    }

    pub fn milli(self) -> u16 {
        self.0
    }
}

/// `readiness(agent)`: 1 with no blockers, else the minimum over its blockers.
/// `dependency_done` answers whether a `dependency(t)` blocker's Task is done; a done
/// dependency no longer blocks. `Other` is treated like a resource wait (0.5): it is a
/// declared, non-human obstacle.
pub fn readiness(blockers: &[Blocker], dependency_done: impl Fn(Id128) -> bool) -> Readiness {
    blockers
        .iter()
        .map(|b| match b {
            Blocker::Human => Readiness::HUMAN,
            Blocker::Dependency(t) if dependency_done(*t) => Readiness::READY,
            Blocker::Dependency(_) => Readiness::DEPENDENCY,
            Blocker::Lock | Blocker::Disk | Blocker::RateLimit | Blocker::Other(_) => {
                Readiness::RESOURCE
            }
        })
        .min()
        .unwrap_or(Readiness::READY)
}

/// Something waiting to be admitted: a waiting lane, or a join request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate<K> {
    pub key: K,
    pub readiness: Readiness,
    /// When the lane started or the request was made; earlier wins a tie.
    pub since_ms: u64,
}

/// Readiest first; ties by `since_ms`, then by key so the order is total and stable.
pub fn rank<K: Ord + Clone>(candidates: &[Candidate<K>]) -> Vec<Candidate<K>> {
    let mut out = candidates.to_vec();
    out.sort_by(|a, b| {
        b.readiness
            .cmp(&a.readiness)
            .then(a.since_ms.cmp(&b.since_ms))
            .then(a.key.cmp(&b.key))
    });
    out
}

/// Which candidates to admit now given `running` admitted already and an explicit `cap`.
/// Blocked candidates are not excluded; they step aside for readier ones.
pub fn admit<K: Ord + Clone>(candidates: &[Candidate<K>], running: usize, cap: usize) -> Vec<K> {
    rank(candidates)
        .into_iter()
        .take(cap.saturating_sub(running))
        .map(|c| c.key)
        .collect()
}
