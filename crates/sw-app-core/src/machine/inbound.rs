//! What arrives from the node, on either transport: each message is decoded once, then a
//! `NOTIFY` becomes a host notification (and nothing else), the link's `PAD_RECEIPT` settles
//! the link screen, and everything else goes to the one fold (`LiveProject::apply`).

use sw_client::SessionPort;
use sw_client::live::Folded;
use sw_protocol::payload::Payload;
use sw_protocol::signed::SwMessage;

use super::{App, Effect};
use crate::link;
use crate::painter::TextMeasure;
use crate::screen::{AppError, EmptyReason, Screen};
use crate::transport::Transport;

/// Everything waiting on `port`, decoded; the first undecodable message stops the drain.
fn drain(port: &mut dyn SessionPort) -> Result<Vec<SwMessage>, AppError> {
    let mut out = Vec::new();
    loop {
        match port.poll_recv() {
            Ok(Some(bytes)) => out.push(SwMessage::from_bytes(&bytes).map_err(|_| AppError::Wire)?),
            Ok(None) => return Ok(out),
            Err(e) => {
                return Err(AppError::Receive {
                    reason: format!("{e:?}"),
                });
            }
        }
    }
}

impl<M: TextMeasure> App<M> {
    pub(super) fn session_message(&mut self, body: Vec<u8>, out: &mut Vec<Effect>) {
        if self.session.message(body).is_err() {
            return self.fail(AppError::Wire);
        }
        match drain(&mut self.session) {
            Ok(messages) => self.apply(messages, out),
            Err(e) => self.fail(e),
        }
    }

    pub(super) fn http_response(&mut self, status: u16, body: &[u8], out: &mut Vec<Effect>) {
        if self.transport != Some(Transport::Fallback) {
            return;
        }
        if status != 200 {
            return self.fail(AppError::Http { status });
        }
        if self.fallback.response(body).is_err() {
            return self.fail(AppError::Wire);
        }
        match drain(&mut self.fallback) {
            Ok(messages) => self.apply(messages, out),
            Err(e) => return self.fail(e),
        }
        if !matches!(self.screen, Screen::Error(_)) {
            self.show_live();
            self.poll(out);
        }
    }

    fn apply(&mut self, messages: Vec<SwMessage>, out: &mut Vec<Effect>) {
        let mut changed = false;
        for message in messages {
            match &message {
                SwMessage::Unsigned(Payload::Notify(n)) => {
                    if self.ctx.granted.notifications
                        && let Some(note) = self.notify.map(n, &self.ctx.node_id)
                    {
                        out.push(Effect::Notify(note));
                    }
                    continue;
                }
                SwMessage::Unsigned(Payload::PadReceipt(r)) => {
                    let pending = match &self.screen {
                        Screen::NeedsLink(l) => l.pending,
                        _ => None,
                    };
                    if let Some(outcome) = pending.and_then(|id| link::outcome(r, &id)) {
                        self.linked(outcome, out);
                        continue;
                    }
                }
                _ => {}
            }
            let Some(live) = &mut self.live else { continue };
            match live.apply(&message) {
                Ok(Folded::Applied) => changed = true,
                Ok(Folded::Ignored | Folded::Stale) => {}
                Err(_) => return self.fail(AppError::Wire),
            }
        }
        if changed {
            self.show_live();
        }
    }

    /// The tree, or the empty state while no Task is visible; the graph follows the fold.
    pub(super) fn show_live(&mut self) {
        let Some(live) = &self.live else { return };
        let state = live.state();
        if state.tasks.is_empty() {
            return self.set(Screen::Empty(EmptyReason::NothingYet));
        }
        match self.graph.update(state, &self.ctx.theme) {
            Ok(_) => {
                // `Lite` lands on the new layout at once; `Full` starts the spring.
                self.graph.step(0.0, self.ctx.theme.motion);
                self.set(Screen::Live);
                self.dirty = true;
            }
            Err(e) => self.fail(AppError::Tree {
                reason: format!("{e:?}"),
            }),
        }
    }

    pub(super) fn frame_tick(&mut self, dt_ms: f32) {
        if self.screen == Screen::Live && self.graph.is_moving() {
            self.graph.step(dt_ms, self.ctx.theme.motion);
            self.dirty = true;
        }
    }
}
