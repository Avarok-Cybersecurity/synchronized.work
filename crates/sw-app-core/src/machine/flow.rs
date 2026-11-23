//! Connecting, linking, and the actions on each screen.

use sw_client::SessionPort;
use sw_client::live::LiveProject;
use sw_protocol::signed::SwMessage;

use super::{App, Effect};
use crate::fallback::PlanError;
use crate::link::{self, LinkOutcome, LinkRecord};
use crate::measure::HostText;
use crate::screen::{AppError, EmptyReason, LinkScreen, Screen};
use crate::session::HostSessionStatus;
use crate::transport::{self, Transport};

impl<M: HostText> App<M> {
    pub(super) fn set(&mut self, screen: Screen) {
        if self.screen != screen {
            self.screen = screen;
            self.dirty = true;
        }
    }

    /// After a `no_identity` refusal, the long-poll failing is shown as what is missing.
    pub(super) fn fail(&mut self, error: AppError) {
        let error = if self.no_identity && error.is_fallback_failure() {
            AppError::NoIdentity
        } else {
            error
        };
        self.set(Screen::Error(error));
    }

    /// One attempt from the start: the transport is chosen again and the session opened first.
    pub(super) fn begin(&mut self, out: &mut Vec<Effect>) {
        self.set(Screen::Connecting);
        self.dirty = true;
        self.no_identity = false;
        self.greeted = false;
        match transport::choose(&self.ctx.granted) {
            Ok(t) => self.transport = Some(t),
            Err(e) => return self.fail(e),
        }
        self.load_link(out);
        if self.transport == Some(Transport::Session) {
            self.open(out);
        }
        self.advance(out);
    }

    pub(super) fn load_link(&mut self, out: &mut Vec<Effect>) {
        if self.link.is_some() {
            return;
        }
        if self.ctx.granted.storage_user {
            out.push(Effect::ReadUserStorage {
                key: link::storage_key(&self.ctx.node_id),
            });
        } else {
            // Nothing can be remembered: every open starts unlinked.
            self.link = Some(None);
        }
    }

    pub(super) fn open(&mut self, out: &mut Vec<Effect>) {
        out.push(Effect::SessionOpen {
            server: self.config.server.clone(),
        });
    }

    pub(super) fn link_loaded(&mut self, record: Option<Vec<u8>>, out: &mut Vec<Effect>) {
        match record.as_deref().map(LinkRecord::from_bytes).transpose() {
            Ok(r) => self.link = Some(r),
            Err(e) => {
                return self.fail(AppError::Storage {
                    reason: format!("{e:?}"),
                });
            }
        }
        self.advance(out);
    }

    pub(super) fn on_status(&mut self, status: HostSessionStatus, out: &mut Vec<Effect>) {
        self.greeted = false;
        match status {
            HostSessionStatus::Connected => self.advance(out),
            HostSessionStatus::Connecting => self.set(Screen::Connecting),
            HostSessionStatus::Closed { reason } => self.fail(AppError::SessionClosed { reason }),
        }
    }

    pub(super) fn open_failed(&mut self, reason: &str, out: &mut Vec<Effect>) {
        self.no_identity = transport::is_no_identity(reason);
        match transport::after_open_failure(reason, &self.ctx.granted) {
            Ok(t) => {
                self.transport = Some(t);
                self.advance(out);
            }
            Err(e) => self.fail(e),
        }
    }

    /// Moves on as far as what is known allows: the saved link decides between the link
    /// screen and a subscription; a (re)connected session is greeted exactly once.
    pub(super) fn advance(&mut self, out: &mut Vec<Effect>) {
        let Some(link) = self.link.clone() else {
            return self.set(Screen::Connecting);
        };
        match self.transport {
            Some(Transport::Session) => {
                if !matches!(
                    self.session.state(),
                    sw_client::SessionState::Connected { .. }
                ) {
                    return self.set(Screen::Connecting);
                }
                if self.greeted {
                    return;
                }
                self.greeted = true;
                match link {
                    None => {
                        self.say_hello();
                        self.set(Screen::NeedsLink(LinkScreen::default()));
                    }
                    Some(LinkRecord { project: None }) => {
                        self.say_hello();
                        self.set(Screen::Empty(EmptyReason::NoProject));
                    }
                    Some(LinkRecord {
                        project: Some(project),
                    }) => self.subscribe(&project),
                }
            }
            Some(Transport::Fallback) => match link.and_then(|l| l.project) {
                None => self.fail(AppError::NotLinkedOnFallback),
                Some(project) => {
                    self.subscribe(&project);
                    self.poll(out);
                }
            },
            None => {}
        }
    }

    fn say_hello(&mut self) {
        let hello = sw_client::live::hello(&self.config.client_build);
        let sent = SwMessage::Unsigned(hello)
            .to_bytes()
            .map_err(|e| format!("{e:?}"))
            .and_then(|b| self.session.send(b).map_err(|e| format!("{e:?}")));
        if let Err(reason) = sent {
            self.fail(AppError::Send { reason });
        }
    }

    /// `HELLO` and `SUBSCRIBE` from the fold's cursor, on whichever port is in use.
    fn subscribe(&mut self, project: &str) {
        if self
            .live
            .as_ref()
            .is_none_or(|l| l.state().project_id != project)
        {
            self.live = Some(LiveProject::new(project, &self.config.client_build));
        }
        let Some(live) = &self.live else { return };
        let port: &mut dyn SessionPort = match self.transport {
            Some(Transport::Fallback) => &mut self.fallback,
            _ => &mut self.session,
        };
        if let Err(e) = live.subscribe(port) {
            return self.fail(AppError::Send {
                reason: format!("{e:?}"),
            });
        }
        self.show_live();
    }

    pub(super) fn poll(&mut self, out: &mut Vec<Effect>) {
        let Some(live) = &self.live else {
            return self.fail(AppError::HttpFailed {
                reason: format!("{:?}", PlanError::NoProject),
            });
        };
        match self
            .fallback
            .next_request(&self.config.poll, live.state().cursor)
        {
            Ok(get) => out.push(Effect::HostFetchOpen(get)),
            Err(e) => self.fail(AppError::HttpFailed {
                reason: format!("{e:?}"),
            }),
        }
    }

    pub(super) fn linked(&mut self, outcome: LinkOutcome, out: &mut Vec<Effect>) {
        let project = match outcome {
            LinkOutcome::Refused(refused) => {
                return self.set(Screen::NeedsLink(LinkScreen {
                    refused: Some(refused),
                    ..LinkScreen::default()
                }));
            }
            LinkOutcome::Linked { project } => project,
        };
        let record = LinkRecord {
            project: project.clone(),
        };
        if self.ctx.granted.storage_user {
            match record.to_bytes() {
                Ok(value) => out.push(Effect::WriteUserStorage {
                    key: link::storage_key(&self.ctx.node_id),
                    value,
                }),
                Err(e) => {
                    return self.fail(AppError::Storage {
                        reason: format!("{e:?}"),
                    });
                }
            }
        }
        self.link = Some(Some(record));
        match project {
            Some(p) => self.subscribe(&p),
            None => self.set(Screen::Empty(EmptyReason::NoProject)),
        }
    }
}
