#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The onboarding steps of `onboard_e2e.rs` against the real Worker under `wrangler dev`,
//! provisioned with the C0 KAT sign-in issuer: `tools/onboard-e2e.sh` starts it and runs this
//! test with `SW_E2E_WRANGLER_PORT` set. Ignored in a plain `cargo test`, which has no Worker;
//! CI's worker job runs it through that script.

mod support;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs wrangler dev: run tools/onboard-e2e.sh"]
async fn against_wrangler_dev_an_owner_invites_a_friend_who_joins_and_they_exchange_notes() {
    let port: u16 = std::env::var("SW_E2E_WRANGLER_PORT")
        .expect("SW_E2E_WRANGLER_PORT: run this through tools/onboard-e2e.sh")
        .parse()
        .unwrap();
    support::onboard_flow::run("wrangler", port).await;
}
