//! `sw-mcp link-code --project <id>`: a link code for the synchronized.work Citadel App
//! (design §5.3 "ships first"). The site holds no session to the node, so this Device is
//! where a signed-in Person mints one. The code is drawn here, from the OS random source, and
//! bound by the node to the Project this session subscribed to last: the daemon subscribes to
//! it (replaying only what its projection lacks), sends the signed `LINK_CODE`, and, if it was
//! not following that Project before, unsubscribes again.

use serde_json::json;
use sw_protocol::link_code::{generate, link_limits::LIFETIME_MS};
use sw_protocol::payload::{Link, LinkOp, Payload, Subscribe, SubscribeOp};
use sw_protocol::signed::SwMessage;

use super::onboard::Awaited;
use super::{Daemon, now_ms};

impl Daemon {
    pub(super) fn link_code_send(&mut self, project: String) -> Result<Awaited, String> {
        let following = self.agents.values().any(|a| a.project == project);
        let since = self.projection.cursor(&project);
        let subscribe = crate::outbound::Outbound::subscribe(&project, since);
        let subscribe = subscribe.map_err(|e| format!("{e:?}"))?;
        let code = generate(&mut self.entropy);
        let (mint, id) = self.signed(Payload::Link(Link {
            op: LinkOp::Code,
            code: code.clone(),
            acting_username: None,
            install_id: None,
            node_id: None,
        }))?;
        self.send(subscribe);
        self.send(mint);
        if !following {
            let leave = SwMessage::Unsigned(Payload::Subscribe(Subscribe {
                op: SubscribeOp::Unsubscribe,
                project_id: project.clone(),
                since_cursor: since,
            }));
            self.send(leave.to_bytes().map_err(|e| format!("{e:?}"))?);
        }
        let minutes = LIFETIME_MS / 60_000;
        let answer = json!({ "code": code, "project": project, "expires_at": now_ms() + LIFETIME_MS,
            "valid_minutes": minutes, "single_use": true,
            "next": format!("type it into the synchronized.work app in your Citadel workspace within {minutes} minutes") });
        Ok(Awaited::Receipts {
            ids: vec![id],
            answer,
            known: None,
        })
    }
}
