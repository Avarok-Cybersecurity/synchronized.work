//! What the node answers the onboarding path: receipts (which complete or refuse a waiting
//! request), the `JOIN_PENDING` list, and the `NOTIFY`s that wake an agent — a join request
//! for its owner, an admission for both sides.

use data_encoding::HEXLOWER;
use serde_json::{Value, json};
use sw_protocol::payload::{JoinPending, Notify, PadReceipt};

use super::onboard::{Awaited, OnboardWait};
use super::{Daemon, now_ms};
use crate::home::{read_private, write_private};
use crate::ipc::Reply;
use crate::pad::state::EventKind;
use crate::projects::{Known, Projects};

pub(super) fn load_projects(home: &crate::home::Home) -> Result<Projects, String> {
    match read_private(&home.projects()).map_err(|e| e.to_string())? {
        Some(bytes) => Projects::from_json(&bytes),
        None => Ok(Projects::default()),
    }
}

impl Daemon {
    fn record_project(&mut self, id: &str, known: Known) {
        self.projects.record(id, known);
        match self.projects.to_json() {
            Ok(bytes) => {
                if let Err(e) = write_private(&self.home.projects(), &bytes) {
                    log::error!("saving projects.json: {e}");
                }
            }
            Err(e) => log::error!("{e}"),
        }
    }

    pub(super) fn project_list(&self) -> Value {
        json!({ "projects": self.projects.projects })
    }

    /// A receipt for one of the onboarding path's writes; `false` if it is not one of them.
    pub(super) fn onboard_receipt(&mut self, r: &PadReceipt) -> bool {
        let id = HEXLOWER.encode(&r.id);
        let Some(at) = self
            .onboard
            .iter()
            .position(|w| matches!(&w.awaited, Awaited::Receipts { ids, .. } if ids.contains(&id)))
        else {
            return false;
        };
        let mut wait = self.onboard.remove(at);
        if !r.stored {
            let why = r.reason.clone().unwrap_or_default();
            log::warn!("the node refused {id} (typ {}): {why}", r.typ);
            let _ = wait
                .reply
                .send(Reply::Error(format!("the node refused it: {why}")));
            return true;
        }
        if let Awaited::Receipts { ids, answer, known } = &mut wait.awaited {
            ids.retain(|i| *i != id);
            if !ids.is_empty() {
                self.onboard.insert(at, wait);
                return true;
            }
            if let Some((project, k)) = known.take() {
                self.record_project(&project, k);
            }
            let _ = wait.reply.send(Reply::Ok(answer.take()));
        }
        true
    }

    pub(super) fn on_join_pending(&mut self, j: JoinPending) {
        let project = j.project.clone();
        let requests: Vec<Value> = j
            .requests
            .unwrap_or_default()
            .into_iter()
            .map(|r| {
                let words = super::onboard::words(r.person.as_str());
                json!({ "request_id": HEXLOWER.encode(&r.request_id), "project": r.project,
                    "project_name": self.projects.name_of(&r.project), "task_id": r.task_id,
                    "person": r.person.as_str(), "display_as_the_node_reports": crate::frame::sanitize(&r.display),
                    "fingerprint": words, "at": r.at })
            })
            .collect();
        let answer = json!({ "requests": requests,
            "answer_with": "sw-mcp join accept <request_id> | sw-mcp join refuse <request_id>" });
        for wait in std::mem::take(&mut self.onboard) {
            if matches!(&wait.awaited, Awaited::Pending(p) if *p == project) {
                let _ = wait.reply.send(Reply::Ok(answer.clone()));
            } else {
                self.onboard.push(wait);
            }
        }
    }

    /// A join request for this Person's project, or an admission, wakes its agents.
    pub(super) fn on_notify(&mut self, n: &Notify) {
        let kind = match n.kind.as_str() {
            "join_requested" => EventKind::JoinRequested,
            "person_joined_task" => EventKind::Joined,
            _ => return,
        };
        let id = HEXLOWER.encode(&n.event_id);
        let text = crate::frame::sanitize(&n.text);
        let exp = now_ms() + sw_protocol::payload::pad::pad_limits::LIFETIME_MS;
        self.pad.notice_in(kind, &id, &n.subject, &text, exp);
        self.save_pad();
        self.wake_pad();
    }

    /// Answers every onboarding request whose time is up.
    pub(super) fn expire_onboard(&mut self) {
        let now = now_ms();
        let (late, live): (Vec<OnboardWait>, Vec<OnboardWait>) = std::mem::take(&mut self.onboard)
            .into_iter()
            .partition(|w| now >= w.deadline_ms);
        self.onboard = live;
        for w in late {
            let _ = w
                .reply
                .send(Reply::Error("the node did not answer in time".into()));
        }
    }
}
