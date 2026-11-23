//! The synchronized.work Citadel App's pure core (design §7). Every decision of the App is
//! here — the first-run states (connecting, link, empty, live, error, each with copy and a
//! next action), the transport (CW-6 `citadel.session`, or the `net.host` long-poll), the
//! mapping of `NOTIFY` to host notifications, link-code redemption, and the two painters over
//! `sw-graph`'s one draw list. The host is reached only through what [`App::handle`] returns
//! ([`Effect`]) and what the binding feeds it ([`Input`]), plus the [`TextMeasure`] port; the
//! `sw-app` binding maps those 1:1 onto `citadel-app-sdk`.
//!
//! No layout, renderer or fold of its own: points, layout and the tree are `sw-view`'s, the
//! movement and the draw list `sw-graph`'s, the fold `sw-client`'s `LiveProject`. The App
//! learns only what [`AppCtx`] holds (design §5.3); signatures and seals, when the composer
//! gains them, are `sw-protocol`'s ML-DSA-65 / ML-KEM-1024.

pub mod command;
pub mod copy;
pub mod ctx;
pub mod fallback;
pub mod graph;
pub mod link;
mod machine;
pub mod notify;
pub mod painter;
pub mod refusal;
pub mod screen;
pub mod screens;
pub mod session;
pub mod transport;
pub mod wrap;

pub use command::{DrawCommand, RegionRole};
pub use ctx::{
    AppConfig, AppCtx, AppTheme, CanvasAbi, Granted, PollConfig, Scope, SessionLimits, Size,
};
pub use machine::{App, Effect, Input};
pub use painter::{
    CitadelPainter, Frame, FrameError, FrameRegion, RectOnlyPainter, RectOnlyParams, TextMeasure,
};
pub use screen::{AppError, EmptyReason, NextAction, Screen};
pub use session::HostSessionStatus;
pub use transport::Transport;

impl<T: TextMeasure + ?Sized> TextMeasure for &T {
    fn width(&self, text: &str, px: f32) -> f32 {
        (**self).width(text, px)
    }
}
