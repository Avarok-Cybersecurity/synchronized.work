#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! Every screen that is not the tree fits a narrow pane: each text line, button and field
//! lies inside the pane's margins at 390 px, measured with the test `TextMeasure`. And a
//! refused open says, in plain words, what happened and what to do, never a bare code.

mod support;

use support::*;
use sw_app_core::link::{CodeEntry, Refused};
use sw_app_core::screen::LinkScreen;
use sw_app_core::screens::{self, Composer};
use sw_app_core::*;
use sw_graph::{DrawOp, Motion};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const NARROW: Size = Size {
    width: 390.0,
    height: 844.0,
};

/// Every error, with reasons as long as the host or the node may give; a new variant fails
/// to compile here until it is listed.
fn every_error() -> Vec<AppError> {
    let long = "a-reason-with-no-spaces-at-all-that-is-longer-than-the-pane".repeat(2);
    let mut errors = vec![
        AppError::NoTransport,
        AppError::NoIdentity,
        AppError::NotLinkedOnFallback,
        AppError::SessionClosed {
            reason: long.clone(),
        },
        AppError::Wire,
        AppError::Send {
            reason: long.clone(),
        },
        AppError::Receive {
            reason: long.clone(),
        },
        AppError::Tree {
            reason: long.clone(),
        },
        AppError::Http { status: 503 },
        AppError::HttpFailed {
            reason: long.clone(),
        },
        AppError::Storage {
            reason: long.clone(),
        },
        AppError::Frame(FrameError::TooManyRegions { regions: 513 }),
    ];
    for reason in REASONS.iter().copied().chain([long.as_str()]) {
        errors.push(AppError::OpenFailed {
            reason: reason.into(),
        });
    }
    for e in &errors {
        match e {
            AppError::NoTransport
            | AppError::NoIdentity
            | AppError::NotLinkedOnFallback
            | AppError::SessionClosed { .. }
            | AppError::OpenFailed { .. }
            | AppError::Wire
            | AppError::Send { .. }
            | AppError::Receive { .. }
            | AppError::Tree { .. }
            | AppError::Http { .. }
            | AppError::HttpFailed { .. }
            | AppError::Storage { .. }
            | AppError::Frame(_) => {}
        }
    }
    errors
}

/// The reasons an open can be refused with: the agent's `AppSessionFailure.reason` names and
/// the binding's `Refused` forms.
const REASONS: &[&str] = &[
    "ungranted",
    "limit",
    "limit: session_rate",
    "policy: server",
    "remote: 503",
    "invalid",
    "identity_locked",
    "not_configured",
    "session",
    "authority",
    "unavailable",
];

fn every_screen() -> Vec<Screen> {
    let mut typed = CodeEntry::default();
    for k in ["A", "B", "C", "D", "1", "2", "3", "4"] {
        typed.key(k);
    }
    let mut screens = vec![
        Screen::Connecting,
        Screen::NeedsLink(LinkScreen::default()),
        Screen::NeedsLink(LinkScreen {
            entry: typed.clone(),
            pending: Some([7; 32]),
            refused: None,
        }),
        Screen::NeedsLink(LinkScreen {
            entry: typed,
            pending: None,
            refused: Some(Refused {
                reason: Some("the code expired a while ago; ask for a new one".repeat(3)),
            }),
        }),
        Screen::Empty(EmptyReason::NoProject),
        Screen::Empty(EmptyReason::NothingYet),
    ];
    screens.extend(every_error().into_iter().map(Screen::Error));
    screens
}

fn assert_fits(ops: &[DrawOp], what: &str) {
    let t = theme(Motion::Lite);
    let (left, right) = (t.margin, NARROW.width - t.margin);
    let inside = |x: f32, w: f32| x >= left - 0.01 && x + w <= right + 0.01;
    for op in ops {
        match op {
            DrawOp::FillText { x, text, size, .. } => {
                let w = FixedAdvance.width(text, *size);
                assert!(inside(*x, w), "{what}: {text:?} spans {x}..{}", x + w);
            }
            DrawOp::FillRect { x, w, .. }
            | DrawOp::StrokeRect { x, w, .. }
            | DrawOp::Region { x, w, .. } => {
                assert!(inside(*x, *w), "{what}: a box spans {x}..{}", x + w);
            }
            _ => {}
        }
    }
}

#[test]
fn every_screen_fits_a_390_px_pane() {
    let t = theme(Motion::Lite);
    for screen in every_screen() {
        let ops = screens::ops(&screen, &t, NARROW, &FixedAdvance);
        assert!(ops.len() > 1, "{screen:?} draws something");
        assert_fits(&ops, &format!("{screen:?}"));
    }
    let footer = screens::composer_footer(Composer::ReadOnly, &t, NARROW, &FixedAdvance);
    assert!(!footer.is_empty());
    assert_fits(&footer, "the composer footer");
}

#[test]
fn the_buttons_of_a_narrow_screen_do_not_overlap() {
    let t = theme(Motion::Lite);
    let ops = screens::ops(
        &Screen::Error(AppError::NoIdentity),
        &t,
        NARROW,
        &FixedAdvance,
    );
    let boxes: Vec<(f32, f32, f32, f32)> = ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::Region { x, y, w, h, .. } => Some((*x, *y, *w, *h)),
            _ => None,
        })
        .collect();
    assert_eq!(boxes.len(), 2);
    let (a, b) = (boxes[0], boxes[1]);
    let apart = a.0 + a.2 <= b.0 || b.0 + b.2 <= a.0 || a.1 + a.3 <= b.1 || b.1 + b.3 <= a.1;
    assert!(apart, "{a:?} and {b:?} overlap");
}

#[test]
fn a_refused_open_says_what_happened_without_its_code() {
    for reason in REASONS {
        let lines = AppError::OpenFailed {
            reason: (*reason).into(),
        }
        .lines();
        let shown = lines.join(" ");
        assert!(lines.len() >= 2, "{reason}: what happened and a next step");
        assert!(
            !shown.contains(copy::REFUSED_DETAIL),
            "{reason:?} not recognised: {shown}"
        );
        assert!(!shown.contains(['_', ':']), "{reason:?}: a code in {shown}");
    }
    let (ungranted, unreachable) = (
        AppError::OpenFailed {
            reason: "ungranted".into(),
        }
        .lines(),
        AppError::OpenFailed {
            reason: "unavailable".into(),
        }
        .lines(),
    );
    assert_ne!(ungranted, unreachable, "each refusal has its own copy");
}

#[test]
fn an_unknown_refusal_is_explained_and_keeps_its_reason_as_a_detail() {
    let lines = AppError::OpenFailed {
        reason: "agent offline".into(),
    }
    .lines();
    assert!(lines.len() >= 3, "{lines:?}");
    assert!(lines[0].contains("could not connect"), "{lines:?}");
    assert!(
        lines.last().unwrap().ends_with("agent offline"),
        "{lines:?}"
    );
}
