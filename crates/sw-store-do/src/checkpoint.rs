//! Checkpoints from the object's alarm (lattice-db `DurableEngine`: "Checkpoints are taken
//! only when the caller asks, for example from an alarm"). The write path never checkpoints,
//! so a burst of writes is not slowed by a snapshot; the alarm takes one once the log has
//! grown past lattice-db's threshold and leaves it alone otherwise.

use lattice_core::engine::collection::DurableEngine;
use lattice_core::error::LatticeResult;
use lattice_core::storage::LatticeStorage;

/// What one alarm did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlarmOutcome {
    /// A snapshot was written at this log position and the log before it truncated.
    Checkpointed(u64),
    /// The log has not grown past the threshold since the last checkpoint.
    NotDue,
}

/// Run from the Durable Object's `alarm()`. A failure loses nothing acknowledged: recovery
/// uses the previous snapshot and replays the log (lattice-db's guarantee), so the caller
/// logs it and lets the next alarm retry the same checkpoint.
pub async fn on_alarm<W: LatticeStorage, D: LatticeStorage>(
    engine: &mut DurableEngine<W, D>,
) -> LatticeResult<AlarmOutcome> {
    if !engine.should_checkpoint() {
        return Ok(AlarmOutcome::NotDue);
    }
    engine.checkpoint().await.map(AlarmOutcome::Checkpointed)
}
