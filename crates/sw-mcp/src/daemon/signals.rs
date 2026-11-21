//! Signals through the gate into the inbox, out to agents, and their `ACK`s back; the tick
//! that flushes progress, expires signals and times out `await_signal`.

use std::collections::BTreeMap;

use sw_crypto::Kid;
use sw_model::Cid;
use sw_protocol::payload::AckResult;

use super::{AgentRec, Daemon, PROGRESS_FLUSH_MS, QueuedSignal, now_ms};
use crate::adapters::CitadelIdentity;
use crate::gate::{GateContext, Outcome, SignalKind};
use crate::ipc::Reply;
use crate::trust::kid_prefix;

/// The agent a signal goes to: the named one if attached, else one attached to its task.
fn target_agent(
    agents: &BTreeMap<String, AgentRec>,
    task: Option<&str>,
    agent: Option<&str>,
) -> Option<String> {
    if let Some(a) = agent.filter(|a| agents.contains_key(*a)) {
        return Some(a.to_owned());
    }
    agents
        .iter()
        .find(|(_, rec)| task.is_none_or(|t| rec.task == t))
        .map(|(id, _)| id.clone())
}

impl Daemon {
    pub(crate) fn ack(
        &mut self,
        to: Cid,
        signal_id: [u8; 32],
        result: AckResult,
        reason: Option<String>,
    ) {
        let Some(outbound) = &mut self.outbound else {
            return;
        };
        match outbound.ack(
            &self.keys.sign,
            to,
            signal_id,
            result,
            reason,
            now_ms(),
            &mut self.entropy,
        ) {
            Ok(bytes) => self.send(bytes),
            Err(e) => log::error!("ACK: {e:?}"),
        }
    }

    pub(crate) fn on_signal(&mut self, bytes: &[u8]) {
        let Some(own_cid) = self.cid else { return };
        let agents = &self.agents;
        let attached =
            |task: Option<&str>, agent: Option<&str>| target_agent(agents, task, agent).is_some();
        let ctx = GateContext {
            trust: &self.trust,
            kem: &self.keys.kem,
            identity: &CitadelIdentity,
            own_cid,
            now_ms: now_ms(),
            pending: self.inbox.len(),
            attached: &attached,
        };
        let outcome = self.gate.admit(bytes, &ctx);
        if let Err(e) = crate::store::save_replay_log(&self.home, self.gate.replay_log()) {
            log::error!("saving the replay log: {e}");
        }
        match outcome {
            Outcome::Admitted(a) => {
                let (task, agent) = match &a.kind {
                    SignalKind::Prompt(p) => (Some(p.task_id.as_str()), p.agent_id.as_deref()),
                    SignalKind::Control(_) => (None, None),
                };
                let Some(agent_id) = target_agent(&self.agents, task, agent) else {
                    log::error!(
                        "admitted signal {} has no agent any more",
                        super::hex(&a.signal_id[..4])
                    );
                    return;
                };
                let queued = QueuedSignal::new(&a, agent_id);
                let held = !queued.approved;
                if let Err(e) = self.inbox.put(queued) {
                    log::error!(
                        "could not queue signal {}: {e}",
                        super::hex(&a.signal_id[..4])
                    );
                    return;
                }
                log::info!(
                    "accepted signal {} from controller {}{}",
                    super::hex(&a.signal_id[..4]),
                    kid_prefix(&a.kid),
                    if held {
                        " (waiting for approval: sw-mcp gate approve)"
                    } else {
                        ""
                    }
                );
                self.ack(a.controller_cid, a.signal_id, AckResult::Delivered, None);
                self.wake_waiters();
            }
            Outcome::Refused(r) => {
                let kid = r.kid.map(|k| kid_prefix(&k)).unwrap_or_else(|| "-".into());
                log::warn!("refused a signal from {kid}: {}", r.reason.code());
                if let Some(to) = r.ack {
                    self.ack(
                        to.controller_cid,
                        to.signal_id,
                        AckResult::Refused,
                        Some(r.reason.code()),
                    );
                }
                if let (true, Some(k)) = (r.pause, r.kid) {
                    self.pause(&k);
                }
            }
        }
    }

    fn pause(&mut self, kid: &Kid) {
        if self.trust.get(kid).is_none_or(|c| c.paused) {
            return;
        }
        self.trust.pause(kid);
        log::warn!(
            "paused controller {} after repeated refusals; `sw-mcp trust` resumes it",
            kid_prefix(kid)
        );
        if let Err(e) = crate::store::save_trust(&self.home, &self.keys, &self.trust) {
            log::error!("saving the allowlist: {e}");
        }
    }

    /// Ends a queued signal: the agent executed or refused it, a person refused it, or it expired.
    pub(crate) fn finish(
        &mut self,
        signal_id: &str,
        result: AckResult,
        reason: Option<String>,
    ) -> Result<(), String> {
        let s = self
            .inbox
            .take(signal_id)?
            .ok_or_else(|| format!("no queued signal {signal_id}"))?;
        let id: [u8; 32] = data_encoding::HEXLOWER
            .decode(s.signal_id.as_bytes())
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or("a queued signal id is not 32 bytes of hex")?;
        self.ack(s.controller_cid, id, result, reason);
        Ok(())
    }

    /// Answers every `await_signal` whose agent now has something, or whose time is up.
    pub(crate) fn wake_waiters(&mut self) {
        let now = now_ms();
        let waiters = std::mem::take(&mut self.waiters);
        for w in waiters {
            let next = self.inbox.next_for(&w.agent_id, false).cloned();
            match next {
                Some(s) => {
                    let mut handed = s.clone();
                    handed.handed_out = true;
                    if let Err(e) = self.inbox.put(handed) {
                        let _ = w.reply.send(Reply::Error(e));
                        continue;
                    }
                    let _ = w.reply.send(Reply::Ok(serde_json::json!({
                        "signal_id": s.signal_id, "instruction": s.as_instruction(),
                    })));
                }
                None if now >= w.deadline_ms => {
                    let _ = w
                        .reply
                        .send(Reply::Ok(serde_json::json!({ "signal_id": null })));
                }
                None => self.waiters.push(w),
            }
        }
    }

    pub(crate) fn on_tick(&mut self) {
        let now = now_ms();
        for id in self.inbox.expired(now) {
            if let Err(e) = self.finish(&id, AckResult::Expired, None) {
                log::error!("expiring a signal: {e}");
            }
        }
        self.gate.prune(now);
        if now.saturating_sub(self.last_flush_ms) >= PROGRESS_FLUSH_MS {
            self.last_flush_ms = now;
            self.flush_progress(now);
        }
        self.wake_waiters();
        if self.pad.prune(now) {
            self.save_pad();
        }
        self.wake_pad();
        self.expire_onboard();
    }

    fn flush_progress(&mut self, now: u64) {
        let batches = std::mem::take(&mut self.batches);
        let Some(outbound) = &mut self.outbound else {
            self.batches = batches;
            return;
        };
        let mut out = Vec::new();
        for (agent_id, events) in batches.into_iter().filter(|(_, e)| !e.is_empty()) {
            match outbound.progress(&self.keys.sign, &agent_id, events, now, &mut self.entropy) {
                Ok(bytes) => out.push(bytes),
                Err(e) => log::error!("PROGRESS_EVENT for {agent_id}: {e:?}"),
            }
        }
        for bytes in out {
            self.send(bytes);
        }
    }
}
