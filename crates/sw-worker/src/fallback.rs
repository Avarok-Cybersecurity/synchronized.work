//! What the Worker answers when the site has no asset at a path, and the headers it adds by
//! path. Pure, so it is tested natively. The asset binding runs with
//! `not_found_handling = "none"`, so a miss reaches here instead of always becoming the shell:
//! an app route (`/p/demo`, `/auth/return`) gets the shell, which routes itself, and a path
//! that names a file (`/og-image.png`, `/favicon.ico`) gets the binding's 404. Answering those
//! with the shell (200 text/html) is what broke link previews and icons silently.

/// Where the app shell is served from.
pub const SHELL_PATH: &str = "/";
/// The service worker script (site/vite/service-worker.mjs emits it at the site's root).
pub const SERVICE_WORKER_PATH: &str = "/sw.js";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Miss {
    /// Serve the app shell with 200; the site's router decides what the path shows.
    Shell,
    /// Pass the asset binding's 404 through.
    NotFound,
}

/// A path names a file when its last segment has a dot in it: an extension (`a.png`) or a
/// dotfile (`.env`). App routes never do.
pub fn names_a_file(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|last| last.contains('.'))
}

pub fn on_miss(path: &str) -> Miss {
    if names_a_file(path) {
        Miss::NotFound
    } else {
        Miss::Shell
    }
}

/// A `cache-control` the Worker sets over the asset binding's own. The service worker must be
/// revalidated on every check, so a deploy's new worker (and its new cache) is found at once.
pub fn cache_control(path: &str) -> Option<&'static str> {
    (path == SERVICE_WORKER_PATH).then_some("no-cache")
}

/// The Windows installer (`irm https://synchronized.work/install.ps1 | iex`).
pub const INSTALL_PS1_PATH: &str = "/install.ps1";

/// A `content-type` the Worker sets over the asset binding's own. The binding knows no `.ps1`
/// type and would answer `application/octet-stream`, which PowerShell's `irm` may hand to `iex`
/// as bytes rather than as the script.
pub fn content_type(path: &str) -> Option<&'static str> {
    (path == INSTALL_PS1_PATH).then_some("text/plain; charset=utf-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_is_not_found() {
        for path in [
            "/og-image.png",
            "/favicon.ico",
            "/assets/missing.js",
            "/a/b/c.webmanifest",
            "/.env",
            "/p/demo.json",
        ] {
            assert_eq!(on_miss(path), Miss::NotFound, "{path}");
        }
    }

    #[test]
    fn an_app_route_gets_the_shell() {
        for path in ["/", "/p/demo", "/auth/return", "/p/demo/", "/no-such-page"] {
            assert_eq!(on_miss(path), Miss::Shell, "{path}");
        }
    }

    /// The negative control: a dot in an earlier segment does not make the path a file.
    #[test]
    fn only_the_last_segment_counts() {
        assert_eq!(on_miss("/v1.2/p/demo"), Miss::Shell);
        assert!(!names_a_file("/.well-known/citadel"));
        assert!(names_a_file("/.well-known/citadel.json"));
    }

    #[test]
    fn only_the_service_worker_is_revalidated_every_time() {
        assert_eq!(cache_control("/sw.js"), Some("no-cache"));
        assert_eq!(cache_control("/sw.js.map"), None);
        assert_eq!(cache_control("/assets/sw.js"), None);
        assert_eq!(cache_control("/"), None);
    }

    #[test]
    fn only_the_windows_installer_is_typed_as_text() {
        assert_eq!(
            content_type("/install.ps1"),
            Some("text/plain; charset=utf-8")
        );
        assert_eq!(content_type("/install.sh"), None);
        assert_eq!(content_type("/a/install.ps1"), None);
    }
}
