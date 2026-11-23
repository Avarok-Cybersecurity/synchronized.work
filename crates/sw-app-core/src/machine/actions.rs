//! What the member does on a screen: type a code, press a button, press Enter.

use sw_client::SessionPort;

use super::{App, Effect};
use crate::ctx::CanvasAbi;
use crate::link;
use crate::measure::HostText;
use crate::screen::{AppError, LinkScreen, NextAction, Screen};
use crate::transport::Transport;

impl<M: HostText> App<M> {
    fn submit(&mut self) {
        let Screen::NeedsLink(screen) = &self.screen else {
            return;
        };
        if !screen.entry.is_complete() || screen.pending.is_some() {
            return;
        }
        let mut next = screen.clone();
        let sent = link::redeem(next.entry.code(), &self.ctx)
            .map_err(|e| format!("{e:?}"))
            .and_then(|(bytes, id)| {
                self.session
                    .send(bytes)
                    .map(|()| id)
                    .map_err(|e| format!("{e:?}"))
            });
        match sent {
            Ok(id) => {
                next.pending = Some(id);
                next.refused = None;
                self.set(Screen::NeedsLink(next));
            }
            Err(reason) => self.fail(AppError::Send { reason }),
        }
    }

    fn perform(&mut self, action: NextAction, out: &mut Vec<Effect>) {
        match action {
            NextAction::Retry => self.retry(out),
            NextAction::EnterLinkCode => self.set(Screen::NeedsLink(LinkScreen::default())),
            NextAction::SubmitCode => self.submit(),
            NextAction::OpenSite => out.push(Effect::CopyText {
                text: self.config.site_url.clone(),
            }),
        }
    }

    /// Reopens what failed; the fold keeps its cursor, so nothing already seen is lost. A
    /// member without an identity may have one now: that attempt starts over.
    fn retry(&mut self, out: &mut Vec<Effect>) {
        let Screen::Error(error) = self.screen.clone() else {
            return;
        };
        self.set(Screen::Connecting);
        match (error, self.transport) {
            (AppError::NoIdentity, _) => self.begin(out),
            (AppError::Storage { .. }, _) => {
                self.link = None;
                self.load_link(out);
            }
            (_, Some(Transport::Fallback)) => self.poll(out),
            _ => {
                self.greeted = false;
                self.open(out);
                self.advance(out);
            }
        }
    }

    pub(super) fn activate(&mut self, id: &str, out: &mut Vec<Effect>) {
        let key = self
            .frame
            .as_ref()
            .and_then(|f| f.key_of(id))
            .map(str::to_owned);
        if let Some(action) = key.as_deref().and_then(NextAction::from_id) {
            self.perform(action, out);
        }
    }

    pub(super) fn pointer_up(&mut self, x: f32, y: f32, out: &mut Vec<Effect>) {
        if self.ctx.canvas != CanvasAbi::Abi1 {
            return; // a CW-5 host reports the region as `Activate`
        }
        let key = self
            .frame
            .as_ref()
            .and_then(|f| f.hit(x, y))
            .map(str::to_owned);
        if let Some(action) = key.as_deref().and_then(NextAction::from_id) {
            self.perform(action, out);
        }
    }

    pub(super) fn key(&mut self, key: &str, out: &mut Vec<Effect>) {
        if key == "Enter" {
            if let Some(primary) = self.screen.actions().first().copied() {
                self.perform(primary, out);
            }
            return;
        }
        if let Screen::NeedsLink(screen) = &mut self.screen
            && screen.pending.is_none()
            && screen.entry.key(key)
        {
            self.dirty = true;
        }
    }
}
