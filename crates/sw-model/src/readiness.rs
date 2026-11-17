//! Readiness and admission order (design §2.4). One ranking serves both of the places the
//! lanes model admits work: a Person's own `sw-mcp` admitting its waiting lanes, and the
//! ProjectDO ordering cross-person join requests under an `aware` Task.

/// Why a lane cannot proceed now (sw-wire contract §3 `Blocker.kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BlockerKind {
    Human,
    Dependency,
    Lock,
    Disk,
    RateLimit,
}

impl BlockerKind {
    /// Design §2.4's table, in permille: the one place a kind's readiness is written.
    pub fn readiness(self) -> Readiness {
        match self {
            Self::Human => Readiness::HUMAN,
            Self::Dependency => Readiness::DEPENDENCY,
            Self::Lock | Self::Disk | Self::RateLimit => Readiness::RESOURCE,
        }
    }
}

/// A declared reason a lane cannot proceed now. `task_id` names the Task waited on, and is
/// present exactly when `kind` is `Dependency`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Blocker {
    pub kind: BlockerKind,
    pub text: String,
    pub task_id: Option<String>,
}

impl Blocker {
    pub const MAX_TEXT_BYTES: usize = 280;

    /// The shape rules every party enforces: text at most 280 bytes, `task_id` iff
    /// `Dependency`.
    pub fn is_well_formed(&self) -> bool {
        self.text.len() <= Self::MAX_TEXT_BYTES
            && (self.kind == BlockerKind::Dependency) == self.task_id.is_some()
    }
}

/// Readiness as a permille integer, `0..=1000`: exact, so every party ranks identically on
/// every target, and the wire carries no floats (sw-wire contract §3 `Readiness`; design
/// §2.4's f32 in [0, 1] scaled by 1000).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Readiness(u16);

impl Readiness {
    pub const READY: Self = Self(1000);
    pub const HUMAN: Self = Self(0);
    pub const DEPENDENCY: Self = Self(200);
    pub const RESOURCE: Self = Self(500);

    pub fn from_permille(permille: u16) -> Option<Self> {
        (permille <= 1000).then_some(Self(permille))
    }

    pub fn permille(self) -> u16 {
        self.0
    }
}

/// `readiness(agent)`: 1000 with no blockers, else the minimum over its blockers.
/// `dependency_done` answers whether a dependency blocker's Task is done; a done dependency
/// no longer blocks.
pub fn readiness(blockers: &[Blocker], dependency_done: impl Fn(&str) -> bool) -> Readiness {
    blockers
        .iter()
        .map(|b| match (&b.kind, &b.task_id) {
            (BlockerKind::Dependency, Some(t)) if dependency_done(t) => Readiness::READY,
            (kind, _) => kind.readiness(),
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
