//! One frame: the screen's draw list through the painter the host's canvas takes, built with
//! the measures the budget allows ([`crate::pace::Ledger::draw`]). A build that needed more is
//! drawn with estimates for the rest and built again, from the latest state, on the next frame
//! the budget allows, until one needs no estimate; a rebuild that measured nothing new and
//! looks the same is not drawn again.

use sw_graph::{DrawOp, Painter};

use super::{App, Effect};
use crate::composer;
use crate::ctx::CanvasAbi;
use crate::fit::Area;
use crate::measure::HostText;
use crate::pace::Tier;
use crate::painter::{CitadelPainter, Frame, FrameError, RectOnlyPainter};
use crate::screen::{AppError, Screen};
use crate::screens;

impl<M: HostText> App<M> {
    fn ops(&self) -> Result<Vec<DrawOp>, AppError> {
        let (theme, size) = (&self.ctx.theme, self.ctx.size);
        if self.screen != Screen::Live {
            return Ok(screens::ops(&self.screen, theme, size, &self.text));
        }
        let footer = composer::footer(self.composer, theme, size, &self.text);
        let area = Area {
            x: theme.margin,
            y: theme.margin,
            w: size.width - 2.0 * theme.margin,
            h: footer.top - 2.0 * theme.margin,
        };
        let mut ops = self
            .graph
            .ops(theme, size, area, &self.config.fit, &self.text)
            .map_err(|e| AppError::Tree {
                reason: format!("{e:?}"),
            })?;
        ops.extend(footer.ops);
        Ok(ops)
    }

    fn paint(&self, ops: &[DrawOp]) -> Result<Frame, FrameError> {
        match self.ctx.canvas {
            CanvasAbi::Cw5 => {
                let mut p = CitadelPainter::new(&self.text);
                p.paint(ops);
                p.finish()
            }
            CanvasAbi::Abi1 => {
                let mut p = RectOnlyPainter::new(&self.text, self.config.rect_only);
                p.paint(ops);
                p.finish()
            }
        }
    }

    fn painted(&self) -> Result<Frame, AppError> {
        self.ops()
            .and_then(|ops| self.paint(&ops).map_err(AppError::Frame))
    }

    /// The tree failing to draw becomes the error screen; a screen of this crate's own failing
    /// is a defect and is reported, not drawn.
    fn build(&mut self) -> Result<Frame, String> {
        match self.painted() {
            Ok(frame) => Ok(frame),
            Err(e) if self.screen == Screen::Live => {
                self.screen = Screen::Error(e);
                self.painted().map_err(|e| format!("{e:?}"))
            }
            Err(e) => Err(format!("{e:?}")),
        }
    }

    /// Draws the current state if the budget takes the draw; `frame`: on a frame's tick.
    pub(super) fn draw(&mut self, frame: bool, out: &mut Vec<Effect>) {
        let Some(measures) = self.ledger.draw(frame) else {
            return;
        };
        self.text.allow(measures);
        let built = self.build();
        let asked = self.text.settle();
        self.ledger.spend(Tier::Low, asked.calls);
        self.dirty = asked.estimated;
        match built {
            Ok(frame) => {
                let same = self
                    .frame
                    .as_ref()
                    .is_some_and(|shown| shown.commands == frame.commands);
                if asked.estimated && asked.calls == 0 && same {
                    return;
                }
                self.ledger.spend(Tier::Low, 1);
                self.ledger.drew();
                out.push(Effect::Draw(frame.commands.clone()));
                self.before = Some(self.frame.replace(frame));
            }
            Err(reason) => {
                self.dirty = false;
                self.deferred.push_back(Effect::Fault { reason });
            }
        }
    }
}
