#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The onboarding path end to end over the real Citadel SDK, against the production sw-node
//! kernel in-process (`support::onboard_flow` has the steps; `onboard_wrangler.rs` runs the
//! same steps against `wrangler dev`). The node starts empty: the project, the invite and the
//! admission are all made through sw-mcp. No stand-ins on the path; the sign-in issuer is the
//! C0 KAT issuer, as in every daemon test.

mod support;

use support::pad_world::{MemStore, start_node};

#[tokio::test(flavor = "multi_thread")]
async fn an_owner_invites_a_friend_who_joins_and_they_exchange_a_note_and_an_ack() {
    let (port, _core) = start_node(MemStore::default());
    support::onboard_flow::run("onboard", port).await;
}
