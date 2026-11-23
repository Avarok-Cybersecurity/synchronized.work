//! One realistic story of the shipped Citadel App on a fresh install, recorded from the real
//! node core and the real `sw_app_core::App`: the App links with a code, follows a Project of
//! three Tasks, two Persons and one Agent, and later sees a second Person admitted to a Task.
//! What crossed its session is committed as `fixtures/app-session/joined.json` ([`fixture`])
//! so another repository can play it to the shipped module ([`replay`] proves that works).
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod fixture;
pub mod replay;

use crate::app_link::{Session, Step};
use crate::app_support::{FixedAdvance, ctx, granted};
use crate::feed_support::*;
use crate::support::{Who, World};
use sw_app_core::notify::HostNotify;
use sw_app_core::{App, AppCtx, CanvasAbi, Effect, Screen};
use sw_graph::Motion;
use sw_model::{EndpointKind, Lane, Visibility};
use sw_protocol::body::Audience;
use sw_protocol::payload::{Payload, StatusUpdate};

/// The code Alice minted, typed exactly so.
pub const CODE: &str = "K7M2Q9XW";

/// The test name that writes and checks the committed transcript.
pub const GENERATED_BY: &str = "sw-node/tests/app_session_fixture.rs::the_committed_transcript_is_what_the_node_and_the_app_produce";

/// The fixture's path in the repository.
pub const PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/app-session/joined.json"
);

/// The App as the shipped module runs it: `sw-app`'s configuration (its server and `HELLO`
/// `client_build`), in this context.
pub fn shipped(ctx: AppCtx) -> App<FixedAdvance> {
    App::new(ctx, sw_app::config::app_config(), FixedAdvance)
}

/// The context the story is recorded in (the support's test install on node `node-7`).
pub fn recorded_ctx() -> AppCtx {
    ctx(granted(true, false), CanvasAbi::Cw5, Motion::Lite)
}

/// Alice's Project `p` with Tasks `t` and `u`; Bob was admitted to `u`, where his Agent
/// works, and has asked to join `t`; Alice minted [`CODE`]. Returns Bob's pending request.
pub fn world() -> (World, [u8; 32]) {
    let mut w = World::enrolled();
    alice(&mut w, create("p", None, Visibility::Members));
    alice(&mut w, create("t", Some("p"), Visibility::Members));
    alice(&mut w, create("u", Some("p"), Visibility::Members));
    alice(&mut w, open_up("t"));
    alice(&mut w, open_up("u"));
    let on_u = bob_requests(&mut w, "u");
    alice(&mut w, accept(on_u));
    let status = Payload::StatusUpdate(StatusUpdate {
        agent_id: "bob-agent".into(),
        task_id: "u".into(),
        lane: Lane::Running,
        blockers: Vec::new(),
        harness: "claude-code".into(),
    });
    w.send_as(Who::Bob, EndpointKind::Device, Audience::Node, status)
        .unwrap();
    let on_t = bob_requests(&mut w, "t");
    mint(&mut w, CODE);
    (w, on_t)
}

/// The labels of the App's tree, in the order `sw_view::points` yields them.
pub fn labels(app: &App<FixedAdvance>) -> Vec<String> {
    let state = app.live().unwrap().state();
    let name = sw_app_core::graph::project_name(state);
    sw_view::points::points(state, &name)
        .into_iter()
        .map(|p| p.label)
        .collect()
}

pub fn host_notifies(effects: &[Effect]) -> Vec<HostNotify> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Notify(n) => Some(n.clone()),
            _ => None,
        })
        .collect()
}

/// What the story produced: the session up to the live tree, the tail that admits Bob to `t`,
/// and what the App showed after each.
pub struct Recorded {
    pub ctx: AppCtx,
    pub steps: Vec<Step>,
    pub tail: Vec<Step>,
    pub project_name: String,
    pub after_catch_up: Vec<String>,
    pub after_join: Vec<String>,
    pub notified: Vec<HostNotify>,
}

pub fn record() -> Recorded {
    let (w, request) = world();
    let ctx = recorded_ctx();
    let mut s = Session::new(shipped(ctx.clone()), w);
    s.open(None);
    assert!(matches!(s.app.screen(), Screen::NeedsLink(_)));
    s.type_line(CODE);
    assert_eq!(s.app.screen(), &Screen::Live);
    let steps = std::mem::take(&mut s.steps);
    let project_name = sw_app_core::graph::project_name(s.app.live().unwrap().state());
    let after_catch_up = labels(&s.app);

    let sent = alice(&mut s.w, accept(request));
    let host = s.deliver(sent);
    Recorded {
        ctx,
        tail: std::mem::take(&mut s.steps),
        steps,
        project_name,
        after_catch_up,
        after_join: labels(&s.app),
        notified: host_notifies(&host),
    }
}
