//! Every sentence the App shows (SSOT). Each entry is a paragraph; `wrap` fits it to the pane.

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
pub const ERROR_NOT_LINKED_FALLBACK: &[&str] = &[
    "Without a Citadel session this app cannot be linked.",
    "Link it once on a workspace that grants citadel.session.",
];
pub const ERROR_SESSION_CLOSED: &str = "The session to synchronized.work closed:";
pub const ERROR_WIRE: &str = "synchronized.work sent a message this app cannot read.";
pub const ERROR_SEND: &str = "A message could not be sent:";
pub const ERROR_RECEIVE: &str = "A message could not be received:";
pub const ERROR_TREE: &str = "The tree could not be laid out:";
pub const ERROR_HTTP: &str = "The HTTPS stream answered with status";
pub const ERROR_HTTP_FAILED: &str = "The HTTPS stream failed:";
pub const ERROR_STORAGE: &str = "This app's saved link could not be read:";
pub const ERROR_FRAME: &str = "The tree is too large to draw accessibly on this canvas.";
pub const ERROR_NEXT_RETRY: &str = "Try again; your place in the project is kept.";

pub const NO_IDENTITY_TITLE: &str = "This workspace account has no Citadel identity yet";
pub const NO_IDENTITY_BODY: &[&str] = &[
    "This app needs a Citadel identity on your workspace account, so your Citadel agent can \
     connect to synchronized.work for you. This account does not have one yet.",
    "Nothing is wrong with your workspace or your data.",
    "A Citadel identity is set up in Citadel Workspaces' own account settings, when your \
     workspace offers it. After that, choose Try again.",
    "Meanwhile, you can use synchronized.work in a browser: copy its address below.",
];

// A refused open (`refusal::OpenRefusal`): what happened, then what to do.
const USE_THE_SITE: &str = "Until then, you can use synchronized.work in a browser.";
const WAIT_AND_RETRY: &str = "Wait a moment, then try again.";
pub const REFUSED_UNGRANTED: &[&str] = &[
    "This workspace has not allowed this app to connect to synchronized.work.",
    "A workspace admin can allow it. Until then, you can use synchronized.work in a browser.",
];
pub const REFUSED_POLICY: &[&str] = &[
    "Your workspace's rules do not let this app connect to synchronized.work.",
    "A workspace admin can change them. Until then, you can use synchronized.work in a browser.",
];
pub const REFUSED_LIMIT: &[&str] = &[
    "This app has reached its limit of connections for now.",
    WAIT_AND_RETRY,
];
pub const REFUSED_IDENTITY_LOCKED: &[&str] = &[
    "Your Citadel identity is locked on this device.",
    "Unlock it in Citadel Workspaces, then try again.",
];
pub const REFUSED_NOT_CONFIGURED: &[&str] = &[
    "Your Citadel agent is not set up to connect apps to other sites.",
    USE_THE_SITE,
];
pub const REFUSED_SIGNED_OUT: &[&str] = &[
    "This window is no longer signed in to your workspace.",
    "Reload the page or sign in again, then try again.",
];
pub const REFUSED_AUTHORITY: &[&str] = &[
    "Your workspace did not confirm which sites this app may reach.",
    WAIT_AND_RETRY,
];
pub const REFUSED_UNREACHABLE: &[&str] = &[
    "Your Citadel agent could not reach synchronized.work.",
    "Check that you are online, then try again.",
];
pub const REFUSED_INVALID: &[&str] = &[
    "Your Citadel agent did not accept this app's request to connect.",
    "Try again. If it keeps happening, you can use synchronized.work in a browser.",
];
pub const REFUSED_OTHER: &[&str] = &[
    "Your Citadel agent could not connect this app to synchronized.work.",
    "Check that Citadel Workspaces is open and online, then try again.",
];
pub const REFUSED_DETAIL: &str = "Reason reported:";
