//! The draw-list IR (design §7.3). ABI-1 has `Clear, FillRect, StrokeRect, Line, FillText`;
//! CW-5 adds circles, paths, arcs and `Region`. A painter that lacks the CW-5 ops (the ABI-1
//! fallback) maps them to rects and lines; the IR stays one.

/// Colours are `0xRRGGBBAA`.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawOp {
    Clear {
        color: u32,
    },
    FillRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: u32,
    },
    StrokeRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        line_width: f32,
        color: u32,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        line_width: f32,
        color: u32,
    },
    FillText {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        color: u32,
    },
    FillCircle {
        cx: f32,
        cy: f32,
        r: f32,
        color: u32,
    },
    StrokeCircle {
        cx: f32,
        cy: f32,
        r: f32,
        line_width: f32,
        color: u32,
    },
    /// At most [`DrawOp::MAX_PATH_POINTS`] points.
    Path {
        points: Vec<(f32, f32)>,
        closed: bool,
        fill: Option<u32>,
        stroke: Option<(f32, u32)>,
    },
    Arc {
        cx: f32,
        cy: f32,
        r: f32,
        start: f32,
        end: f32,
        line_width: f32,
        color: u32,
    },
    /// An invisible, focusable hit region the host renders for screen readers and pointer
    /// hit-testing; activation comes back as `AppEvent::Activate { id }`.
    Region {
        id: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        label: String,
        role: RegionRole,
    },
}

impl DrawOp {
    pub const MAX_PATH_POINTS: usize = 256;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionRole {
    Button,
    Link,
    Img,
    Group,
}

/// The output port: the App's ABI canvas (`CitadelPainter`, `RectOnlyPainter`) or the
/// site's Canvas 2D. `measure_text` sizes labels before layout.
pub trait Painter {
    fn measure_text(&self, text: &str, size: f32) -> f32;
    fn paint(&mut self, ops: &[DrawOp]);
}
