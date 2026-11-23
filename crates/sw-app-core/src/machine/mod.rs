//! The App as one state machine: host inputs in, [`Effect`]s out. The binding turns each
//! `AppEvent` / reply into an [`Input`], calls [`App::handle`], and performs the effects in
//! order with the SDK; it decides nothing itself.

mod actions;
mod draw;
mod finish;
mod flow;
mod inbound;
mod types;

pub use types::{Effect, Input};

use std::collections::VecDeque;

use sw_client::live::LiveProject;

use crate::ctx::{AppConfig, AppCtx};
use crate::fallback::FallbackPort;
use crate::graph::GraphView;
use crate::link::LinkRecord;
use crate::measure::{HostText, MeasureCache};
use crate::notify::NotifyMap;
use crate::notify_retry::NotifyQueue;
use crate::pace::Ledger;
use crate::painter::Frame;
use crate::screen::Screen;
use crate::screens::Composer;
use crate::session::PushedSession;
use crate::transport::Transport;

#[derive(Debug)]
pub struct App<M> {
    ctx: AppCtx,
    config: AppConfig,
    /// Every width, through one cache.
    text: MeasureCache<M>,
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
    notices: NotifyQueue,
    graph: GraphView,
    composer: Composer,
    /// The frame the host shows (hit-testing and `Activate` resolve against it).
    frame: Option<Frame>,
    /// The frame before the last one sent, while the host may still refuse that one.
    before: Option<Option<Frame>>,
    /// The host calls of the current window, and frame time.
    ledger: Ledger,
    /// Host calls decided but not yet within the budget, in order.
    deferred: VecDeque<Effect>,
    /// Whether frames are on.
    animating: bool,
    /// The screen changed since the last frame the host took.
    dirty: bool,
    /// The host refused the last draw: it is drawn again on the next frame.
    refused: bool,
}

impl<M: HostText> App<M> {
    pub fn new(ctx: AppCtx, config: AppConfig, measure: M) -> Self {
        Self {
            session: PushedSession::new(config.session),
            notify: NotifyMap::new(config.notify_memory),
            notices: NotifyQueue::new(config.notify_retry),
            text: MeasureCache::new(measure, config.measure),
            ledger: Ledger::new(config.host_calls),
            deferred: VecDeque::new(),
            before: None,
            refused: false,
            ctx,
            config,
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

    /// The last frame drawn (and not refused).
    pub fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    /// The widths measured so far.
    pub fn text(&self) -> &MeasureCache<M> {
        &self.text
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
            Input::DrawRefused => self.draw_refused(),
            Input::NotifyDelivered { event_id } => self.notices.delivered(&event_id),
            Input::NotifyRefused { event_id } => self.notify_refused(&event_id),
        }
        self.finish(out)
    }
}
