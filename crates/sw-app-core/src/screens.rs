//! The screens that are not the live tree, as `sw-graph` draw lists like the tree itself: a
//! title, short lines, and one `Region` button per next action, so every action is reachable by
//! keyboard and screen reader on a CW-5 host (and by pointer hit-testing on an ABI-1 host).

use sw_graph::{DrawOp, RegionRole};

use crate::copy;
use crate::ctx::{AppTheme, Size};
use crate::screen::{EmptyReason, LinkScreen, NextAction, Screen};

struct Column<'t> {
    theme: &'t AppTheme,
    ops: Vec<DrawOp>,
    y: f32,
}

impl<'t> Column<'t> {
    fn new(theme: &'t AppTheme) -> Self {
        Self {
            theme,
            ops: vec![DrawOp::Clear {
                color: theme.graph.background,
            }],
            y: theme.margin,
        }
    }

    fn text(&mut self, text: &str, px: f32, color: u32) {
        self.y += px.max(self.theme.line_height);
        self.ops.push(DrawOp::FillText {
            x: self.theme.margin,
            y: self.y,
            text: text.to_owned(),
            size: px,
            color,
        });
    }

    fn lines<S: AsRef<str>>(&mut self, lines: &[S], color: u32) {
        for line in lines {
            self.text(line.as_ref(), self.theme.body_px, color);
        }
    }

    fn field(&mut self, link: &LinkScreen) {
        let t = self.theme;
        let (w, h) = t.button_size;
        self.y += t.line_height / 2.0;
        let (x, y) = (t.margin, self.y);
        let shown: String = (0..sw_protocol::payload::Link::CODE_LEN)
            .map(|i| link.entry.code().chars().nth(i).unwrap_or('_'))
            .collect();
        self.ops.push(DrawOp::FillRect {
            x,
            y,
            w,
            h,
            color: t.field,
        });
        self.ops.push(DrawOp::StrokeRect {
            x,
            y,
            w,
            h,
            line_width: t.graph.edge_width,
            color: t.muted,
        });
        self.ops.push(DrawOp::FillText {
            x: x + t.body_px / 2.0,
            y: y + (h + t.body_px) / 2.0,
            text: shown,
            size: t.body_px,
            color: t.text,
        });
        self.ops.push(DrawOp::Region {
            id: "link-code".into(),
            x,
            y,
            w,
            h,
            label: format!("{}: {}", copy::LINK_FIELD_LABEL, link.entry.code()),
            role: RegionRole::Group,
        });
        self.y += h;
    }

    fn buttons(&mut self, actions: &[NextAction]) {
        let t = self.theme;
        let (w, h) = t.button_size;
        let y = self.y + t.line_height;
        for (i, action) in actions.iter().enumerate() {
            let x = t.margin + i as f32 * (w + t.margin);
            self.ops.push(DrawOp::FillRect {
                x,
                y,
                w,
                h,
                color: t.button,
            });
            self.ops.push(DrawOp::FillText {
                x: x + t.body_px / 2.0,
                y: y + (h + t.body_px) / 2.0,
                text: action.label().to_owned(),
                size: t.body_px,
                color: t.button_text,
            });
            self.ops.push(DrawOp::Region {
                id: action.id().to_owned(),
                x,
                y,
                w,
                h,
                label: action.label().to_owned(),
                role: RegionRole::Button,
            });
        }
        self.y = y + h;
    }
}

/// The draw list of every screen but [`Screen::Live`] (an empty list for that one).
pub fn ops(screen: &Screen, theme: &AppTheme) -> Vec<DrawOp> {
    let mut c = Column::new(theme);
    let (title, lines, color): (&str, Vec<String>, u32) = match screen {
        Screen::Live => return Vec::new(),
        Screen::Connecting => (
            copy::CONNECTING_TITLE,
            own(copy::CONNECTING_BODY),
            theme.muted,
        ),
        Screen::NeedsLink(_) => (copy::LINK_TITLE, own(copy::LINK_BODY), theme.muted),
        Screen::Empty(EmptyReason::NoProject) => (
            copy::EMPTY_NO_PROJECT_TITLE,
            own(copy::EMPTY_NO_PROJECT_BODY),
            theme.muted,
        ),
        Screen::Empty(EmptyReason::NothingYet) => (
            copy::EMPTY_NOTHING_TITLE,
            own(copy::EMPTY_NOTHING_BODY),
            theme.muted,
        ),
        Screen::Error(e) => (copy::ERROR_TITLE, e.lines(), theme.error),
    };
    c.text(title, theme.title_px, theme.text);
    c.lines(&lines, color);
    if let Screen::NeedsLink(link) = screen {
        c.field(link);
        if link.pending.is_some() {
            c.lines(&[copy::LINK_PENDING], theme.muted);
        } else if let Some(refused) = &link.refused {
            c.lines(&[copy::LINK_REFUSED], theme.error);
            if let Some(reason) = &refused.reason {
                c.lines(&[reason], theme.muted);
            }
        }
    }
    c.buttons(&screen.actions());
    c.ops
}

/// The prompt composer (design §7.5): read-only until this browser holds a Controller key
/// paired with `sw-mcp trust`. Sending prompts is not part of this crate yet.
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

/// The live tree's footer: what the composer may do.
pub fn composer_footer(composer: Composer, theme: &AppTheme, size: Size) -> Option<DrawOp> {
    (composer == Composer::ReadOnly).then(|| DrawOp::FillText {
        x: theme.margin,
        y: size.height - theme.margin,
        text: copy::COMPOSER_READ_ONLY.to_owned(),
        size: theme.body_px,
        color: theme.muted,
    })
}

fn own(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|s| (*s).to_owned()).collect()
}
