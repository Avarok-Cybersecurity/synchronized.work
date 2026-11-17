//! The headers every page and asset carries (lane S3). The site loads only its own scripts,
//! styles and wasm, talks only to its own origin, and is never framed. Static assets run
//! through the Worker (`run_worker_first`, which `xtask plan` requires), so no asset can be
//! served without these.

/// `'wasm-unsafe-eval'` lets the page compile its own wasm (the view core); it does not allow
/// `eval` of JavaScript. `form-action` admits the issuer because "Sign in with Citadel" is a
/// navigation there. Nothing inline: no script or style attribute in markup is honoured.
pub const CONTENT_SECURITY_POLICY: &str = "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; \
style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'self'; manifest-src 'self'; \
base-uri 'none'; form-action 'self' https://work.avarok.net; frame-ancestors 'none'; \
upgrade-insecure-requests";

pub const HEADERS: &[(&str, &str)] = &[
    ("content-security-policy", CONTENT_SECURITY_POLICY),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "no-referrer"),
    ("x-frame-options", "DENY"),
    ("cross-origin-opener-policy", "same-origin"),
    ("cross-origin-resource-policy", "same-origin"),
    (
        "permissions-policy",
        "camera=(), microphone=(), geolocation=(), payment=(), usb=(), browsing-topics=()",
    ),
    (
        "strict-transport-security",
        "max-age=31536000; includeSubDomains",
    ),
];
