#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests fail loudly by design

//! The two painters over sw-graph's draw list: what each emits, the contract's limits, and the
//! ABI-1 painter's label contrast.

mod support;

use support::FixedAdvance;
use sw_app_core::command::{is_region_id, limits};
use sw_app_core::painter::{contrast, region_id, region_label};
use sw_app_core::*;
use sw_graph::{DrawOp, Painter, RegionRole as Role};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const PARAMS: RectOnlyParams = RectOnlyParams {
    arc_segments: 4,
    path_line_width: 1.0,
};

fn region(key: &str, x: f32) -> DrawOp {
    DrawOp::Region {
        id: key.into(),
        x,
        y: 0.0,
        w: 10.0,
        h: 10.0,
        label: key.into(),
        role: Role::Button,
    }
}

fn every_op() -> Vec<DrawOp> {
    vec![
        DrawOp::Clear { color: 0xFFFF_FFFF },
        DrawOp::FillCircle {
            cx: 5.0,
            cy: 5.0,
            r: 4.0,
            color: 0x2040_80FF,
        },
        DrawOp::StrokeCircle {
            cx: 5.0,
            cy: 5.0,
            r: 4.0,
            line_width: 1.0,
            color: 0x2040_80FF,
        },
        DrawOp::Path {
            points: vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)],
            closed: true,
            fill: Some(0x2040_80FF),
            stroke: None,
        },
        DrawOp::Arc {
            cx: 5.0,
            cy: 5.0,
            r: 4.0,
            start: 0.0,
            end: 3.0,
            line_width: 1.0,
            color: 0x2040_80FF,
        },
        DrawOp::FillText {
            x: 0.0,
            y: 30.0,
            text: "label".into(),
            size: 12.0,
            color: 0x1010_10FF,
        },
        region("task:t1/person:x", 0.0),
    ]
}

fn cw5(ops: &[DrawOp]) -> Result<Frame, FrameError> {
    let mut p = CitadelPainter::new(FixedAdvance);
    p.paint(ops);
    p.finish()
}

fn abi1(ops: &[DrawOp]) -> Result<Frame, FrameError> {
    let mut p = RectOnlyPainter::new(FixedAdvance, PARAMS);
    p.paint(ops);
    p.finish()
}

#[test]
fn the_citadel_painter_passes_circles_paths_arcs_and_regions_through() {
    let frame = cw5(&every_op()).unwrap();
    let c = &frame.commands;
    assert!(matches!(c[1], DrawCommand::FillCircle { r, .. } if r == 4.0));
    assert!(matches!(c[2], DrawCommand::StrokeCircle { .. }));
    assert!(matches!(&c[3], DrawCommand::Path { points, closed: true, .. } if points.len() == 3));
    assert!(matches!(c[4], DrawCommand::Arc { end, .. } if end == 3.0));
    assert!(matches!(&c[5], DrawCommand::FillText { px, .. } if *px == 12.0));
    let DrawCommand::Region {
        id, label, role, ..
    } = &c[6]
    else {
        panic!("no region: {c:?}")
    };
    assert!(is_region_id(id), "{id}");
    assert_eq!(
        (label.as_str(), *role),
        ("task:t1/person:x", RegionRole::Button)
    );
    assert_eq!(frame.key_of(id), Some("task:t1/person:x"));
}

#[test]
fn the_rect_only_painter_emits_only_abi_1_commands() {
    let frame = abi1(&every_op()).unwrap();
    for c in &frame.commands {
        assert!(
            matches!(
                c,
                DrawCommand::Clear { .. }
                    | DrawCommand::FillRect { .. }
                    | DrawCommand::StrokeRect { .. }
                    | DrawCommand::Line { .. }
                    | DrawCommand::FillText { .. }
            ),
            "not ABI-1: {c:?}"
        );
    }
    let lines = frame
        .commands
        .iter()
        .filter(|c| matches!(c, DrawCommand::Line { .. }))
        .count();
    // A closed 3-point path is 3 lines; the arc is `arc_segments` of them.
    assert_eq!(lines, 3 + 4);
    // The region is not emitted but still hit-tests for the App itself.
    assert_eq!(frame.hit(5.0, 5.0), Some("task:t1/person:x"));
}

#[test]
fn region_ids_fit_the_grammar_and_stay_the_same_across_frames() {
    for key in [
        "task:abc/person:3foriftckvpoy2rh/agent:a-1",
        "Project",
        "x".repeat(200).as_str(),
    ] {
        let id = region_id(key);
        assert!(is_region_id(&id), "{key} -> {id}");
        assert_eq!(id, region_id(key));
    }
    assert_ne!(region_id("task:a"), region_id("task:b"));
    // An id already in the grammar (a screen button) is kept as it is.
    assert_eq!(region_id("retry"), "retry");
}

#[test]
fn a_label_longer_than_256_characters_is_cut_and_marked() {
    let label = "\u{e9}".repeat(300);
    let cut = region_label(&label);
    assert_eq!(cut.chars().count(), limits::REGION_LABEL_CHARS);
    assert!(cut.ends_with('\u{2026}'));
    assert_eq!(region_label("short"), "short");
}

#[test]
fn a_path_outside_2_to_256_points_or_painting_nothing_refuses_the_frame() {
    let path = |n: usize, fill: Option<u32>| DrawOp::Path {
        points: vec![(0.0, 0.0); n],
        closed: false,
        fill,
        stroke: None,
    };
    for paint in [cw5, abi1] {
        assert_eq!(
            paint(&[path(257, Some(1))]),
            Err(FrameError::PathPoints { points: 257 })
        );
        assert_eq!(
            paint(&[path(1, Some(1))]),
            Err(FrameError::PathPoints { points: 1 })
        );
        assert_eq!(paint(&[path(2, None)]), Err(FrameError::PathPaintsNothing));
        assert!(paint(&[path(256, Some(1))]).is_ok());
    }
}

#[test]
fn more_than_512_regions_refuse_the_frame() {
    let ops: Vec<DrawOp> = (0..513)
        .map(|i| region(&format!("n{i}"), i as f32))
        .collect();
    assert_eq!(cw5(&ops), Err(FrameError::TooManyRegions { regions: 513 }));
    assert!(cw5(&ops[..512]).is_ok());
}

#[test]
fn the_rect_only_painter_keeps_labels_legible_on_what_they_are_drawn_on() {
    let pale = 0xEEEE_EEFF;
    let ops = vec![
        DrawOp::Clear { color: 0xFFFF_FFFF },
        // A label in the background's own colour: unreadable as given.
        DrawOp::FillText {
            x: 0.0,
            y: 100.0,
            text: "on the page".into(),
            size: 12.0,
            color: pale,
        },
        DrawOp::FillRect {
            x: 0.0,
            y: 0.0,
            w: 50.0,
            h: 50.0,
            color: 0x1010_10FF,
        },
        DrawOp::FillText {
            x: 5.0,
            y: 30.0,
            text: "on a dark button".into(),
            size: 12.0,
            color: 0x2020_20FF,
        },
    ];
    let frame = abi1(&ops).unwrap();
    let texts: Vec<(u32, u32)> = frame
        .commands
        .iter()
        .filter_map(|c| match c {
            DrawCommand::FillText { color, y, .. } if *y > 50.0 => Some((*color, 0xFFFF_FFFF)),
            DrawCommand::FillText { color, .. } => Some((*color, 0x1010_10FF)),
            _ => None,
        })
        .collect();
    assert_eq!(texts.len(), 2);
    for (fg, bg) in texts {
        assert!(
            contrast::ratio(fg, bg) >= contrast::MIN_TEXT_CONTRAST,
            "{fg:08x} on {bg:08x}"
        );
    }
}
