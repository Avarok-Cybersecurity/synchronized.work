//! A consumer's replay of the fixture, with no node: a fresh App mounted with the fixture's
//! context is given only the `deliver` steps (and the typed code), and every `SessionSend` it
//! makes must be the next `expect` step's bytes.

use std::collections::VecDeque;

use super::fixture::Fixture;
use super::{host_notifies, labels, recorded_ctx, shipped};
use crate::app_link::Step;
use crate::app_support::FixedAdvance;
use sw_app_core::notify::HostNotify;
use sw_app_core::{App, Effect, HostSessionStatus, Input, Scope, Screen};

/// What the replayed App showed.
pub struct Replayed {
    pub live_after_steps: bool,
    pub after_catch_up: Vec<String>,
    pub after_join: Vec<String>,
    /// Notifications during `steps`, then during `tail`.
    pub notified: (Vec<HostNotify>, Vec<HostNotify>),
}

struct Player {
    app: App<FixedAdvance>,
    sends: VecDeque<Vec<u8>>,
    host: Vec<Effect>,
    code: String,
    typed: bool,
}

impl Player {
    fn take(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::SessionSend { body } => self.sends.push_back(body),
                other => self.host.push(other),
            }
        }
    }

    fn play(&mut self, steps: &[(Step, String)], part: &str) -> Vec<HostNotify> {
        self.host.clear();
        for (i, (step, what)) in steps.iter().enumerate() {
            match step {
                Step::Expect(bytes) => {
                    if what == "redeem" && !self.typed {
                        self.typed = true;
                        let keys: Vec<String> = self.code.chars().map(String::from).collect();
                        for key in keys.into_iter().chain(["Enter".to_owned()]) {
                            let effects = self.app.handle(Input::Key { key });
                            self.take(effects);
                        }
                    }
                    let sent = self.sends.pop_front();
                    assert_eq!(sent.as_ref(), Some(bytes), "{part}[{i}] ({what})");
                }
                Step::Deliver(bytes) => {
                    let body = bytes.clone();
                    let effects = self.app.handle(Input::SessionMessage { body });
                    self.take(effects);
                }
            }
        }
        assert!(self.sends.is_empty(), "{part}: unexpected sends");
        host_notifies(&self.host)
    }
}

pub fn replay(f: &Fixture) -> Replayed {
    let mut ctx = recorded_ctx();
    ctx.acting_username = f.acting_username.clone();
    ctx.install_id = f.install_id.clone();
    ctx.node_id = f.node_id.clone();
    ctx.app_id = f.app_id.clone();
    ctx.scope = Scope::Node {
        node_id: f.node_id.clone(),
    };
    let mut p = Player {
        app: shipped(ctx),
        sends: VecDeque::new(),
        host: Vec::new(),
        code: f.link_code.clone(),
        typed: false,
    };
    let mut out = p.app.start();
    out.extend(p.app.handle(Input::LinkLoaded { record: None }));
    out.extend(p.app.handle(Input::SessionOpened {
        cid: f.cid,
        status: HostSessionStatus::Connected,
    }));
    p.take(out);
    let opened = p.host.iter().find_map(|e| match e {
        Effect::SessionOpen { server } => Some(server.clone()),
        _ => None,
    });
    assert_eq!(opened.as_ref(), Some(&f.server), "the session's server");
    let during_steps = p.play(&f.steps, "steps");
    let live_after_steps = p.app.screen() == &Screen::Live;
    let after_catch_up = if live_after_steps {
        labels(&p.app)
    } else {
        Vec::new()
    };
    let during_tail = p.play(&f.tail, "tail");
    Replayed {
        live_after_steps,
        after_catch_up,
        after_join: labels(&p.app),
        notified: (during_steps, during_tail),
    }
}
