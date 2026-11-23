//! One frame: the screen's draw list through the painter the host's canvas takes.

use sw_graph::{DrawOp, Painter};

use super::{App, Effect};
use crate::ctx::CanvasAbi;
use crate::painter::{CitadelPainter, Frame, FrameError, RectOnlyPainter, TextMeasure};
use crate::screen::{AppError, Screen};
use crate::screens;

impl<M: TextMeasure> App<M> {
    fn ops(&self) -> Vec<DrawOp> {
        if self.screen != Screen::Live {
            return screens::ops(&self.screen, &self.ctx.theme);
        }
        let mut ops = self.graph.ops(&self.ctx.theme);
        ops.extend(screens::composer_footer(
            self.composer,
            &self.ctx.theme,
            self.ctx.size,
        ));
        ops
    }

    fn paint(&self, ops: &[DrawOp]) -> Result<Frame, FrameError> {
        match self.ctx.canvas {
            CanvasAbi::Cw5 => {
                let mut p = CitadelPainter::new(&self.measure);
                p.paint(ops);
                p.finish()
            }
            CanvasAbi::Abi1 => {
                let mut p = RectOnlyPainter::new(&self.measure, self.config.rect_only);
                p.paint(ops);
                p.finish()
            }
        }
    }

    /// The tree breaking a canvas limit becomes the error screen; a screen of this crate's
    /// own breaking one is a defect and is reported, not drawn.
    pub(super) fn draw(&mut self, out: &mut Vec<Effect>) {
        let frame = match self.paint(&self.ops()) {
            Ok(frame) => frame,
            Err(e) if self.screen == Screen::Live => {
                self.screen = Screen::Error(AppError::Frame(e));
                match self.paint(&self.ops()) {
                    Ok(frame) => frame,
                    Err(e) => {
                        return out.push(Effect::Fault {
                            reason: format!("{e:?}"),
                        });
                    }
                }
            }
            Err(e) => {
                return out.push(Effect::Fault {
                    reason: format!("{e:?}"),
                });
            }
        };
        out.push(Effect::Draw(frame.commands.clone()));
        self.frame = Some(frame);
    }
}
