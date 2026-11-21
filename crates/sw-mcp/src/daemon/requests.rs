//! The socket's requests, answered from the daemon's task.

use serde_json::{Value, json};
use sw_model::Lane;
use sw_model::readiness::Blocker;
use sw_protocol::payload::{AckResult, ProgressItem, limits};
use tokio::sync::oneshot;

use super::admin::{blocker, lane};
use super::{AgentRec, Daemon, Waiter, now_ms};
use crate::frame::{MAX_ITEMS, frame, sanitize};
use crate::ipc::{Reply, Request};
use crate::outbound::Outbound;

/// `await_signal` blocks for at most this long (design §9.1: `max_wait_s ≤ 25`).
pub const MAX_WAIT_S: u64 = 25;

impl Daemon {
    pub(crate) fn handle(&mut self, request: Request, reply: oneshot::Sender<Reply>) {
        if matches!(
            request,
            Request::PadWait { .. } | Request::PadPost { .. } | Request::PadMembers { .. }
        ) {
            return self.pad_deferred(request, reply);
        }
        if request.waits_for_the_node() {
            return self.onboard_deferred(request, reply);
        }
        if let Request::AwaitSignal {
            agent_id,
            max_wait_s,
        } = request
        {
            if !self.agents.contains_key(&agent_id) {
                let _ = reply.send(Reply::Error("call attach first".into()));
                return;
            }
            let wait = max_wait_s.min(MAX_WAIT_S);
            self.waiters.push(Waiter {
                agent_id,
                deadline_ms: now_ms() + wait * 1000,
                reply,
            });
            self.wake_waiters();
            return;
        }
        let answer = match self.answer(request) {
            Ok(v) => Reply::Ok(v),
            Err(e) => Reply::Error(e),
        };
        let _ = reply.send(answer);
    }

    fn agent(&self, agent_id: &str) -> Result<AgentRec, String> {
        self.agents
            .get(agent_id)
            .cloned()
            .ok_or_else(|| "call attach first".to_owned())
    }

    fn status_now(
        &mut self,
        agent_id: &str,
        lane: Lane,
        blockers: Vec<Blocker>,
    ) -> Result<(), String> {
        let rec = self.agent(agent_id)?;
        let harness = format!("{:?}", self.config.harness).to_ascii_lowercase();
        let outbound = self
            .outbound
            .as_mut()
            .ok_or("not connected to the node yet")?;
        let bytes = outbound
            .status(
                &self.keys.sign,
                agent_id,
                &rec.task,
                lane,
                blockers,
                &harness,
                now_ms(),
                &mut self.entropy,
            )
            .map_err(|e| format!("{e:?}"))?;
        self.send(bytes);
        Ok(())
    }

    fn answer(&mut self, request: Request) -> Result<Value, String> {
        match request {
            Request::WhoAmI => Ok(self.whoami()),
            Request::Attach {
                agent_id,
                project,
                task,
                cwd,
            } => {
                self.signed_in
                    .as_ref()
                    .ok_or("not signed in: call sign_in_start")?;
                let since = self.projection.cursor(&project);
                self.agents.insert(
                    agent_id.clone(),
                    AgentRec {
                        project: project.clone(),
                        task,
                        cwd,
                    },
                );
                if let Err(e) = self.status_now(&agent_id, Lane::Running, Vec::new()) {
                    self.agents.remove(&agent_id);
                    return Err(e);
                }
                self.send(Outbound::subscribe(&project, since).map_err(|e| format!("{e:?}"))?);
                Ok(json!({ "agent_id": agent_id }))
            }
            Request::PostStatus {
                agent_id,
                lane: l,
                blockers,
            } => {
                let blockers = blockers
                    .into_iter()
                    .map(blocker)
                    .collect::<Result<Vec<_>, _>>()?;
                self.status_now(&agent_id, lane(&l)?, blockers)?;
                Ok(json!({ "posted": true }))
            }
            Request::PostProgress {
                agent_id,
                phase,
                percent,
                line,
            } => {
                self.agent(&agent_id)?;
                let mut parts = Vec::new();
                if let Some(p) = phase {
                    parts.push(format!("phase: {p}"));
                }
                if let Some(p) = percent {
                    parts.push(format!("{}%", p.min(100)));
                }
                if let Some(l) = line {
                    parts.push(l);
                }
                let body = sanitize(&parts.join(" · "));
                let batch = self.batches.entry(agent_id).or_default();
                if batch.len() >= limits::PROGRESS_EVENTS_PER_BATCH {
                    return Err("64 progress lines are waiting to be sent; post less often".into());
                }
                batch.push(ProgressItem {
                    at: now_ms(),
                    kind: "progress".into(),
                    body,
                });
                Ok(json!({ "queued": true }))
            }
            Request::ReadFeed {
                agent_id,
                task,
                since,
                limit,
            } => {
                let rec = self.agent(&agent_id)?;
                let own = self
                    .signed_in
                    .as_ref()
                    .map(|s| s.id_fp.as_str().to_owned())
                    .unwrap_or_default();
                let items =
                    self.projection
                        .feed(&rec.project, &task, &own, since, limit.min(MAX_ITEMS));
                let next = since + items.len();
                log::info!(
                    "read_task_feed: {} items of {task} for {agent_id}",
                    items.len()
                );
                Ok(
                    json!({ "frame": frame(&items).map_err(|e| e.to_string())?, "next_since": next }),
                )
            }
            Request::AwaitSignal { .. } => Err("await_signal is answered by handle".into()),
            Request::AckSignal {
                agent_id,
                signal_id,
                executed,
                reason,
            } => {
                let s = self.inbox.get(&signal_id).ok_or("no such signal")?;
                if s.agent_id != agent_id || !s.handed_out {
                    return Err("that signal was not handed to this agent".into());
                }
                let (result, reason) = if executed {
                    (AckResult::Executed, None)
                } else {
                    (
                        AckResult::Refused,
                        Some(reason.unwrap_or_else(|| "agent_refused".into())),
                    )
                };
                self.finish(&signal_id, result, reason)?;
                Ok(json!({ "acked": true }))
            }
            Request::NextHookPrompt {
                cwd,
                interrupt_only,
            } => self.hook_prompt(&cwd, interrupt_only),
            r @ (Request::ProjectStatus { .. }
            | Request::TaskStatus { .. }
            | Request::AgentStatus { .. }
            | Request::ProgressTail { .. }) => self.query(r),
            Request::GateList => Ok(json!(self.inbox.all().collect::<Vec<_>>())),
            Request::GateApprove { signal_id } => {
                let mut s = self
                    .inbox
                    .get(&signal_id)
                    .cloned()
                    .ok_or("no such signal")?;
                s.approved = true;
                self.inbox.put(s)?;
                self.wake_waiters();
                Ok(json!({ "approved": signal_id }))
            }
            Request::GateRefuse { signal_id } => {
                self.finish(&signal_id, AckResult::Refused, Some("owner_refused".into()))?;
                Ok(json!({ "refused": signal_id }))
            }
            Request::Reload => self.reload(),
            r @ (Request::PadInbox { .. }
            | Request::PadAck { .. }
            | Request::PadPolicy { .. }
            | Request::PadTrust { .. }) => self.pad_answer(r),
            Request::ProjectList => Ok(self.project_list()),
            r
            @ (Request::PadWait { .. } | Request::PadPost { .. } | Request::PadMembers { .. }) => {
                Err(format!("answered by handle: {r:?}"))
            }
            r if r.waits_for_the_node() => Err("answered by handle".into()),
            r => Err(format!("not a daemon request: {r:?}")),
        }
    }
}
