//! CW-5 hosts (`sdk_abi_minor >= 1`): every draw op has a command of its own.

use sw_graph::{DrawOp, Painter};

use super::{Collector, Frame, FrameError, TextMeasure};
use crate::command::DrawCommand;

#[derive(Debug)]
pub struct CitadelPainter<M> {
    measure: M,
    out: Collector,
}

impl<M: TextMeasure> CitadelPainter<M> {
    pub fn new(measure: M) -> Self {
        Self {
            measure,
            out: Collector::default(),
        }
    }

    /// The frame painted since the last `finish`, or the first contract limit it broke.
    pub fn finish(&mut self) -> Result<Frame, FrameError> {
        self.out.finish()
    }
}

impl<M: TextMeasure> Painter for CitadelPainter<M> {
    fn measure_text(&self, text: &str, size: f32) -> f32 {
        self.measure.width(text, size)
    }

    fn paint(&mut self, ops: &[DrawOp]) {
        for op in ops {
            let command = match op.clone() {
                DrawOp::Clear { color } => DrawCommand::Clear { color },
                DrawOp::FillRect { x, y, w, h, color } => {
                    DrawCommand::FillRect { x, y, w, h, color }
                }
                DrawOp::StrokeRect {
                    x,
                    y,
                    w,
                    h,
                    line_width,
                    color,
                } => DrawCommand::StrokeRect {
                    x,
                    y,
                    w,
                    h,
                    line_width,
                    color,
                },
                DrawOp::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    line_width,
                    color,
                } => DrawCommand::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    line_width,
                    color,
                },
                DrawOp::FillText {
                    x,
                    y,
                    text,
                    size,
                    color,
                } => DrawCommand::FillText {
                    x,
                    y,
                    text,
                    px: size,
                    color,
                },
                DrawOp::FillCircle { cx, cy, r, color } => {
                    DrawCommand::FillCircle { cx, cy, r, color }
                }
                DrawOp::StrokeCircle {
                    cx,
                    cy,
                    r,
                    line_width,
                    color,
                } => DrawCommand::StrokeCircle {
                    cx,
                    cy,
                    r,
                    line_width,
                    color,
                },
                DrawOp::Path {
                    points,
                    closed,
                    fill,
                    stroke,
                } => {
                    self.out.path(&points, fill, stroke);
                    DrawCommand::Path {
                        points,
                        closed,
                        fill,
                        stroke,
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
                } => DrawCommand::Arc {
                    cx,
                    cy,
                    r,
                    start,
                    end,
                    line_width,
                    color,
                },
                region @ DrawOp::Region { .. } => match self.out.region(&region) {
                    Some(command) => command,
                    None => continue,
                },
            };
            self.out.commands.push(command);
        }
    }
}
