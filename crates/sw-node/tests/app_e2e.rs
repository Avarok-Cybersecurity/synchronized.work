#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The Citadel App's core (`sw-app-core`'s `App`) against this node's real core, joined by an
//! in-memory session: first run → link code → the live tree → a join arrives as exactly one
//! host notification and a redraw. Every byte either side sends is the other's input.

mod feed_support;
mod support;

#[path = "../../sw-app-core/tests/support/mod.rs"]
mod app_support;

use app_support::{FixedAdvance, app, granted};
use feed_support::*;
use support::*;
use sw_app_core::link::LinkRecord;
use sw_app_core::{App, CanvasAbi, Effect, HostSessionStatus, Input, Screen};

/// The App account's CID: the agent's session to the node.
const APP: u64 = 7000;

/// Carries the App's `SessionSend`s to the node and the node's answers back, until both are
/// quiet; returns every other effect the App asked of its host.
fn pump(app: &mut App<FixedAdvance>, w: &mut World, first: Vec<Effect>) -> Vec<Effect> {
    let mut host = Vec::new();
    let mut queue = first;
    while !queue.is_empty() {
        let mut next = Vec::new();
        for effect in queue {
            let Effect::SessionSend { body } = effect else {
                host.push(effect);
                continue;
            };
            let effects = w.node.received(APP, &body, NOW).unwrap();
            next.extend(deliver(app, w.apply(effects)));
        }
        queue = next;
    }
    host
}

/// What the node sent the App's session, as `SessionEvent::Message`s.
fn deliver(app: &mut App<FixedAdvance>, sent: Sent) -> Vec<Effect> {
    let mut out = Vec::new();
    for (to, body) in sent {
        if to == APP {
            out.extend(app.handle(Input::SessionMessage { body }));
        }
    }
    out
}

fn notifies(effects: &[Effect]) -> usize {
    effects
        .iter()
        .filter(|e| matches!(e, Effect::Notify(_)))
        .count()
}

#[test]
fn first_run_links_with_a_code_shows_the_live_tree_and_a_join_is_one_notification() {
    let mut w = project();
    mint(&mut w, "K7M2Q9XW");
    let mut app = app(granted(true, false), CanvasAbi::Cw5);

    // First run: no saved link; the agent opens the session; the App says HELLO and asks.
    w.node.connected(APP, NOW);
    let mut out = app.start();
    out.extend(app.handle(Input::LinkLoaded { record: None }));
    out.extend(app.handle(Input::SessionOpened {
        cid: APP,
        status: HostSessionStatus::Connected,
    }));
    pump(&mut app, &mut w, out);
    assert!(
        matches!(app.screen(), Screen::NeedsLink(_)),
        "{:?}",
        app.screen()
    );

    // The member types the code and presses Enter.
    let mut out = Vec::new();
    for key in "k7m2q9xw"
        .chars()
        .map(String::from)
        .chain(["Enter".to_owned()])
    {
        out.extend(app.handle(Input::Key { key }));
    }
    let host = pump(&mut app, &mut w, out);
    assert_eq!(app.screen(), &Screen::Live);
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
    let state = app.live().unwrap().state();
    assert_eq!(state.tasks.keys().collect::<Vec<_>>(), ["p", "t"]);

    // Bob asks to join `t` and Alice admits him: the App's Person owns the task.
    let request = bob_requests(&mut w, "t");
    let sent = alice(&mut w, accept(request));
    let host = deliver(&mut app, sent);
    assert_eq!(notifies(&host), 1, "{host:?}");
    assert!(
        host.iter().any(|e| matches!(e, Effect::Draw(_))),
        "the tree redraws"
    );
}
