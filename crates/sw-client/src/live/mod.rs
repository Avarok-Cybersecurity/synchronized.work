//! The viewer's side of a Project subscription (design §4.6): `HELLO` as a viewer, then
//! `SUBSCRIBE` from the last applied cursor, then every message the node sends folded into a
//! [`ProjectState`]. After a reconnect the same [`LiveProject`] subscribes again and resumes
//! from its cursor; an `EVENT` already applied is [`Folded::Stale`], never applied twice.

mod state;

pub use state::{AgentView, Folded, PersonView, ProjectState, TaskView};

use sw_protocol::WireError;
use sw_protocol::payload::{Hello, HelloKind, Payload, Subscribe, SubscribeOp};
use sw_protocol::signed::SwMessage;

use crate::{SessionError, SessionPort};

/// The `HELLO.protocol_version` this client speaks (sw-wire §3, typ 1).
pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveError {
    Session(SessionError),
    /// The node sent bytes that are not an `SwMessage` this client accepts.
    Wire(WireError),
}

impl From<SessionError> for LiveError {
    fn from(e: SessionError) -> Self {
        Self::Session(e)
    }
}

impl From<WireError> for LiveError {
    fn from(e: WireError) -> Self {
        Self::Wire(e)
    }
}

#[derive(Debug, Clone)]
pub struct LiveProject {
    state: ProjectState,
    client_build: String,
}

impl LiveProject {
    /// `client_build` is the build string `HELLO` reports (the site's commit).
    pub fn new(project_id: &str, client_build: &str) -> Self {
        Self {
            state: ProjectState::new(project_id),
            client_build: client_build.to_owned(),
        }
    }

    pub fn state(&self) -> &ProjectState {
        &self.state
    }

    /// `HELLO` then `SUBSCRIBE` since the cursor. Call on every (re)connect.
    pub fn subscribe(&self, port: &mut dyn SessionPort) -> Result<(), LiveError> {
        let hello = Payload::Hello(Hello {
            protocol_version: PROTOCOL_VERSION,
            endpoint_kind: HelloKind::Viewer,
            client_build: self.client_build.clone(),
        });
        let subscribe = Payload::Subscribe(Subscribe {
            op: SubscribeOp::Subscribe,
            project_id: self.state.project_id.clone(),
            since_cursor: self.state.cursor,
        });
        for payload in [hello, subscribe] {
            port.send(SwMessage::Unsigned(payload).to_bytes()?)?;
        }
        Ok(())
    }

    /// Folds everything that has arrived; returns how many messages changed the state. A
    /// malformed message stops the pump with its error (never skipped silently).
    pub fn pump(&mut self, port: &mut dyn SessionPort) -> Result<usize, LiveError> {
        let mut applied = 0;
        while let Some(bytes) = port.poll_recv()? {
            if self.state.fold(&SwMessage::from_bytes(&bytes)?)? == Folded::Applied {
                applied += 1;
            }
        }
        Ok(applied)
    }
}
