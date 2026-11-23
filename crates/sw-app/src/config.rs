//! Every value the App runs with that is not the host's (PCND: one module, each stated). The
//! manifest in citadel-workspace (`apps/first-party/synchronized-work/manifest.json`) states the
//! same id, server, host and session limits; a test there fails when the two differ.

use sw_app_core::painter::RectOnlyParams;
use sw_app_core::{AppConfig, PollConfig, SessionLimits};
use sw_graph::Spring;
use sw_view::{KindSizes, ViewParams};

use crate::theme::Hsl;

/// The manifest's `id`.
pub const APP_ID: &str = "work.synchronized.app";
/// The one `citadel.session` server (the manifest's `servers`).
pub const SERVER: &str = "synchronized.work";
/// The one `net.host` host (the manifest's `hosts`), for the HTTPS fallback.
pub const POLL_HOST: &str = "synchronized.work";
/// What "Copy the synchronized.work address" copies.
pub const SITE_URL: &str = "https://synchronized.work";
/// The manifest's `limits.session` (`citadel-session-abi` §2).
pub const SESSION: SessionLimits = SessionLimits {
    max_msg_bytes: 65536,
    max_msgs_per_s: 8,
};
/// How long the node may hold one fallback request open (design §7.4; the host caps at 30 s).
pub const POLL_MAX_MS: u32 = 25_000;
/// `NOTIFY` `event_id`s remembered for de-duplication.
pub const NOTIFY_MEMORY: usize = 256;
/// `HELLO.client_build`.
pub const CLIENT_BUILD: &str = concat!("sw-app/", env!("CARGO_PKG_VERSION"));
/// ABI-1 hosts: line segments per arc, and the outline of a path that only has a fill.
pub const RECT_ONLY: RectOnlyParams = RectOnlyParams {
    arc_segments: 16,
    path_line_width: 1.0,
};

/// `NetStreamRead`'s `max` per read of a fallback response.
pub const STREAM_READ_BYTES: u32 = 65536;
/// A fallback response body larger than this is abandoned (closed and reported as failed).
pub const STREAM_MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
/// When the host refuses `MeasureText`, a label is taken to be this many `px` per character.
pub const MEASURE_REFUSED_EM: f32 = 0.6;

/// The tree's sizes and gaps: the site's (`site/src/view/params.json`, which a test compares
/// with these), so the App and the site lay the tree out alike.
pub const VIEW: ViewParams = ViewParams {
    node_gap: 24.0,
    level_gap: 120.0,
    sizes: KindSizes {
        project: (220.0, 64.0),
        task: (220.0, 72.0),
        person: (180.0, 64.0),
        agent: (180.0, 64.0),
        progress: (200.0, 64.0),
    },
};

/// The host sends no reduced-motion preference (`AppContext` has no such field), so the tree
/// always moves with this spring; `Motion::Lite` waits for a host signal.
pub const SPRING: Spring = Spring {
    stiffness: 170.0,
    damping: 26.0,
    rest: 0.5,
};

/// Sizes of the screens' text and controls, in CSS px. Buttons are at least 44 px tall
/// (design §8.3: tap targets in px).
pub const MARGIN: f32 = 16.0;
pub const TITLE_PX: f32 = 20.0;
pub const BODY_PX: f32 = 14.0;
pub const LINE_HEIGHT: f32 = 22.0;
pub const BUTTON_SIZE: (f32, f32) = (300.0, 44.0);
pub const LABEL_PX: f32 = 13.0;
pub const EDGE_WIDTH: f32 = 1.5;

/// The brand's green (`site/brand/brand.json` `colors.green`): Agents, which no host token names.
pub const BRAND_GREEN: u32 = 0x76B9_00FF;

/// The host tokens the App reads (`page-runtime.ts` `THEME_TOKENS`, sent as `hsl(H S% L%)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Background,
    Foreground,
    Primary,
    PrimaryForeground,
    Muted,
    MutedForeground,
    Border,
    Destructive,
}

impl Token {
    pub const ALL: [Token; 8] = [
        Token::Background,
        Token::Foreground,
        Token::Primary,
        Token::PrimaryForeground,
        Token::Muted,
        Token::MutedForeground,
        Token::Border,
        Token::Destructive,
    ];

    /// The key in `AppContext.theme`.
    pub const fn name(self) -> &'static str {
        match self {
            Token::Background => "background",
            Token::Foreground => "foreground",
            Token::Primary => "primary",
            Token::PrimaryForeground => "primary-foreground",
            Token::Muted => "muted",
            Token::MutedForeground => "muted-foreground",
            Token::Border => "border",
            Token::Destructive => "destructive",
        }
    }

    /// Used when the host sends none, or one that does not parse: the workspace UI's own light
    /// theme (`citadel-workspaces/src/index.css` `:root`).
    pub const fn fallback(self) -> Hsl {
        match self {
            Token::Background => Hsl::new(0.0, 0.0, 100.0),
            Token::Foreground => Hsl::new(235.0, 18.0, 13.0),
            Token::Primary => Hsl::new(257.0, 45.0, 45.0),
            Token::PrimaryForeground => Hsl::new(0.0, 0.0, 100.0),
            Token::Muted => Hsl::new(240.0, 20.0, 96.0),
            Token::MutedForeground => Hsl::new(235.0, 12.0, 38.0),
            Token::Border => Hsl::new(240.0, 15.0, 88.0),
            Token::Destructive => Hsl::new(0.0, 72.0, 45.0),
        }
    }
}

/// The App's settings.
pub fn app_config() -> AppConfig {
    AppConfig {
        server: SERVER.to_owned(),
        site_url: SITE_URL.to_owned(),
        client_build: CLIENT_BUILD.to_owned(),
        session: SESSION,
        poll: PollConfig {
            origin: format!("https://{POLL_HOST}"),
            max_ms: POLL_MAX_MS,
        },
        notify_memory: NOTIFY_MEMORY,
        rect_only: RECT_ONLY,
    }
}

/// A label's width when the host refused to measure it.
pub fn refused_measure(text: &str, px: f32) -> f32 {
    text.chars().count() as f32 * px * MEASURE_REFUSED_EM
}
