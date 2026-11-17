//! A [`SessionPort`] that plays back a fixed list of node messages: the site's demo mode runs
//! the same [`crate::live::LiveProject`] over it that a signed-in viewer runs over the real
//! Citadel session, so the demo exercises the production fold and nothing pretends to be live.

use std::collections::VecDeque;

use sw_model::Cid;

use crate::{SessionError, SessionPort, SessionState};

#[derive(Debug, Clone)]
pub struct ReplayPort {
    cid: Cid,
    frames: VecDeque<Vec<u8>>,
    sent: Vec<Vec<u8>>,
}

impl ReplayPort {
    pub fn new(cid: Cid, frames: Vec<Vec<u8>>) -> Self {
        Self {
            cid,
            frames: frames.into(),
            sent: Vec::new(),
        }
    }

    /// What the client sent, in order.
    pub fn sent(&self) -> &[Vec<u8>] {
        &self.sent
    }
}

impl SessionPort for ReplayPort {
    fn state(&self) -> SessionState {
        SessionState::Connected { cid: self.cid }
    }

    fn send(&mut self, sw_message: Vec<u8>) -> Result<(), SessionError> {
        self.sent.push(sw_message);
        Ok(())
    }

    fn poll_recv(&mut self) -> Result<Option<Vec<u8>>, SessionError> {
        Ok(self.frames.pop_front())
    }
}
