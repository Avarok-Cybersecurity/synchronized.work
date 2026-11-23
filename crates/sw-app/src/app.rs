//! [`SwApp`]: the SDK's `App`, performing what `sw-app-core` decides. Each host event or reply
//! becomes one [`Input`] (through [`crate::map`]), and each [`Effect`] the core answers becomes
//! one SDK call, in order. A reply comes back through `Pending::then` as the next `Input`.

use citadel_app_sdk::types::Capability;
use citadel_app_sdk::types::calls::EventKind;
use citadel_app_sdk::{App, AppContext, AppEvent, CloseContext, CloseReason, InitFailure};
use citadel_app_sdk::{Refused, SuspendReason};
use citadel_app_sdk::{canvas, clipboard, events, log, notify, session, storage, ui};
use sw_app_core::{App as Core, Effect, Input, TextMeasure};

use crate::map::{self, reason_of};
use crate::{config, context};

/// `canvas.MeasureText`, the core's [`TextMeasure`].
#[derive(Debug, Clone, Copy)]
pub struct HostMeasure;

impl TextMeasure for HostMeasure {
    fn width(&self, text: &str, px: f32) -> f32 {
        canvas::text_width(text, px).unwrap_or_else(|_| config::refused_measure(text, px))
    }
}

#[derive(Debug)]
pub struct SwApp {
    core: Core<HostMeasure>,
    /// The number `session::open` answered, once it has.
    session: Option<u32>,
    clipboard: bool,
}

pub(crate) fn warn(what: &str, refused: &Refused) {
    let _ = log::warn(&format!("{what}: {}", reason_of(refused)));
}

impl SwApp {
    pub(crate) fn feed(&mut self, input: Input) {
        let effects = self.core.handle(input);
        self.perform(effects);
    }

    fn perform(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            self.effect(effect);
        }
    }

    fn effect(&mut self, effect: Effect) {
        match effect {
            Effect::ReadUserStorage { key } => {
                storage::user().get(&key).then(|app: &mut SwApp, r| {
                    app.feed(match r {
                        Ok(record) => Input::LinkLoaded { record },
                        Err(e) => Input::StorageFailed {
                            reason: reason_of(&e),
                        },
                    })
                })
            }
            Effect::WriteUserStorage { key, value } => {
                storage::user()
                    .set(&key, &value)
                    .then(|app: &mut SwApp, r| {
                        if let Err(e) = r {
                            app.feed(Input::StorageFailed {
                                reason: format!("write: {}", reason_of(&e)),
                            });
                        }
                    })
            }
            Effect::SessionOpen { server } => {
                session::open(&server).then(|app: &mut SwApp, r| match r {
                    Ok(state) => {
                        app.session = Some(state.session);
                        app.feed(Input::SessionOpened {
                            cid: state.cid,
                            status: map::status_of(state.state),
                        });
                    }
                    Err(e) => app.feed(Input::SessionOpenFailed {
                        reason: reason_of(&e),
                    }),
                })
            }
            Effect::SessionSend { body } => match self.session {
                Some(s) => {
                    if let Err(e) = session::send(s, &body) {
                        warn("session send", &e);
                    }
                }
                None => {
                    let _ = log::error("session send before the session opened");
                }
            },
            Effect::HostFetchOpen(get) => crate::fetch::open(self, get),
            Effect::Notify(n) => notify::notify(&n.title, &n.body)
                .then(|_: &mut SwApp, r| r.err().iter().for_each(|e| warn("notify", e))),
            Effect::Draw(commands) => {
                let commands = commands.into_iter().map(map::draw_command_of).collect();
                if let Err(e) = canvas::draw(commands) {
                    warn("canvas draw", &e);
                }
            }
            Effect::RequestFrames { on } => events::request_frames(on)
                .then(|_: &mut SwApp, r| r.err().iter().for_each(|e| warn("frames", e))),
            Effect::CopyText { text } if self.clipboard => {
                clipboard::write_text(&text).then(move |_: &mut SwApp, r| {
                    if let Err(e) = r {
                        warn("clipboard", &e);
                        toast(&text);
                    }
                })
            }
            Effect::CopyText { text } => toast(&text),
            Effect::Fault { reason } => {
                let _ = log::error(&reason);
            }
        }
    }
}

/// `CopyText` without the clipboard: the text, shown for the member to copy by hand.
fn toast(text: &str) {
    ui::toast(text).then(|_: &mut SwApp, r| r.err().iter().for_each(|e| warn("toast", e)));
}

impl App for SwApp {
    fn init(ctx: AppContext) -> Result<Self, InitFailure> {
        if ctx.app_id != config::APP_ID {
            let _ = log::error(&format!(
                "sw-app runs as {}, not {}",
                config::APP_ID,
                ctx.app_id
            ));
            return Err(InitFailure::DECLINED);
        }
        Ok(SwApp {
            core: Core::new(context::ctx_of(&ctx), config::app_config(), HostMeasure),
            session: None,
            clipboard: ctx.is_granted(Capability::ClipboardWrite),
        })
    }

    fn event(&mut self, event: AppEvent) {
        if matches!(event, AppEvent::Init) {
            events::subscribe(vec![EventKind::Pointer, EventKind::Key])
                .then(|_: &mut SwApp, r| r.err().iter().for_each(|e| warn("subscribe", e)));
            let effects = self.core.start();
            return self.perform(effects);
        }
        if let Some(input) = map::input_of(event, self.session) {
            self.feed(input);
        }
    }

    fn frame(&mut self, dt_ms: u32) {
        self.feed(map::frame_input(dt_ms));
    }

    /// The host stops frames and input itself; the core keeps its state.
    fn suspend(&mut self, _reason: SuspendReason) {}

    fn resume(&mut self) {}

    /// Nothing to persist: the link is written when it is made, and the agent keeps the session
    /// (it outlives the instance by design, §12 Phase 4).
    fn close(&mut self, _ctx: &CloseContext, _reason: CloseReason) {}
}
