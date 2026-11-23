//! One frame: the screen's draw list through the painter the host's canvas takes, built with
//! the measures the budget allows; a build that needed more is not drawn (it is built again,
//! from the latest state, once the window has room).

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

    /// Draws the current state if the budget takes the measures it needs and the draw.
    pub(super) fn draw(&mut self, out: &mut Vec<Effect>) {
        let room = self.ledger.room(Tier::Low);
        let Some(measures) = room.checked_sub(1) else {
            return;
        };
        self.text.allow(measures);
        let built = self.build();
        let asked = self.text.settle();
        self.ledger.spend(asked.calls);
        if asked.starved {
            return;
        }
        self.dirty = false;
        match built {
            Ok(frame) => {
                self.ledger.spend(1);
                out.push(Effect::Draw(frame.commands.clone()));
                self.before = Some(self.frame.replace(frame));
            }
            Err(reason) => self.deferred.push_back(Effect::Fault { reason }),
        }
    }
}
