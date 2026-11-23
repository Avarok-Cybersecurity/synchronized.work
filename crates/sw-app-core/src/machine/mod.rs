//! The App as one state machine: host inputs in, [`Effect`]s out. The binding turns each
//! `AppEvent` / reply into an [`Input`], calls [`App::handle`], and performs the effects in
//! order with the SDK; it decides nothing itself.

mod actions;
mod draw;
mod flow;
mod inbound;

use sw_client::live::LiveProject;
use sw_model::Cid;

use crate::command::DrawCommand;
use crate::ctx::{AppConfig, AppCtx, Size};
use crate::fallback::{FallbackPort, HttpGet};
use crate::graph::GraphView;
use crate::link::LinkRecord;
use crate::notify::{HostNotify, NotifyMap};
use crate::painter::{Frame, TextMeasure};
use crate::screen::Screen;
use crate::screens::Composer;
use crate::session::{HostSessionStatus, PushedSession};
use crate::transport::Transport;

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

#[derive(Debug)]
pub struct App<M> {
    ctx: AppCtx,
    config: AppConfig,
    measure: M,
    screen: Screen,
    transport: Option<Transport>,
    session: PushedSession,
    fallback: FallbackPort,
    /// This attempt's open was refused with `no_identity` (until the next attempt).
    no_identity: bool,
    /// Whether this session's `HELLO` (and `SUBSCRIBE`) went out since it last connected.
    greeted: bool,
    link: Option<Option<LinkRecord>>,
    live: Option<LiveProject>,
    notify: NotifyMap,
    graph: GraphView,
    composer: Composer,
    frame: Option<Frame>,
    animating: bool,
    dirty: bool,
}

impl<M: TextMeasure> App<M> {
    pub fn new(ctx: AppCtx, config: AppConfig, measure: M) -> Self {
        Self {
            session: PushedSession::new(config.session),
            notify: NotifyMap::new(config.notify_memory),
            ctx,
            config,
            measure,
            screen: Screen::Connecting,
            transport: None,
            fallback: FallbackPort::new(),
            no_identity: false,
            greeted: false,
            link: None,
            live: None,
            graph: GraphView::new(),
            composer: Composer::ReadOnly,
            frame: None,
            animating: false,
            dirty: true,
        }
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    pub fn ctx(&self) -> &AppCtx {
        &self.ctx
    }

    pub fn transport(&self) -> Option<Transport> {
        self.transport
    }

    pub fn composer(&self) -> Composer {
        self.composer
    }

    /// The folded Project, once one is subscribed.
    pub fn live(&self) -> Option<&LiveProject> {
        self.live.as_ref()
    }

    /// The last frame drawn.
    pub fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    /// After `app_init`: choose the transport, read the saved link, open the session.
    pub fn start(&mut self) -> Vec<Effect> {
        let mut out = Vec::new();
        self.begin(&mut out);
        self.finish(out)
    }

    pub fn handle(&mut self, input: Input) -> Vec<Effect> {
        let mut out = Vec::new();
        match input {
            Input::LinkLoaded { record } => self.link_loaded(record, &mut out),
            Input::StorageFailed { reason } => {
                self.fail(crate::screen::AppError::Storage { reason })
            }
            Input::SessionOpened { cid, status } => {
                self.session.opened(cid, status.clone());
                self.on_status(status, &mut out);
            }
            Input::SessionState { status } => {
                self.session.status(status.clone());
                self.on_status(status, &mut out);
            }
            Input::SessionMessage { body } => self.session_message(body, &mut out),
            Input::SessionOpenFailed { reason } => self.open_failed(&reason, &mut out),
            Input::HttpResponse { status, body } => self.http_response(status, &body, &mut out),
            Input::HttpFailed { reason } => {
                self.fail(crate::screen::AppError::HttpFailed { reason })
            }
            Input::Activate { id } => self.activate(&id, &mut out),
            Input::PointerUp { x, y } => self.pointer_up(x, y, &mut out),
            Input::Key { key } => self.key(&key, &mut out),
            Input::Frame { dt_ms } => self.frame_tick(dt_ms),
            Input::Resize { size } => {
                self.ctx.size = size;
                self.dirty = true;
            }
        }
        self.finish(out)
    }

    /// Sends queue before the frame that shows their result.
    fn finish(&mut self, mut out: Vec<Effect>) -> Vec<Effect> {
        out.extend(
            self.session
                .take_outbox()
                .into_iter()
                .map(|body| Effect::SessionSend { body }),
        );
        if std::mem::take(&mut self.dirty) {
            self.draw(&mut out);
        }
        let want = self.screen == Screen::Live && self.graph.is_moving();
        if want != self.animating {
            self.animating = want;
            out.push(Effect::RequestFrames { on: want });
        }
        out
    }
}
