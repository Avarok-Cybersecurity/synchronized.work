#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Every core `DrawCommand` becomes the SDK's command of the same name with the same values,
//! and the result is a frame the SDK's own canvas check admits.

use citadel_app_sdk_types::calls::{CanvasDraw, DrawCommand as Sdk, RegionRole as SdkRole};
use sw_app::map::{draw_command_of, region_role_of};
use sw_app_core::{DrawCommand as Core, RegionRole};

/// The ten commands, each with values distinct from every other field's.
fn pairs() -> Vec<(Core, Sdk)> {
    vec![
        (Core::Clear { color: 1 }, Sdk::Clear { color: 1 }),
        (
            Core::FillRect {
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
                color: 5,
            },
            Sdk::FillRect {
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
                color: 5,
            },
        ),
        (
            Core::StrokeRect {
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
                line_width: 5.0,
                color: 6,
            },
            Sdk::StrokeRect {
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
                line_width: 5.0,
                color: 6,
            },
        ),
        (
            Core::Line {
                x1: 1.0,
                y1: 2.0,
                x2: 3.0,
                y2: 4.0,
                line_width: 5.0,
                color: 6,
            },
            Sdk::Line {
                x1: 1.0,
                y1: 2.0,
                x2: 3.0,
                y2: 4.0,
                line_width: 5.0,
                color: 6,
            },
        ),
        (
            Core::FillText {
                x: 1.0,
                y: 2.0,
                text: "t".into(),
                px: 3.0,
                color: 4,
            },
            Sdk::FillText {
                x: 1.0,
                y: 2.0,
                text: "t".into(),
                px: 3.0,
                color: 4,
            },
        ),
        (
            Core::FillCircle {
                cx: 1.0,
                cy: 2.0,
                r: 3.0,
                color: 4,
            },
            Sdk::FillCircle {
                cx: 1.0,
                cy: 2.0,
                r: 3.0,
                color: 4,
            },
        ),
        (
            Core::StrokeCircle {
                cx: 1.0,
                cy: 2.0,
                r: 3.0,
                line_width: 4.0,
                color: 5,
            },
            Sdk::StrokeCircle {
                cx: 1.0,
                cy: 2.0,
                r: 3.0,
                line_width: 4.0,
                color: 5,
            },
        ),
        (
            Core::Path {
                points: vec![(1.0, 2.0), (3.0, 4.0)],
                closed: true,
                fill: Some(5),
                stroke: Some((6.0, 7)),
            },
            Sdk::Path {
                points: vec![(1.0, 2.0), (3.0, 4.0)],
                closed: true,
                fill: Some(5),
                stroke: Some((6.0, 7)),
            },
        ),
        (
            Core::Arc {
                cx: 1.0,
                cy: 2.0,
                r: 3.0,
                start: 4.0,
                end: 5.0,
                line_width: 6.0,
                color: 7,
            },
            Sdk::Arc {
                cx: 1.0,
                cy: 2.0,
                r: 3.0,
                start: 4.0,
                end: 5.0,
                line_width: 6.0,
                color: 7,
            },
        ),
        (
            Core::Region {
                id: "retry".into(),
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
                label: "Try again".into(),
                role: RegionRole::Link,
            },
            Sdk::Region {
                id: "retry".into(),
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
                label: "Try again".into(),
                role: SdkRole::Link,
            },
        ),
    ]
}

#[test]
fn each_of_the_ten_commands_maps_to_the_sdk_command_with_the_same_values() {
    let pairs = pairs();
    assert_eq!(pairs.len(), 10);
    for (core, sdk) in pairs {
        assert_eq!(draw_command_of(core.clone()), sdk, "{core:?}");
    }
}

#[test]
fn the_mapped_frame_is_one_the_sdks_canvas_check_admits() {
    let commands: Vec<Sdk> = pairs()
        .into_iter()
        .map(|(c, _)| draw_command_of(c))
        .collect();
    assert_eq!(CanvasDraw { commands }.validate(), Ok(()));
}

#[test]
fn each_region_role_maps_to_its_namesake() {
    let roles = [
        (RegionRole::Button, SdkRole::Button),
        (RegionRole::Link, SdkRole::Link),
        (RegionRole::Img, SdkRole::Img),
        (RegionRole::Group, SdkRole::Group),
    ];
    for (core, sdk) in roles {
        assert_eq!(region_role_of(core), sdk);
    }
}
