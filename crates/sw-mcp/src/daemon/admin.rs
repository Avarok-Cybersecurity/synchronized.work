//! The socket's queries and housekeeping: status views, the hook's next prompt, reload
//! after `trust` / `revoke` / sign-in, and `whoami`.

use serde_json::{Value, json};
use sw_model::Lane;
use sw_model::readiness::{Blocker, BlockerKind};
use sw_protocol::payload::AckResult;

use super::Daemon;
use crate::frame::MAX_ITEMS;
use crate::ipc::{BlockerIn, Request};
use crate::trust::{fingerprint_words, kid_prefix};

pub(super) fn lane(name: &str) -> Result<Lane, String> {
    Ok(match name {
        "running" => Lane::Running,
        "waiting" => Lane::Waiting,
        "blocked" => Lane::Blocked,
        "done" => Lane::Done,
        "stalled" => Lane::Stalled,
        _ => {
            return Err(format!(
                "lane {name:?} is not running|waiting|blocked|done|stalled"
            ));
        }
    })
}

pub(super) fn blocker(b: BlockerIn) -> Result<Blocker, String> {
    let kind = match b.kind.as_str() {
        "human" => BlockerKind::Human,
        "dependency" => BlockerKind::Dependency,
        "lock" => BlockerKind::Lock,
        "disk" => BlockerKind::Disk,
        "rate_limit" => BlockerKind::RateLimit,
        other => return Err(format!("blocker kind {other:?} is unknown")),
    };
    if b.text.len() > Blocker::MAX_TEXT_BYTES
        || b.task_id.is_some() != (kind == BlockerKind::Dependency)
    {
        return Err(
            "a blocker's text is at most 280 bytes, and task_id is given iff dependency".into(),
        );
    }
    Ok(Blocker {
        kind,
        text: b.text,
        task_id: b.task_id,
    })
}

impl Daemon {
    pub(super) fn query(&self, request: Request) -> Result<Value, String> {
        match request {
            Request::ProjectStatus { project } => Ok(json!(self.projection.project(&project))),
            Request::TaskStatus { project, task } => {
                let p = self
                    .projection
                    .project(&project)
                    .ok_or("no events from that project yet")?;
                let agents: Vec<_> = p.agents.iter().filter(|(_, a)| a.task_id == task).collect();
                Ok(json!({ "task": p.tasks.get(&task), "agents": agents }))
            }
            Request::AgentStatus { agent_id } => {
                let found = self
                    .agents
                    .get(&agent_id)
                    .and_then(|rec| self.projection.project(&rec.project))
                    .and_then(|p| p.agents.get(&agent_id));
                Ok(json!({ "attached_here": self.agents.contains_key(&agent_id), "agent": found }))
            }
            Request::ProgressTail {
                project,
                task,
                limit,
            } => {
                let p = self
                    .projection
                    .project(&project)
                    .ok_or("no events from that project yet")?;
                let tails: Vec<Value> = p
                    .agents
                    .iter()
                    .filter(|(_, a)| task.as_ref().is_none_or(|t| &a.task_id == t))
                    .map(|(id, a)| {
                        let skip = a.progress.len().saturating_sub(limit.min(MAX_ITEMS));
                        json!({ "agent_id": id, "task_id": a.task_id, "tail": a.tail,
                                "lines": a.progress.iter().skip(skip).collect::<Vec<_>>() })
                    })
                    .collect();
                Ok(json!({ "untrusted": true, "agents": tails }))
            }
            _ => Err("not a status query".into()),
        }
    }

    pub(super) fn hook_prompt(&mut self, cwd: &str, interrupt_only: bool) -> Result<Value, String> {
        let Some(agent_id) = self
            .agents
            .iter()
            .find(|(_, r)| r.cwd == cwd)
            .map(|(id, _)| id.clone())
        else {
            return Ok(json!({ "signal_id": null }));
        };
        let Some(mut s) = self.inbox.next_for(&agent_id, interrupt_only).cloned() else {
            return Ok(json!({ "signal_id": null }));
        };
        s.handed_out = true;
        self.inbox.put(s.clone())?;
        Ok(json!({ "signal_id": s.signal_id, "instruction": s.as_instruction() }))
    }

    pub(super) fn reload(&mut self) -> Result<Value, String> {
        let trust = crate::store::load_trust(&self.home, &self.keys)?;
        for c in trust.entries().iter().filter(|c| !c.paused) {
            self.gate.forgive(&c.kid);
        }
        self.trust = trust;
        let fingerprints: Vec<String> = self
            .trust
            .entries()
            .iter()
            .map(|c| fingerprint_words(&c.kid))
            .collect();
        let orphaned: Vec<String> = self
            .inbox
            .all()
            .filter(|s| !fingerprints.contains(&s.controller_fingerprint))
            .map(|s| s.signal_id.clone())
            .collect();
        for id in orphaned {
            self.finish(&id, AckResult::Refused, Some("controller_revoked".into()))?;
        }
        if self.signed_in.is_none() {
            self.load_sign_in();
        }
        self.ensure_session()?;
        Ok(self.whoami())
    }

    pub(super) fn whoami(&self) -> Value {
        let window_ms = self.config.approval_mode_new_controller_hours * 3_600_000;
        let trusted: Vec<Value> = self.trust.entries().iter().map(|c| json!({
            "kid": kid_prefix(&c.kid), "label": c.label, "fingerprint": fingerprint_words(&c.kid),
            "cid": c.cid, "paused": c.paused, "approval_mode_until_ms": c.trusted_at_ms + window_ms,
        })).collect();
        json!({
            "signed_in": self.signed_in.as_ref().map(|s| s.id_fp.as_str()),
            "assertion_exp_s": self.signed_in.as_ref().map(|s| s.exp_s),
            "device": self.keys.device_label(&self.config.device_slug),
            "device_cid": self.cid,
            "agents": self.agents.keys().collect::<Vec<_>>(),
            "trusted_controllers": trusted,
            "approval_mode_new_controller_hours": self.config.approval_mode_new_controller_hours,
            "harness": format!("{:?}", self.config.harness).to_ascii_lowercase(),
        })
    }
}
