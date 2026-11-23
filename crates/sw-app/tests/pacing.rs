#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! The App's own host-call budget against the manifest's rate, and the host's answers to a
//! draw and a notification as the core's inputs.

use citadel_app_sdk_types::Refused;
use sw_app::config;
use sw_app::map::{draw_reply, notify_reply};
use sw_app_core::Input;

/// `MAX_CALL_RATE_PER_S` is the manifest's `limits.max_call_rate_per_s`, which the manifest
/// test in citadel-workspace compares with it, as it does the id, server, host and session.
#[test]
fn the_budget_is_well_under_the_manifest_call_rate_and_leaves_a_reserve() {
    let budget = config::HOST_CALLS;
    assert!(
        budget.per_s * 3 <= config::MAX_CALL_RATE_PER_S * 2,
        "{} calls a second is more than two thirds of the manifest's {}",
        budget.per_s,
        config::MAX_CALL_RATE_PER_S
    );
    assert!(budget.reserve >= 2 && budget.reserve < budget.per_s);
    assert!(
        budget.per_s - budget.reserve >= 60,
        "draws keep up with a 60 Hz animation"
    );
}

#[test]
fn a_refused_draw_is_the_cores_to_retry_and_a_taken_one_is_nothing() {
    assert_eq!(draw_reply(&Ok::<(), Refused>(())), None);
    let refused: Result<(), Refused> = Err(Refused::Limit("host_calls".into()));
    assert_eq!(draw_reply(&refused), Some(Input::DrawRefused));
}

#[test]
fn a_notification_answer_names_its_event() {
    let id = [3; 32];
    assert_eq!(
        notify_reply(id, &Ok::<(), Refused>(())),
        Input::NotifyDelivered { event_id: id }
    );
    let refused: Result<(), Refused> = Err(Refused::Limit("host_calls".into()));
    assert_eq!(
        notify_reply(id, &refused),
        Input::NotifyRefused { event_id: id }
    );
}
