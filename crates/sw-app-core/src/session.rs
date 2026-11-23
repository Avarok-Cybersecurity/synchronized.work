//! The `citadel.session` transport as a [`SessionPort`] (CW-6, `citadel-session-abi` §1). The
//! host pushes: the `SessionOpen` reply and `AppEvent::Session` events go in through
//! [`PushedSession::opened`], [`PushedSession::status`] and [`PushedSession::message`]; what
//! the client logic sends waits in an outbox the binding drains into `SessionSend` calls.
//! `limits.session.max_msg_bytes` is enforced both ways; `max_msgs_per_s` is the host's token
//! bucket (the App sends two messages per (re)connect and one per link attempt).

use std::collections::VecDeque;

use sw_client::{SessionError, SessionPort, SessionState};
use sw_model::Cid;

use crate::ctx::SessionLimits;

/// The contract's `SessionStatus`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostSessionStatus {
    Connecting,
    Connected,
    Closed { reason: String },
}

#[derive(Debug, Clone)]
pub struct PushedSession {
    limits: SessionLimits,
    cid: Option<Cid>,
    status: HostSessionStatus,
    inbound: VecDeque<Vec<u8>>,
    outbox: Vec<Vec<u8>>,
}

impl PushedSession {
    pub fn new(limits: SessionLimits) -> Self {
        Self {
            limits,
            cid: None,
            status: HostSessionStatus::Connecting,
            inbound: VecDeque::new(),
            outbox: Vec::new(),
        }
    }

    /// The `SessionOpen` reply: the agent-held account's CID on the server and its state.
    pub fn opened(&mut self, cid: Cid, status: HostSessionStatus) {
        self.cid = Some(cid);
        self.status = status;
    }

    /// `SessionEvent::State`.
    pub fn status(&mut self, status: HostSessionStatus) {
        self.status = status;
    }

    /// `SessionEvent::Message`: one encoded `SwMessage` from the server.
    pub fn message(&mut self, body: Vec<u8>) -> Result<(), SessionError> {
        if body.len() > self.limits.max_msg_bytes {
            return Err(SessionError::TooLarge {
                max: self.limits.max_msg_bytes,
            });
        }
        self.inbound.push_back(body);
        Ok(())
    }

    /// What to send, in order, each body one `SessionSend`.
    pub fn take_outbox(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.outbox)
    }
}

impl SessionPort for PushedSession {
    fn state(&self) -> SessionState {
        match (&self.status, self.cid) {
            (HostSessionStatus::Connected, Some(cid)) => SessionState::Connected { cid },
            (HostSessionStatus::Closed { reason }, _) => SessionState::Disconnected {
                reason: reason.clone(),
            },
            _ => SessionState::Connecting,
        }
    }

    fn send(&mut self, sw_message: Vec<u8>) -> Result<(), SessionError> {
        if !matches!(self.state(), SessionState::Connected { .. }) {
            return Err(SessionError::NotConnected);
        }
        if sw_message.len() > self.limits.max_msg_bytes {
            return Err(SessionError::TooLarge {
                max: self.limits.max_msg_bytes,
            });
        }
        self.outbox.push(sw_message);
        Ok(())
    }

    fn poll_recv(&mut self) -> Result<Option<Vec<u8>>, SessionError> {
        Ok(self.inbound.pop_front())
    }
}
