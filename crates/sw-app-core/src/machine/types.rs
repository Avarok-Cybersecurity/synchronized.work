//! What the binding feeds the core and what the core asks of the host.

use sw_model::Cid;

use crate::command::DrawCommand;
use crate::ctx::Size;
use crate::fallback::HttpGet;
use crate::notify::HostNotify;
use crate::pace::Tier;
use crate::session::HostSessionStatus;

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// The `storage.user` value under `link::storage_key(node_id)` (`None`: absent).
    LinkLoaded {
        record: Option<Vec<u8>>,
    },
    StorageFailed {
        reason: String,
    },
    /// The `SessionOpen` reply.
    SessionOpened {
        cid: Cid,
        status: HostSessionStatus,
    },
    /// `SessionEvent::State`.
    SessionState {
        status: HostSessionStatus,
    },
    /// `SessionEvent::Message`.
    SessionMessage {
        body: Vec<u8>,
    },
    /// `SessionOpen` refused, or `AppSessionFailure { reason }` (e.g. `no_identity`).
    SessionOpenFailed {
        reason: String,
    },
    /// The `HostFetchOpen` stream, read to its end.
    HttpResponse {
        status: u16,
        body: Vec<u8>,
    },
    HttpFailed {
        reason: String,
    },
    /// `AppEvent::Activate { id }` (CW-5 hosts).
    Activate {
        id: String,
    },
    /// A primary-button pointer release at canvas coordinates (hit-tested on ABI-1 hosts).
    PointerUp {
        x: f32,
        y: f32,
    },
    /// `KeyEvent.key` of a key press.
    Key {
        key: String,
    },
    Frame {
        dt_ms: f32,
    },
    Resize {
        size: Size,
    },
    /// The host refused the last `CanvasDraw`: what it shows is the frame before.
    DrawRefused,
    /// The host showed the notification for this `event_id`.
    NotifyDelivered {
        event_id: [u8; 32],
    },
    /// The host refused the notification for this `event_id`.
    NotifyRefused {
        event_id: [u8; 32],
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    ReadUserStorage {
        key: String,
    },
    WriteUserStorage {
        key: String,
        value: Vec<u8>,
    },
    /// `SessionOpen { server }`; a second open of the same server returns the same session.
    SessionOpen {
        server: String,
    },
    /// `SessionSend` on the open session.
    SessionSend {
        body: Vec<u8>,
    },
    /// `HostFetchOpen` (`GET`, empty body), read with `NetStreamRead` to its end.
    HostFetchOpen(HttpGet),
    Notify(HostNotify),
    /// One `CanvasDraw`.
    Draw(Vec<DrawCommand>),
    RequestFrames {
        on: bool,
    },
    /// `ClipboardWriteText` (the activation is the user gesture it needs).
    CopyText {
        text: String,
    },
    /// A screen that is not the tree could not be drawn: a defect, for the host's log.
    Fault {
        reason: String,
    },
}

/// A notification's share of the budget: before draws, with sends.
pub(crate) const NOTIFY_TIER: Tier = Tier::Essential;

impl Effect {
    /// Which share of the host-call budget it takes ([`crate::pace`]).
    pub fn tier(&self) -> Tier {
        match self {
            Effect::Draw(_) | Effect::RequestFrames { on: false } => Tier::Low,
            Effect::RequestFrames { on: true } => Tier::Wake,
            Effect::Notify(_) => NOTIFY_TIER,
            _ => Tier::Essential,
        }
    }
}
