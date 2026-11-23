//! What the App knows about where it runs (design §5.3: `acting_username`, `install_id`,
//! `node_id`, `app_id`, `scope`; plus the grant, size, theme and locale). Nothing else about
//! the member or the workspace can enter: there is no field for it.

use sw_graph::Motion;
use sw_view::ViewParams;

/// The install's scope, as the SDK's `AppScope`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Workspace,
    Node { node_id: String },
    NodeTree { node_id: String },
    Users { usernames: Vec<String> },
    SelfOnly { username: String },
}

/// The capabilities the core decides on (`AppContext.is_granted`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Granted {
    pub citadel_session: bool,
    pub net_host: bool,
    pub notifications: bool,
    pub storage_user: bool,
    pub storage_local: bool,
}

/// Which canvas the host paints: ABI-1's five commands, or CW-5's ten
/// (`canvas-ext-abi` §3: `ctx.sdk_abi_minor >= 1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasAbi {
    Abi1,
    Cw5,
}

impl CanvasAbi {
    pub fn from_abi_minor(minor: u32) -> Self {
        if minor >= 1 { Self::Cw5 } else { Self::Abi1 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// Every colour, size and gap the App draws with (PCND: the binding resolves them from the
/// host's theme tokens; nothing here has a default).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppTheme {
    pub graph: sw_graph::Theme,
    pub view: ViewParams,
    /// Inset of everything from the canvas edge.
    pub margin: f32,
    pub text: u32,
    pub muted: u32,
    pub error: u32,
    pub button: u32,
    pub button_text: u32,
    pub field: u32,
    pub title_px: f32,
    pub body_px: f32,
    /// Baseline to baseline.
    pub line_height: f32,
    pub button_size: (f32, f32),
    /// `Motion::Lite` when the member asked for reduced motion.
    pub motion: Motion,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppCtx {
    pub acting_username: String,
    pub install_id: String,
    pub node_id: String,
    pub app_id: String,
    pub scope: Scope,
    pub granted: Granted,
    pub canvas: CanvasAbi,
    pub size: Size,
    pub theme: AppTheme,
    pub locale: String,
}

/// The manifest's `limits.session` (`citadel-session-abi` §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionLimits {
    pub max_msg_bytes: usize,
    pub max_msgs_per_s: u32,
}

/// The `net.host` fallback's stream (design §7.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollConfig {
    /// `https://` and the one manifest host, e.g. `https://synchronized.work`.
    pub origin: String,
    /// How long the node may hold one request open (`max_ms=`); the host caps it at 30 s.
    pub max_ms: u32,
}

/// The App's own settings, from its manifest and build (PCND: all explicit).
#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
    /// The `citadel.session` server, one of the manifest's `servers`.
    pub server: String,
    /// What "Open synchronized.work" hands the member.
    pub site_url: String,
    /// `HELLO.client_build`.
    pub client_build: String,
    pub session: SessionLimits,
    pub poll: PollConfig,
    /// How many `NOTIFY` `event_id`s are remembered for de-duplication.
    pub notify_memory: usize,
    pub rect_only: crate::painter::RectOnlyParams,
}
