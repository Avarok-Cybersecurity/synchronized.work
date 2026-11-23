//! The prompt composer (design §7.5) and the live tree's footer that states what it may do.

use sw_graph::DrawOp;

use crate::copy;
use crate::ctx::{AppTheme, Size};
use crate::painter::TextMeasure;
use crate::screens::inner_width;
use crate::wrap::wrap;

/// Read-only until this browser holds a Controller key paired with `sw-mcp trust`. Sending
/// prompts is not part of this crate yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Composer {
    ReadOnly,
    Paired,
}

impl Composer {
    pub fn new(controller_paired: bool) -> Self {
        if controller_paired {
            Self::Paired
        } else {
            Self::ReadOnly
        }
    }
}

/// The footer's text and where it starts: the tree is fitted above `top`.
#[derive(Debug, Clone, PartialEq)]
pub struct Footer {
    pub ops: Vec<DrawOp>,
    /// The top of the footer's first line (the pane's bottom margin when there is none).
    pub top: f32,
}

/// What the composer may do, wrapped to the pane, its last line at the bottom margin.
pub fn footer(
    composer: Composer,
    theme: &AppTheme,
    size: Size,
    measure: &dyn TextMeasure,
) -> Footer {
    let bottom = size.height - theme.margin;
    if composer != Composer::ReadOnly {
        return Footer {
            ops: Vec::new(),
            top: bottom,
        };
    }
    let lines = wrap(
        copy::COMPOSER_READ_ONLY,
        theme.body_px,
        inner_width(theme, size),
        measure,
    );
    let rows = lines.len();
    let first = bottom - rows.saturating_sub(1) as f32 * theme.line_height;
    let ops = lines
        .into_iter()
        .enumerate()
        .map(|(i, text)| DrawOp::FillText {
            x: theme.margin,
            y: bottom - (rows - 1 - i) as f32 * theme.line_height,
            text,
            size: theme.body_px,
            color: theme.muted,
        })
        .collect();
    let top = if rows == 0 {
        bottom
    } else {
        first - theme.body_px
    };
    Footer { ops, top }
}

/// The footer's draw list alone.
pub fn composer_footer(
    composer: Composer,
    theme: &AppTheme,
    size: Size,
    measure: &dyn TextMeasure,
) -> Vec<DrawOp> {
    footer(composer, theme, size, measure).ops
}
