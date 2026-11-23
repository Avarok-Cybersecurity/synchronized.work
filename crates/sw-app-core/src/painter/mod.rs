//! The two App painters of design §7.3, pure translators from `sw-graph`'s one draw list to
//! [`DrawCommand`]s: [`CitadelPainter`] (CW-5: circles, paths, arcs and regions pass through)
//! and [`RectOnlyPainter`] (ABI-1: rects, lines and text only). Both implement
//! [`sw_graph::Painter`], collect a frame, and refuse it whole in [`Frame`] form if it breaks
//! a contract limit — the host would refuse it anyway, so nothing half-drawn is ever sent.

mod citadel;
pub mod contrast;
mod rect_only;

pub use citadel::CitadelPainter;
pub use rect_only::{RectOnlyPainter, RectOnlyParams};

use crate::command::{DrawCommand, is_region_id, limits};

/// Text widths come from the host (`canvas.MeasureText`); the binding implements this.
pub trait TextMeasure {
    fn width(&self, text: &str, px: f32) -> f32;
}

/// One frame, ready for `CanvasDraw`, and the regions it drew: on a CW-5 host the host
/// reports `Activate { id }` for them; on an ABI-1 host (no `Region` command) the App
/// hit-tests pointer input against the same rectangles.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub commands: Vec<DrawCommand>,
    pub regions: Vec<FrameRegion>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameRegion {
    /// As emitted ([`region_id`] of `key`).
    pub id: String,
    /// The draw list's key: a screen action or a graph node.
    pub key: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Frame {
    /// The key of the region with emitted id `id`.
    pub fn key_of(&self, id: &str) -> Option<&str> {
        self.regions
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.key.as_str())
    }

    /// The key of the topmost region containing `(x, y)`.
    pub fn hit(&self, x: f32, y: f32) -> Option<&str> {
        self.regions
            .iter()
            .rev()
            .find(|r| x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h)
            .map(|r| r.key.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// A path with a point count outside `limits::PATH_POINTS`.
    PathPoints { points: usize },
    /// A path with neither `fill` nor `stroke`.
    PathPaintsNothing,
    /// More than `limits::REGIONS_PER_FRAME` regions.
    TooManyRegions { regions: usize },
    /// Two different keys mapped to one region id.
    RegionIdCollision { id: String },
}

/// The region id for a draw-list key: the key itself when it is already in the contract's
/// grammar (screen buttons such as `retry`), otherwise `k-` and 24 hex digits of the key's
/// SHA3-256 — deterministic, so a node keeps its id (and the host its focus) across frames.
pub fn region_id(key: &str) -> String {
    if is_region_id(key) {
        return key.to_owned();
    }
    let digest = sw_crypto::sha3_256(key.as_bytes());
    let hex: String = digest[..12].iter().map(|b| format!("{b:02x}")).collect();
    format!("k-{hex}")
}

/// A label cut to the contract's character limit, the cut marked with an ellipsis.
pub fn region_label(label: &str) -> String {
    if label.chars().count() <= limits::REGION_LABEL_CHARS {
        return label.to_owned();
    }
    let mut out: String = label.chars().take(limits::REGION_LABEL_CHARS - 1).collect();
    out.push('\u{2026}');
    out
}

/// What both painters share: the commands so far and the first limit broken.
#[derive(Debug, Default)]
struct Collector {
    commands: Vec<DrawCommand>,
    regions: Vec<FrameRegion>,
    error: Option<FrameError>,
}

impl Collector {
    fn fail(&mut self, e: FrameError) {
        self.error.get_or_insert(e);
    }

    fn path(&mut self, points: &[(f32, f32)], fill: Option<u32>, stroke: Option<(f32, u32)>) {
        if !limits::PATH_POINTS.contains(&points.len()) {
            self.fail(FrameError::PathPoints {
                points: points.len(),
            });
        }
        if fill.is_none() && stroke.is_none() {
            self.fail(FrameError::PathPaintsNothing);
        }
    }

    /// Records the region; returns its command for a painter that can emit one.
    fn region(&mut self, op: &sw_graph::DrawOp) -> Option<DrawCommand> {
        let sw_graph::DrawOp::Region {
            id: key,
            x,
            y,
            w,
            h,
            label,
            role,
        } = op
        else {
            return None;
        };
        let id = region_id(key);
        if self.regions.iter().any(|r| r.id == id && r.key != *key) {
            self.fail(FrameError::RegionIdCollision { id: id.clone() });
        }
        self.regions.push(FrameRegion {
            id: id.clone(),
            key: key.clone(),
            x: *x,
            y: *y,
            w: *w,
            h: *h,
        });
        Some(DrawCommand::Region {
            id,
            x: *x,
            y: *y,
            w: *w,
            h: *h,
            label: region_label(label),
            role: (*role).into(),
        })
    }

    fn finish(&mut self) -> Result<Frame, FrameError> {
        let commands = std::mem::take(&mut self.commands);
        let regions = std::mem::take(&mut self.regions);
        if let Some(e) = self.error.take() {
            return Err(e);
        }
        if regions.len() > limits::REGIONS_PER_FRAME {
            return Err(FrameError::TooManyRegions {
                regions: regions.len(),
            });
        }
        Ok(Frame { commands, regions })
    }
}
