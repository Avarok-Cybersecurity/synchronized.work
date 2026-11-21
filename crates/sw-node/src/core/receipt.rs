//! What the onboarding path tells its writers: a `PAD_RECEIPT` for every write it applies or
//! refuses (types 8, 9, 10, 20, 23), and the `JOIN_PENDING` answer an owner reads requests from.

use sw_model::Cid;
use sw_protocol::body::Body;
use sw_protocol::payload::invite::invite_limits::PENDING_LISTED;
use sw_protocol::payload::{JoinPending, PadReceipt, Payload, PendingJoin};
use sw_protocol::signed::SwMessage;

use super::{Effect, NodeCore, NodeRefusal};

/// The writes whose writer is told whether they were stored.
pub const RECEIPTED: [u8; 5] = [8, 9, 10, 20, 23];

fn receipt(
    to: Cid,
    project: String,
    id: [u8; 32],
    typ: u8,
    refused: Option<&NodeRefusal>,
) -> Result<Effect, NodeRefusal> {
    let receipt = PadReceipt {
        project,
        id,
        typ,
        stored: refused.is_none(),
        reason: refused.map(|r| format!("{r:?}")),
    };
    let bytes = SwMessage::Unsigned(Payload::PadReceipt(receipt)).to_bytes()?;
    Ok(Effect::Send { to, bytes })
}

impl NodeCore {
    /// The project a receipted write names, read before it is applied: a new root Task is
    /// its own project; a request is named by the Task it waits on.
    pub(super) fn receipt_project(&self, payload: &Payload) -> Option<String> {
        let of_task = |id: &str| self.tree.get(id).map(|t| t.project.clone());
        Some(match payload {
            Payload::TaskWrite(w) => of_task(&w.task_id)
                .or_else(|| w.fields.parent.as_deref().and_then(of_task))
                .unwrap_or_else(|| w.task_id.clone()),
            Payload::JoinRequest(r) => of_task(&r.task_id).unwrap_or_else(|| r.task_id.clone()),
            Payload::JoinAnswer(a) => self
                .tree
                .tasks
                .values()
                .find(|t| t.requests.contains_key(&a.request_id))
                .map(|t| t.project.clone())
                .unwrap_or_default(),
            Payload::PadPolicy(p) => p.project.clone(),
            Payload::Invite(i) => i.project.clone(),
            _ => return None,
        })
    }

    /// The receipt for an applied write, sent to the session that wrote it.
    pub(super) fn stored_receipt(
        cid: Cid,
        project: String,
        id: [u8; 32],
        typ: u8,
    ) -> Result<Effect, NodeRefusal> {
        receipt(cid, project, id, typ, None)
    }

    /// What the adapter sends after `received` refused `bytes` from session `cid`: a receipt
    /// naming the reason, if the bytes are a signed write of a receipted type. Nothing changed.
    pub fn refusal_receipt(&self, cid: Cid, bytes: &[u8], refusal: &NodeRefusal) -> Vec<Effect> {
        let Ok(SwMessage::Signed { typ, signed }) = SwMessage::from_bytes(bytes) else {
            return Vec::new();
        };
        let (Ok(id), Ok(body)) = (signed.signal_id(), Body::from_bytes(&signed.body.0)) else {
            return Vec::new();
        };
        if !RECEIPTED.contains(&typ) {
            return Vec::new();
        }
        let project = self.receipt_project(&body.payload).unwrap_or_default();
        receipt(cid, project, id, typ, Some(refusal))
            .map(|e| vec![e])
            .unwrap_or_default()
    }

    /// The join requests waiting on the asking session's Person, in one project or in all.
    pub(super) fn join_pending(
        &self,
        cid: Cid,
        project: Option<String>,
    ) -> Result<Vec<Effect>, NodeRefusal> {
        let (person, _) = self
            .directory
            .endpoint(cid)
            .ok_or(NodeRefusal::UnknownPerson)?;
        let mut requests = Vec::new();
        let mine = self.tree.tasks.values().filter(|t| {
            t.coop.owner == *person && project.as_ref().is_none_or(|p| t.project == *p)
        });
        for task in mine {
            for (id, who) in &task.requests {
                let Some(r) = task.coop.pending.iter().find(|r| r.person == *who) else {
                    continue;
                };
                requests.push(PendingJoin {
                    request_id: *id,
                    project: task.project.clone(),
                    task_id: task.id.clone(),
                    person: who.clone(),
                    display: self
                        .directory
                        .person(who)
                        .map(|p| p.display.clone())
                        .unwrap_or_default(),
                    at: r.at_ms,
                });
            }
        }
        requests.sort_by_key(|r| r.at);
        requests.truncate(PENDING_LISTED);
        let answer = JoinPending {
            project,
            requests: Some(requests),
        };
        let bytes = SwMessage::Unsigned(Payload::JoinPending(answer)).to_bytes()?;
        Ok(vec![Effect::Send { to: cid, bytes }])
    }
}
