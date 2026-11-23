//! Creating a project, inviting someone and answering them (the onboarding path). Each request
//! signs its writes with this Device's key and is answered once the node's receipts (or its
//! `JOIN_PENDING` answer) arrive; a refusal reaches the caller with the node's reason.

use serde_json::{Value, json};
use sw_protocol::body::Audience;
use sw_protocol::invite::{InviteCode, token_hash};
use sw_protocol::payload::invite::invite_limits::MAX_LIFETIME_MS;
use sw_protocol::payload::limits;
use sw_protocol::payload::{
    Invite, JoinAnswer, JoinPending, JoinRequest, PadPolicy, Payload, TaskFields, TaskOp, TaskWrite,
};
use sw_protocol::signed::SwMessage;
use tokio::sync::oneshot;

use super::pad::{ASK_TIMEOUT_MS, hex32};
use super::pad_in::signal_hex;
use super::{Daemon, hex, now_ms};
use crate::ipc::{Reply, Request};
use crate::projects::{Known, Role};

/// What a waiting request needs from the node before it is answered.
pub(crate) enum Awaited {
    /// The receipts of these signal ids, all stored; then `answer`, recording `known`.
    Receipts {
        ids: Vec<String>,
        answer: Value,
        known: Option<(String, Known)>,
    },
    /// The node's `JOIN_PENDING` answer for this project filter.
    Pending(Option<String>),
}

pub(crate) struct OnboardWait {
    pub(crate) awaited: Awaited,
    pub(crate) deadline_ms: u64,
    pub(crate) reply: oneshot::Sender<Reply>,
}

/// The six words two people compare for an `id_fp` (as `scratchpad_members` shows them).
pub(super) fn words(id_fp: &str) -> String {
    crate::trust::fingerprint_words(&sw_crypto::sha3_256(id_fp.as_bytes()))
}

/// A join request's readiness: a person joining a project is ready now.
const JOIN_READINESS_PERMILLE: u16 = 1000;

impl Daemon {
    pub(super) fn onboard_deferred(&mut self, request: Request, reply: oneshot::Sender<Reply>) {
        let ready = self.online && self.signed_in.is_some() && self.outbound.is_some();
        let awaited = if ready {
            self.onboard_send(request)
        } else {
            Err("not connected to the node; try again shortly".into())
        };
        match awaited {
            Ok(awaited) => self.onboard.push(OnboardWait {
                awaited,
                deadline_ms: now_ms() + ASK_TIMEOUT_MS,
                reply,
            }),
            Err(e) => drop(reply.send(Reply::Error(e))),
        }
    }

    pub(super) fn signed(&mut self, payload: Payload) -> Result<(Vec<u8>, String), String> {
        let outbound = self.outbound.as_mut().ok_or("not connected to the node")?;
        let bytes = outbound
            .sign(
                &self.keys.sign,
                Audience::Node,
                payload,
                now_ms(),
                &mut self.entropy,
            )
            .map_err(|e| format!("{e:?}"))?;
        let id = signal_hex(&bytes)?;
        Ok((bytes, id))
    }

    fn onboard_send(&mut self, request: Request) -> Result<Awaited, String> {
        let me = self.own_id();
        let (writes, answer, known) = match request {
            Request::ProjectCreate { name } => {
                if name.is_empty() || name.len() > limits::TASK_TITLE_BYTES {
                    return Err(format!("a name is 1..={} bytes", limits::TASK_TITLE_BYTES));
                }
                let project = hex(&self.entropy.array::<16>());
                let create = Payload::TaskWrite(TaskWrite {
                    op: TaskOp::Create,
                    task_id: project.clone(),
                    fields: TaskFields {
                        title: Some(name.clone()),
                        visibility: Some(sw_model::Visibility::Members),
                        ..TaskFields::EMPTY
                    },
                });
                // Explicit, signed by the owner: the node assumes no retention (PCND).
                let policy = PadPolicy::for_new_project(project.clone());
                let hours = policy.retention_ms / 3_600_000;
                let known = Known {
                    name: name.clone(),
                    role: Role::Owner,
                    owner: me.clone(),
                    at: now_ms(),
                };
                let answer = json!({ "project": project, "name": name, "scratchpad_retention_hours": hours,
                    "next": "make an invite for each person: sw-mcp invite create <project> --valid-hours <1-168>" });
                let writes = vec![create, Payload::PadPolicy(policy)];
                (writes, answer, Some((project, known)))
            }
            Request::InviteCreate {
                project,
                valid_hours,
            } => {
                let max_hours = MAX_LIFETIME_MS / 3_600_000;
                if !(1..=max_hours).contains(&valid_hours) {
                    return Err(format!("valid_hours is 1..={max_hours}"));
                }
                let name = self
                    .projects
                    .name_of(&project)
                    .ok_or("not a project created on this device (see sw-mcp project list)")?
                    .to_owned();
                let token = self.entropy.array::<32>();
                let exp = now_ms() + valid_hours * 3_600_000;
                let owner = sw_model::IdFp::parse(&me).map_err(|_| "not signed in")?;
                let code = InviteCode {
                    project: project.clone(),
                    owner,
                    name,
                    exp,
                    token,
                };
                let invite = Payload::Invite(Invite {
                    project: project.clone(),
                    token_hash: token_hash(&token),
                    exp,
                });
                let code = code.encode().map_err(|e| format!("{e:?}"))?;
                let answer = json!({ "invite": code, "project": project, "expires_at": exp,
                    "single_use": true, "your_fingerprint": words(&me), "send_it_to": "one person, privately; anyone holding it can ask to join" });
                (vec![invite], answer, None)
            }
            Request::InviteRedeem { code, note } => {
                if note.len() > limits::JOIN_NOTE_BYTES {
                    return Err(format!(
                        "a note is at most {} bytes",
                        limits::JOIN_NOTE_BYTES
                    ));
                }
                let code = InviteCode::decode(&code).map_err(|e| format!("invite code: {e:?}"))?;
                if code.exp <= now_ms() {
                    return Err("this invite has expired; ask for a new one".into());
                }
                let join = Payload::JoinRequest(JoinRequest {
                    task_id: code.project.clone(),
                    note,
                    readiness: sw_model::readiness::Readiness::from_permille(
                        JOIN_READINESS_PERMILLE,
                    )
                    .ok_or("readiness")?,
                    agent_id: None,
                    invite: Some(code.token),
                });
                let known = Known {
                    name: code.name.clone(),
                    role: Role::Requested,
                    owner: code.owner.as_str().to_owned(),
                    at: now_ms(),
                };
                let answer = json!({ "requested": true, "project": code.project, "name": code.name,
                    "owner": code.owner.as_str(),
                    "owner_fingerprint": words(code.owner.as_str()), "your_fingerprint": words(&me),
                    "compare": "read your_fingerprint to the owner (they see it in sw-mcp join list) and check theirs against owner_fingerprint",
                    "next": "the owner approves you; then sw-mcp pad wait wakes with a `joined` event and scratchpad_members lists you both" });
                (vec![join], answer, Some((code.project, known)))
            }
            Request::JoinAnswer {
                request_id,
                accept,
                reason,
            } => {
                let answer = Payload::JoinAnswer(JoinAnswer {
                    request_id: hex32(&request_id)?,
                    accept,
                    reason,
                });
                let done = json!({ "request_id": request_id, "accepted": accept });
                (vec![answer], done, None)
            }
            Request::JoinRequests { project } => {
                let ask = SwMessage::Unsigned(Payload::JoinPending(JoinPending {
                    project: project.clone(),
                    requests: None,
                }));
                self.send(ask.to_bytes().map_err(|e| format!("{e:?}"))?);
                return Ok(Awaited::Pending(project));
            }
            Request::LinkCode { project } => return self.link_code_send(project),
            _ => return Err("not an onboarding request".into()),
        };
        let mut ids = Vec::new();
        let mut signed = Vec::new();
        for payload in writes {
            let (bytes, id) = self.signed(payload)?;
            ids.push(id);
            signed.push(bytes);
        }
        for bytes in signed {
            self.send(bytes);
        }
        Ok(Awaited::Receipts { ids, answer, known })
    }
}
