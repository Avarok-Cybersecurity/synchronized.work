//! The `net.host` fallback (design §7.4: hosts without CW-6, or an open answered
//! `no_identity`): a long-poll of `/v1/projects/{id}/stream?cursor=&max_ms=` through
//! `HostFetchOpen`. [`FallbackPort`] is a [`SessionPort`] so the same
//! [`sw_client::live::LiveProject`] runs over it: its `SUBSCRIBE` names the project, and each
//! response's messages are folded exactly as session messages are. The cursor of every request
//! is the fold's own (SSOT), so a failed or empty poll repeats from where the fold stands and
//! no cursor is skipped.
//!
//! A response body is one canonical CBOR array of byte strings, each one encoded `SwMessage`.

use std::collections::VecDeque;

use sw_client::{SessionError, SessionPort, SessionState};
use sw_protocol::WireError;
use sw_protocol::payload::{Payload, SubscribeOp};
use sw_protocol::signed::SwMessage;
use sw_protocol::wire::{Bytes, Wire};

use crate::copy;
use crate::ctx::PollConfig;

/// One `HostFetchOpen` request: always `GET`, empty body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpGet {
    pub url: String,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// A header `net.host` never carries: `Upgrade`, `Connection` or any `Sec-*`.
    ForbiddenHeader(String),
    /// No `SUBSCRIBE` has named a project yet.
    NoProject,
}

/// `net.host` carries no WebSocket handshake (`host/limits.ts`): these never leave the App.
pub fn check_headers(headers: &[(String, String)]) -> Result<(), PlanError> {
    for (name, _) in headers {
        let lower = name.to_ascii_lowercase();
        if lower == "upgrade" || lower == "connection" || lower.starts_with("sec-") {
            return Err(PlanError::ForbiddenHeader(name.clone()));
        }
    }
    Ok(())
}

/// Percent-encodes everything outside RFC 3986's unreserved set (a path segment).
fn segment(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(b).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The request for the messages after `cursor` of `project_id`.
pub fn plan(config: &PollConfig, project_id: &str, cursor: u64) -> Result<HttpGet, PlanError> {
    let headers = vec![("Accept".to_owned(), "application/cbor".to_owned())];
    check_headers(&headers)?;
    Ok(HttpGet {
        url: format!(
            "{}/v1/projects/{}/stream?cursor={cursor}&max_ms={}",
            config.origin,
            segment(project_id),
            config.max_ms
        ),
        headers,
    })
}

/// The messages of one response body.
pub fn decode_body(body: &[u8]) -> Result<Vec<Vec<u8>>, WireError> {
    let items = Vec::<Bytes>::from_value(sw_protocol::cbor::decode(body)?)?;
    Ok(items.into_iter().map(|b| b.0).collect())
}

#[derive(Debug, Clone, Default)]
pub struct FallbackPort {
    project: Option<String>,
    inbound: VecDeque<Vec<u8>>,
}

impl FallbackPort {
    pub fn new() -> Self {
        Self::default()
    }

    /// The project the last `SUBSCRIBE` named.
    pub fn project(&self) -> Option<&str> {
        self.project.as_deref()
    }

    /// The next request, from the fold's `cursor`.
    pub fn next_request(&self, config: &PollConfig, cursor: u64) -> Result<HttpGet, PlanError> {
        plan(config, self.project().ok_or(PlanError::NoProject)?, cursor)
    }

    /// A `200` response's body: its messages wait for the next `poll_recv`.
    pub fn response(&mut self, body: &[u8]) -> Result<usize, WireError> {
        let messages = decode_body(body)?;
        let n = messages.len();
        self.inbound.extend(messages);
        Ok(n)
    }
}

impl SessionPort for FallbackPort {
    /// There is no Citadel session on this path, and so no CID.
    fn state(&self) -> SessionState {
        SessionState::Disconnected {
            reason: copy::FALLBACK_STATE.to_owned(),
        }
    }

    /// `HELLO` needs no request; `SUBSCRIBE` becomes the stream's project. Nothing else is
    /// carried: the fallback only reads.
    fn send(&mut self, sw_message: Vec<u8>) -> Result<(), SessionError> {
        let message = SwMessage::from_bytes(&sw_message)
            .map_err(|e| SessionError::Transport(format!("{e:?}")))?;
        match message {
            SwMessage::Unsigned(Payload::Hello(_)) => Ok(()),
            SwMessage::Unsigned(Payload::Subscribe(s)) if s.op == SubscribeOp::Subscribe => {
                self.project = Some(s.project_id);
                Ok(())
            }
            other => Err(SessionError::Transport(format!(
                "typ {} is not carried by the HTTPS fallback",
                match other {
                    SwMessage::Unsigned(p) => p.typ(),
                    SwMessage::Signed { typ, .. } => typ,
                }
            ))),
        }
    }

    fn poll_recv(&mut self) -> Result<Option<Vec<u8>>, SessionError> {
        Ok(self.inbound.pop_front())
    }
}
