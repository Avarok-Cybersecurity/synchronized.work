//! Per-task visibility (design §6.5): the one function the ProjectDO snapshot, its fan-out
//! and the `NOTIFY` text filter all call.

use crate::readiness::Blocker;
use crate::{Progress, Visibility};

/// How the viewer relates to the Task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Viewer {
    TaskOwner,
    /// Any other member of the Project, whatever their role.
    ProjectMember,
    /// Not a member of the Project.
    Outsider,
}

/// What of a Task's progress a viewer may receive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Nothing,
    /// That the Task exists, and each Agent's percent.
    ExistenceAndPercent,
    /// Phase and percent, no bodies, no blockers.
    PhaseAndPercent,
    /// Everything: bodies, blockers, last lines.
    Full,
}

pub fn level(visibility: Visibility, viewer: Viewer) -> Level {
    match (visibility, viewer) {
        (_, Viewer::Outsider) => Level::Nothing,
        (_, Viewer::TaskOwner) => Level::Full,
        (Visibility::Owner, Viewer::ProjectMember) => Level::ExistenceAndPercent,
        (Visibility::Members, Viewer::ProjectMember) => Level::Full,
        (Visibility::Summary, Viewer::ProjectMember) => Level::PhaseAndPercent,
    }
}

/// A Progress as one viewer may see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Visible {
    pub percent: u8,
    pub phase: Option<String>,
    pub last_line: Option<String>,
    pub blockers: Option<Vec<Blocker>>,
}

/// Whether a viewer at `level` is shown bodies, blockers and last lines.
pub fn bodies_shown(level: Level) -> bool {
    level == Level::Full
}

pub fn redact(level: Level, progress: &Progress, blockers: &[Blocker]) -> Option<Visible> {
    let full = bodies_shown(level);
    match level {
        Level::Nothing => None,
        Level::ExistenceAndPercent | Level::PhaseAndPercent | Level::Full => Some(Visible {
            percent: progress.percent,
            phase: (level >= Level::PhaseAndPercent).then(|| progress.phase.clone()),
            last_line: full.then(|| progress.last_line.clone()),
            blockers: full.then(|| blockers.to_vec()),
        }),
    }
}

/// Whether a viewer at `level` is shown the Task's title (and, at `Full`, its body): the
/// rule both the `EVENT` projection and the `NOTIFY` text follow.
pub fn title_shown(level: Level) -> bool {
    level >= Level::PhaseAndPercent
}

/// `NOTIFY` text for a viewer: a `vis:owner` task reaches non-owners without its title.
pub fn notify_subject(visibility: Visibility, viewer: Viewer, title: &str) -> Option<String> {
    match level(visibility, viewer) {
        Level::Nothing => None,
        l if title_shown(l) => Some(title.to_owned()),
        _ => Some("a task you work under".to_owned()),
    }
}
