//! The fixture played in time to an App under a scripted host: the host answers every call the
//! App makes (each effect, each text measure) through one token bucket like the host's own
//! (`rate` calls a second, holding `rate`), refusing what is over it, and delivers frames only
//! while the App has asked for them. The member types the link code as one burst of keys.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use super::fixture::Fixture;
use super::motion::{Accepted, fold_nodes};
use super::recorded_ctx;
use crate::app_link::Step;
use crate::app_support::FixedAdvance;
use sw_app_core::notify::HostNotify;
use sw_app_core::*;

/// citadel-workspaces' host `TokenBucket`.
pub struct Bucket {
    rate: f64,
    tokens: f64,
    last: f64,
}

impl Bucket {
    pub fn new(rate: f64) -> Self {
        Self {
            rate,
            tokens: rate,
            last: 0.0,
        }
    }

    fn take(&mut self, now: f64) -> bool {
        let elapsed = (now - self.last).max(0.0);
        self.last = self.last.max(now);
        self.tokens = self.rate.min(self.tokens + elapsed * self.rate / 1000.0);
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
    }
}

pub struct Host {
    pub now_ms: f64,
    bucket: Bucket,
    /// When the App made each call.
    pub calls: Vec<f64>,
    /// When the App asked for each text width.
    pub measures: Vec<f64>,
    pub refused: usize,
}

impl Host {
    fn call(&mut self) -> bool {
        self.calls.push(self.now_ms);
        let ok = self.bucket.take(self.now_ms);
        self.refused += usize::from(!ok);
        ok
    }
}

/// `canvas.MeasureText` on the scripted host: the support's fixed advance, or refused.
#[derive(Clone)]
pub struct ScriptedMeasure(pub Rc<RefCell<Host>>);

impl HostText for ScriptedMeasure {
    fn measure(&self, text: &str, px: f32) -> Option<f32> {
        let mut host = self.0.borrow_mut();
        let at = host.now_ms;
        host.measures.push(at);
        host.call().then(|| FixedAdvance.width(text, px))
    }
}

pub struct Paced {
    pub app: App<ScriptedMeasure>,
    pub host: Rc<RefCell<Host>>,
    pub frames_on: bool,
    /// What the host took.
    pub sends: Vec<Vec<u8>>,
    pub notified: Vec<HostNotify>,
    pub drawn: Option<Vec<DrawCommand>>,
    /// Every draw the host took, when, and the fold's tree nodes at that moment.
    pub accepted: Vec<Accepted>,
    /// When each message was delivered, and the labels the fold held after it.
    pub delivered: Vec<f64>,
    pub folded: Vec<Vec<String>>,
    /// The link record the App saved.
    pub stored: Option<Vec<u8>>,
}

/// The fixture's context, as `replay` mounts it.
pub fn ctx_of(f: &Fixture) -> AppCtx {
    let mut ctx = recorded_ctx();
    ctx.acting_username = f.acting_username.clone();
    ctx.install_id = f.install_id.clone();
    ctx.node_id = f.node_id.clone();
    ctx.app_id = f.app_id.clone();
    ctx.scope = Scope::Node {
        node_id: f.node_id.clone(),
    };
    ctx
}

impl Paced {
    pub fn new(ctx: AppCtx, config: AppConfig, rate: f64) -> Self {
        let host = Rc::new(RefCell::new(Host {
            now_ms: 0.0,
            bucket: Bucket::new(rate),
            calls: Vec::new(),
            measures: Vec::new(),
            refused: 0,
        }));
        Self {
            app: App::new(ctx, config, ScriptedMeasure(host.clone())),
            host,
            frames_on: false,
            sends: Vec::new(),
            notified: Vec::new(),
            drawn: None,
            accepted: Vec::new(),
            delivered: Vec::new(),
            folded: Vec::new(),
            stored: None,
        }
    }

    pub fn feed_effects(&mut self, effects: Vec<Effect>) {
        self.perform(effects);
    }

    fn perform(&mut self, effects: Vec<Effect>) {
        let mut queue: VecDeque<Effect> = effects.into();
        while let Some(effect) = queue.pop_front() {
            let ok = self.host.borrow_mut().call();
            match effect {
                Effect::Draw(commands) if ok => {
                    self.accepted.push(Accepted {
                        at: self.host.borrow().now_ms,
                        commands: commands.clone(),
                        nodes: fold_nodes(&self.app),
                    });
                    self.drawn = Some(commands);
                }
                Effect::Draw(_) => queue.extend(self.app.handle(Input::DrawRefused)),
                Effect::Notify(n) => {
                    let event_id = n.event_id;
                    let reply = if ok {
                        self.notified.push(n);
                        Input::NotifyDelivered { event_id }
                    } else {
                        Input::NotifyRefused { event_id }
                    };
                    queue.extend(self.app.handle(reply));
                }
                Effect::SessionSend { body } if ok => self.sends.push(body),
                Effect::RequestFrames { on } if ok => self.frames_on = on,
                Effect::WriteUserStorage { value, .. } if ok => self.stored = Some(value),
                _ => {}
            }
        }
    }

    pub fn feed(&mut self, input: Input) {
        let effects = self.app.handle(input);
        self.perform(effects);
    }

    /// `dt_ms` of host time; a frame if the App asked for frames.
    pub fn tick(&mut self, dt_ms: f32) {
        self.host.borrow_mut().now_ms += f64::from(dt_ms);
        if self.frames_on {
            self.feed(Input::Frame { dt_ms });
        }
    }

    pub fn settle(&mut self, dt_ms: f32, ms: f32) {
        for _ in 0..(ms / dt_ms) as usize {
            self.tick(dt_ms);
        }
    }

    /// The whole story: the session opens, each delivery one frame interval after the last,
    /// the code typed as one burst just before the redeem, `settle_ms` after the steps and
    /// after the tail.
    pub fn play(&mut self, f: &Fixture, dt_ms: f32, settle_ms: f32) {
        let effects = self.app.start();
        self.perform(effects);
        self.feed(Input::LinkLoaded { record: None });
        self.feed(Input::SessionOpened {
            cid: f.cid,
            status: HostSessionStatus::Connected,
        });
        for part in [&f.steps, &f.tail] {
            for (step, what) in part {
                match step {
                    Step::Expect(_) if what == "redeem" => {
                        let keys = f.link_code.chars().map(String::from);
                        for key in keys.chain(["Enter".to_owned()]) {
                            self.feed(Input::Key { key });
                        }
                    }
                    Step::Expect(_) => {}
                    Step::Deliver(body) => {
                        self.deliver(dt_ms, body);
                    }
                }
            }
            self.settle(dt_ms, settle_ms);
        }
    }
}

/// The fixture's `expect` steps, in order.
pub fn expected_sends(f: &Fixture) -> Vec<Vec<u8>> {
    f.steps
        .iter()
        .chain(&f.tail)
        .filter_map(|(s, _)| match s {
            Step::Expect(b) => Some(b.clone()),
            Step::Deliver(_) => None,
        })
        .collect()
}

/// The most calls in any window `(t - 1000, t]`.
pub fn busiest_second(calls: &[f64]) -> usize {
    calls
        .iter()
        .map(|t| {
            calls
                .iter()
                .filter(|u| **u > t - 1000.0 && **u <= *t)
                .count()
        })
        .max()
        .unwrap_or(0)
}
