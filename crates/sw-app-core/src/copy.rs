//! Every sentence the App shows (SSOT). Lines are kept short: the canvas has no text wrap.

pub const APP_NAME: &str = "synchronized.work";
pub const PROJECT_UNTITLED: &str = "Project (title hidden)";
pub const FALLBACK_STATE: &str = "HTTPS fallback, no Citadel session";

pub const CONNECTING_TITLE: &str = "Connecting to synchronized.work";
pub const CONNECTING_BODY: &[&str] = &["Your Citadel agent is opening a session for this app."];

pub const LINK_TITLE: &str = "Link your synchronized.work person";
pub const LINK_BODY: &[&str] = &[
    "On your computer, run: sw-mcp link-code --project <id>",
    "It prints an 8-character code. Type it here and press Enter.",
];
pub const LINK_FIELD_LABEL: &str = "Link code";
pub const LINK_PENDING: &str = "Checking the code with synchronized.work...";
pub const LINK_REFUSED: &str = "That code was not accepted. Get a new one and try again.";

pub const EMPTY_NO_PROJECT_TITLE: &str = "Linked, with no project for this page yet";
pub const EMPTY_NO_PROJECT_BODY: &[&str] = &[
    "Get a code for a project with sw-mcp link-code,",
    "then link again with the new code.",
];
pub const EMPTY_NOTHING_TITLE: &str = "Nothing to show yet";
pub const EMPTY_NOTHING_BODY: &[&str] = &[
    "This project has no tasks you can see.",
    "They appear here as soon as someone adds one.",
];

pub const COMPOSER_READ_ONLY: &str =
    "Prompts are read-only here. Pair this browser with sw-mcp trust to send them.";

pub const ACTION_RETRY: &str = "Try again";
pub const ACTION_ENTER_CODE: &str = "Enter a link code";
pub const ACTION_SUBMIT_CODE: &str = "Link";
pub const ACTION_OPEN_SITE: &str = "Copy the synchronized.work address";

pub const ERROR_TITLE: &str = "synchronized.work is not available";
pub const ERROR_NO_TRANSPORT: &[&str] = &[
    "This workspace gives the app neither a Citadel session nor HTTPS access.",
    "Ask an admin to grant citadel.session, or use synchronized.work directly.",
];
pub const ERROR_NO_IDENTITY: &[&str] = &[
    "Your workspace account has no Citadel identity, which linking needs.",
    "Use synchronized.work directly, or ask an admin about portable identity.",
];
pub const ERROR_NOT_LINKED_FALLBACK: &[&str] = &[
    "Without a Citadel session this app cannot be linked.",
    "Link it once on a workspace that grants citadel.session.",
];
pub const ERROR_SESSION_CLOSED: &str = "The session to synchronized.work closed:";
pub const ERROR_OPEN_FAILED: &str = "The session to synchronized.work could not open:";
pub const ERROR_WIRE: &str = "synchronized.work sent a message this app cannot read.";
pub const ERROR_SEND: &str = "A message could not be sent:";
pub const ERROR_RECEIVE: &str = "A message could not be received:";
pub const ERROR_TREE: &str = "The tree could not be laid out:";
pub const ERROR_HTTP: &str = "The HTTPS stream answered with status";
pub const ERROR_HTTP_FAILED: &str = "The HTTPS stream failed:";
pub const ERROR_STORAGE: &str = "This app's saved link could not be read:";
pub const ERROR_FRAME: &str = "The tree is too large to draw accessibly on this canvas.";
pub const ERROR_NEXT_RETRY: &str = "Try again; your place in the project is kept.";
