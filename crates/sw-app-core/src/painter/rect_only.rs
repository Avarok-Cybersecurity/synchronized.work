//! ABI-1 hosts (`sdk_abi_minor == 0`): only `Clear, FillRect, StrokeRect, Line, FillText`.
//! Circles become their bounding squares, a path its segments, an arc a polyline; regions
//! cannot be expressed, so they are not emitted and the App hit-tests pointer input against
//! them itself ([`Frame::hit`]). Every label is checked against what it is drawn on
//! (the topmost filled rect under it, else the cleared background) and recoloured to black or
//! white when it falls below [`contrast::MIN_TEXT_CONTRAST`].

use sw_graph::{DrawOp, Painter};

use super::{Collector, Frame, FrameError, TextMeasure, contrast};
use crate::command::DrawCommand;

/// Explicit (PCND): how a curve is approximated with lines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectOnlyParams {
    /// Line segments per arc (at least 1).
    pub arc_segments: u16,
    /// Line width for a path that has only a fill.
    pub path_line_width: f32,
}

#[derive(Debug)]
pub struct RectOnlyPainter<M> {
    measure: M,
    params: RectOnlyParams,
    out: Collector,
    background: Option<u32>,
    fills: Vec<(f32, f32, f32, f32, u32)>,
}

impl<M: TextMeasure> RectOnlyPainter<M> {
    pub fn new(measure: M, params: RectOnlyParams) -> Self {
        Self {
            measure,
            params,
            out: Collector::default(),
            background: None,
            fills: Vec::new(),
        }
    }

    pub fn finish(&mut self) -> Result<Frame, FrameError> {
        self.background = None;
        self.fills.clear();
        self.out.finish()
    }

    fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, color: u32) {
        self.fills.push((x, y, w, h, color));
        self.out
            .commands
            .push(DrawCommand::FillRect { x, y, w, h, color });
    }

    fn under(&self, x: f32, y: f32) -> Option<u32> {
        self.fills
            .iter()
            .rev()
            .find(|(fx, fy, fw, fh, _)| x >= *fx && x <= fx + fw && y >= *fy && y <= fy + fh)
            .map(|f| f.4)
            .or(self.background)
    }

    fn line(&mut self, (x1, y1): (f32, f32), (x2, y2): (f32, f32), line_width: f32, color: u32) {
        self.out.commands.push(DrawCommand::Line {
            x1,
            y1,
            x2,
            y2,
            line_width,
            color,
        });
    }
}

impl<M: TextMeasure> Painter for RectOnlyPainter<M> {
    fn measure_text(&self, text: &str, size: f32) -> f32 {
        self.measure.width(text, size)
    }

    fn paint(&mut self, ops: &[DrawOp]) {
        for op in ops {
            match op.clone() {
                DrawOp::Clear { color } => {
                    self.background = Some(color);
                    self.fills.clear();
                    self.out.commands.push(DrawCommand::Clear { color });
                }
                DrawOp::FillRect { x, y, w, h, color } => self.fill(x, y, w, h, color),
                DrawOp::FillCircle { cx, cy, r, color } => {
                    self.fill(cx - r, cy - r, 2.0 * r, 2.0 * r, color)
                }
                DrawOp::StrokeRect {
                    x,
                    y,
                    w,
                    h,
                    line_width,
                    color,
                } => self.out.commands.push(DrawCommand::StrokeRect {
                    x,
                    y,
                    w,
                    h,
                    line_width,
                    color,
                }),
                DrawOp::StrokeCircle {
                    cx,
                    cy,
                    r,
                    line_width,
                    color,
                } => self.out.commands.push(DrawCommand::StrokeRect {
                    x: cx - r,
                    y: cy - r,
                    w: 2.0 * r,
                    h: 2.0 * r,
                    line_width,
                    color,
                }),
                DrawOp::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    line_width,
                    color,
                } => self.line((x1, y1), (x2, y2), line_width, color),
                DrawOp::FillText {
                    x,
                    y,
                    text,
                    size,
                    color,
                } => {
                    let on = self.under(x, y - size / 2.0);
                    let color = on.map_or(color, |bg| contrast::legible(color, bg));
                    self.out.commands.push(DrawCommand::FillText {
                        x,
                        y,
                        text,
                        px: size,
                        color,
                    });
                }
                DrawOp::Path {
                    points,
                    closed,
                    fill,
                    stroke,
                } => {
                    self.out.path(&points, fill, stroke);
                    let Some((width, color)) =
                        stroke.or(fill.map(|c| (self.params.path_line_width, c)))
                    else {
                        continue;
                    };
                    for pair in points.windows(2) {
                        self.line(pair[0], pair[1], width, color);
                    }
                    if let (true, Some(first), Some(last)) = (closed, points.first(), points.last())
                    {
                        self.line(*last, *first, width, color);
                    }
                }
                DrawOp::Arc {
                    cx,
                    cy,
                    r,
                    start,
                    end,
                    line_width,
                    color,
                } => {
                    let n = self.params.arc_segments.max(1);
                    let at = |i: u16| {
                        let a = start + (end - start) * f32::from(i) / f32::from(n);
                        (cx + r * a.cos(), cy + r * a.sin())
                    };
                    for i in 0..n {
                        self.line(at(i), at(i + 1), line_width, color);
                    }
                }
                // Not an ABI-1 command: recorded for the App's own hit-testing, never emitted.
                region @ DrawOp::Region { .. } => {
                    let _command = self.out.region(&region);
                }
            }
        }
    }
}
