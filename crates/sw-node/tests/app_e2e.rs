#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The Citadel App's core (`sw-app-core`'s `App`) against this node's real core, joined by an
//! in-memory session: first run → link code → the live tree → a join arrives as exactly one
//! host notification and a redraw. Every byte either side sends is the other's input.

mod app_link;
mod feed_support;
mod support;

#[path = "../../sw-app-core/tests/support/mod.rs"]
mod app_support;

use app_link::{Session, notifies};
use app_support::{app, granted};
use feed_support::*;
use sw_app_core::link::LinkRecord;
use sw_app_core::{CanvasAbi, Effect, Screen};

#[test]
fn first_run_links_with_a_code_shows_the_live_tree_and_a_join_is_one_notification() {
    let mut w = project();
    mint(&mut w, "K7M2Q9XW");
    let mut s = Session::new(app(granted(true, false), CanvasAbi::Cw5), w);

    // First run: no saved link; the agent opens the session; the App says HELLO and asks.
    s.open(None);
    assert!(
        matches!(s.app.screen(), Screen::NeedsLink(_)),
        "{:?}",
        s.app.screen()
    );

    // The member types the code and presses Enter.
    let host = s.type_line("k7m2q9xw");
    assert_eq!(s.app.screen(), &Screen::Live);
    let saved = host.iter().find_map(|e| match e {
        Effect::WriteUserStorage { value, .. } => Some(LinkRecord::from_bytes(value).unwrap()),
        _ => None,
    });
    assert_eq!(
        saved,
        Some(LinkRecord {
            project: Some("p".into())
        })
    );
    let state = s.app.live().unwrap().state();
    assert_eq!(state.tasks.keys().collect::<Vec<_>>(), ["p", "t"]);

    // Bob asks to join `t` and Alice admits him: the App's Person owns the task.
    let request = bob_requests(&mut s.w, "t");
    let sent = alice(&mut s.w, accept(request));
    let host = s.deliver(sent);
    assert_eq!(notifies(&host), 1, "{host:?}");
    assert!(
        host.iter().any(|e| matches!(e, Effect::Draw(_))),
        "the tree redraws"
    );
}
