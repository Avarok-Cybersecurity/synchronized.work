#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! `fixtures/app-session/joined.json`: what one `citadel.session` carried between the shipped
//! App and the real node core, for another repository to play to the module in a browser.
//! The first test regenerates it and requires the committed file to be byte-identical
//! (`SW_WRITE_FIXTURES=1` rewrites it instead); the second plays it to a fresh App with no node.

mod app_link;
mod app_story;
mod feed_support;
mod support;

#[path = "../../sw-app-core/tests/support/mod.rs"]
mod app_support;

use app_story::fixture::Fixture;
use app_story::{PATH, record, replay::replay};

fn committed() -> String {
    std::fs::read_to_string(PATH).unwrap_or_else(|e| panic!("{PATH}: {e}"))
}

#[test]
fn the_committed_transcript_is_what_the_node_and_the_app_produce() {
    let now = Fixture::of(&record()).to_json();
    assert_eq!(
        now,
        Fixture::of(&record()).to_json(),
        "two recordings differ"
    );
    if std::env::var_os("SW_WRITE_FIXTURES").is_some_and(|v| v == "1") {
        std::fs::create_dir_all(std::path::Path::new(PATH).parent().unwrap()).unwrap();
        std::fs::write(PATH, &now).unwrap();
        return;
    }
    let was = Fixture::from_json(&committed());
    let is = Fixture::from_json(&now);
    for (part, (was, is)) in [
        ("steps", (&was.steps, &is.steps)),
        ("tail", (&was.tail, &is.tail)),
    ] {
        assert_eq!(was.len(), is.len(), "{part}: step count");
        for (i, (a, b)) in was.iter().zip(is.iter()).enumerate() {
            assert_eq!(
                a, b,
                "{part}[{i}] differs from what the node and the App produce"
            );
        }
    }
    assert_eq!(
        committed(),
        now,
        "the committed fixture differs; regenerate with SW_WRITE_FIXTURES=1"
    );
}

#[test]
fn replaying_only_the_fixture_drives_a_fresh_app_to_live_and_one_notification() {
    let f = Fixture::from_json(&committed());
    let r = replay(&f);
    assert!(r.live_after_steps, "the App reached the live tree");
    assert_eq!(r.after_catch_up, f.after_catch_up);
    assert!(
        r.notified.0.is_empty(),
        "nothing is notified before the tail"
    );
    let [n] = r.notified.1.as_slice() else {
        panic!("the tail notifies once, not {:?}", r.notified.1);
    };
    assert_eq!((&n.title, &n.body), (&f.title, &f.body));
    assert_eq!(r.after_join, f.after_join);
}
