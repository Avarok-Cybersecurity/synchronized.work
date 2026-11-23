//! The fixture played in time to an App under a scripted host: the host answers every call the
//! App makes (each effect, each text measure) through one token bucket like the host's own
//! (`rate` calls a second, holding `rate`), refusing what is over it, and delivers frames only
//! while the App has asked for them. The member types the link code as one burst of keys.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use super::fixture::Fixture;
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
        self.0
            .borrow_mut()
            .call()
            .then(|| FixedAdvance.width(text, px))
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
            refused: 0,
        }));
        Self {
            app: App::new(ctx, config, ScriptedMeasure(host.clone())),
            host,
            frames_on: false,
            sends: Vec::new(),
            notified: Vec::new(),
            drawn: None,
        }
    }

    fn perform(&mut self, effects: Vec<Effect>) {
        let mut queue: VecDeque<Effect> = effects.into();
        while let Some(effect) = queue.pop_front() {
            let ok = self.host.borrow_mut().call();
            match effect {
                Effect::Draw(commands) if ok => self.drawn = Some(commands),
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

    fn settle(&mut self, dt_ms: f32, ms: f32) {
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
                        self.tick(dt_ms);
                        self.feed(Input::SessionMessage { body: body.clone() });
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
