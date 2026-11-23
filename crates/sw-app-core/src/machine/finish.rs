//! What one input leaves for the host, within the App's host-call budget ([`crate::pace`]).
//! Host calls go out in the order they were decided; sends and notifications before the draw
//! that shows their result; a draw is built from the state as it is when it goes out, so a
//! deferred or refused draw is replaced by the latest one, never queued. Whatever waits (a
//! deferred call, a draw, a notification's next attempt) keeps frames on, because frames are
//! what move the App's clock and so its budget window.

use super::types::NOTIFY_TIER;
use super::{App, Effect};
use crate::measure::HostText;
use crate::notify::HostNotify;
use crate::notify_retry::GaveUp;
use crate::pace::Tier;
use crate::screen::Screen;

impl<M: HostText> App<M> {
    pub(super) fn finish(&mut self, out: Vec<Effect>) -> Vec<Effect> {
        self.deferred.extend(out);
        self.deferred.extend(
            self.session
                .take_outbox()
                .into_iter()
                .map(|body| Effect::SessionSend { body }),
        );
        let mut emitted = Vec::new();
        self.flush(&mut emitted);
        if self.dirty && self.ledger.room(Tier::Low) > 0 {
            self.draw(&mut emitted);
        }
        self.flush(&mut emitted);
        self.frames(&mut emitted);
        emitted
    }

    /// The deferred calls, then the notifications due, while the budget has room.
    fn flush(&mut self, out: &mut Vec<Effect>) {
        while let Some(effect) = self.deferred.front() {
            if self.ledger.room(effect.tier()) == 0 {
                return;
            }
            self.ledger.spend(1);
            out.extend(self.deferred.pop_front());
        }
        while self.ledger.room(NOTIFY_TIER) > 0 {
            let Some(note) = self.notices.due(self.ledger.now_ms()) else {
                break;
            };
            self.ledger.spend(1);
            out.push(Effect::Notify(note));
        }
    }

    fn frames(&mut self, out: &mut Vec<Effect>) {
        let want = (self.screen == Screen::Live && self.graph.is_moving())
            || self.dirty
            || self.refused
            || !self.deferred.is_empty()
            || self.notices.waiting();
        if want == self.animating {
            return;
        }
        let effect = Effect::RequestFrames { on: want };
        if self.ledger.room(effect.tier()) > 0 {
            self.ledger.spend(1);
            self.animating = want;
            out.push(effect);
        }
    }

    /// One frame: the clock moves, the tree moves, and a refused draw is tried once more.
    pub(super) fn frame_tick(&mut self, dt_ms: f32) {
        self.ledger.advance(dt_ms);
        if std::mem::take(&mut self.refused) {
            self.dirty = true;
        }
        if self.screen == Screen::Live && self.graph.is_moving() {
            self.graph.step(dt_ms, self.ctx.theme.motion);
            self.dirty = true;
        }
    }

    /// The host shows the frame before the refused one; the current state is drawn next frame.
    pub(super) fn draw_refused(&mut self) {
        if let Some(before) = self.before.take() {
            self.frame = before;
        }
        self.refused = true;
    }

    /// A `NOTIFY` mapped to a host notification: held until the host takes it.
    pub(super) fn queue_notify(&mut self, note: HostNotify) {
        if let Err(gave_up) = self.notices.push(note, self.ledger.now_ms()) {
            self.gave_up(&gave_up);
        }
    }

    pub(super) fn notify_refused(&mut self, event_id: &[u8; 32]) {
        if let Some(gave_up) = self.notices.refused(event_id, self.ledger.now_ms()) {
            self.gave_up(&gave_up);
        }
    }

    fn gave_up(&mut self, gave_up: &GaveUp) {
        let reason = match gave_up {
            GaveUp::Full => "a notification was dropped: the notification queue is full".into(),
            GaveUp::Refused { attempts } => {
                format!("a notification was given up: the host refused all {attempts} attempts")
            }
        };
        self.deferred.push_back(Effect::Fault { reason });
    }
}
