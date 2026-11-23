//! What one applied change sends and stores: the Tasks it wrote, the viewers' `EVENT`s
//! (`feed`) and the significant events' `NOTIFY`s (design §4.6, §4.7).

use std::collections::BTreeMap;

use super::dispatch::task_key;
use super::feed::Author;
use super::tree::Change;
use super::{Effect, NodeCore, NodeRefusal, codec};

impl NodeCore {
    pub(super) fn apply_change(
        &mut self,
        change: Change,
        by: &Author,
        now_ms: u64,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        let mut effects = Vec::new();
        for id in &change.stored {
            if let Some(task) = self.tree.get(id) {
                effects.push(Effect::Put {
                    key: task_key(id),
                    value: codec::task_to_bytes(task)?,
                });
            }
        }
        for id in &change.deleted {
            effects.push(Effect::Delete { key: task_key(id) });
        }
        effects.extend(self.feed_change(&change, by)?);
        for event in &change.events {
            let Some(task) = self.tree.get(&event.task) else {
                continue;
            };
            let live: BTreeMap<_, _> = event
                .recipients
                .iter()
                .map(|p| (p.clone(), self.sessions_of(p, &task.project)))
                .collect();
            let sessions = |p: &sw_model::IdFp| live.get(p).cloned().unwrap_or_default();
            for (to, bytes) in self.notifier.fan_out(event, task, &sessions, now_ms)? {
                effects.push(Effect::Send { to, bytes });
            }
        }
        Ok(effects)
    }
}
