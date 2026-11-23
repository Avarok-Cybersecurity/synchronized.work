//! The tree fitted to the pane: one uniform view transform over `sw-graph`'s layout (no
//! second layout). `screen = offset + scale × layout`, with
//!
//! `scale = min(1, area.w / tree.w, (area.h − label_row(top_px)) / tree.h)`
//!
//! where `tree` is the extent of the node rectangles at rest, `area` the pane inside its margins
//! and above the footer, and `label_row(px) = px × (1 + label_descent_em)` the row a label takes
//! under the bottom nodes. The tree is centred in `area` on both axes (as the site's `fitView`
//! centres it). The transform is computed from the positions at rest, so it holds still while
//! the tween moves the nodes; it is applied to the tween's current positions.
//!
//! The scale is stepped: the largest power of [`FitParams::scale_ratio`] not above that
//! exact value. Everything a label's text depends on — its size and its node's width — comes
//! from the stepped scale, so a node added to the tree changes no other label (and costs no
//! other label's measures) unless it moves the fit past a step.
//!
//! Text is not scaled below [`FitParams::label_min_px`]; a label wider than its node's scaled
//! width is cut with an ellipsis (measured through the App's one width cache); labels are drawn
//! only while a label row fits between one level's nodes and the next — below that scale they
//! are hidden and the regions keep the full names. The App draws no secondary text for a node
//! (`sw-view`'s `detail` is not part of the draw list), so nothing else can collide.

use sw_graph::Pos;

use crate::ctx::Size;
use crate::painter::TextMeasure;

/// Marks a cut label (as `painter::region_label` does).
pub const ELLIPSIS: char = '\u{2026}';

/// Explicit (PCND): what fitting may not take away.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitParams {
    /// Labels are never drawn smaller than this many CSS px.
    pub label_min_px: f32,
    /// A label's depth below its baseline, in ems.
    pub label_descent_em: f32,
    /// Every node's hit region is at least this `(w, h)` in CSS px, grown about the node's
    /// centre and kept inside the pane.
    pub region_min: (f32, f32),
    /// One step of the fit scale, in `(0, 1)`.
    pub scale_ratio: f32,
    /// Steps taken at most; a tree that needs a smaller scale than `scale_ratio^scale_steps`
    /// is drawn at its exact scale.
    pub scale_steps: u32,
}

impl FitParams {
    /// The height of one label line at `px`.
    pub fn label_row(&self, px: f32) -> f32 {
        px * (1.0 + self.label_descent_em)
    }
}

/// Where the tree may be drawn, in CSS px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// The view transform and what it leaves of the labels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewFit {
    pub scale: f32,
    pub dx: f32,
    pub dy: f32,
    /// The size labels are drawn at.
    pub label_px: f32,
    /// Whether labels are drawn at all.
    pub labels: bool,
}

impl ViewFit {
    pub fn at(&self, p: Pos) -> Pos {
        Pos {
            x: self.dx + self.scale * p.x,
            y: self.dy + self.scale * p.y,
        }
    }
}

/// The transform for nodes at rest: each centre and `(w, h)` in layout units.
pub fn fit(boxes: &[(Pos, (f32, f32))], area: Area, label_size: f32, p: &FitParams) -> ViewFit {
    let mut ext = (
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
    );
    for (c, (w, h)) in boxes {
        ext.0 = ext.0.min(c.x - w / 2.0);
        ext.1 = ext.1.max(c.x + w / 2.0);
        ext.2 = ext.2.min(c.y - h / 2.0);
        ext.3 = ext.3.max(c.y + h / 2.0);
    }
    if boxes.is_empty() {
        ext = (0.0, 0.0, 0.0, 0.0);
    }
    let (tree_w, tree_h) = (ext.1 - ext.0, ext.3 - ext.2);
    let ratio = |avail: f32, len: f32| {
        if len > 0.0 { avail.max(0.0) / len } else { 1.0 }
    };
    let top_px = label_size.max(p.label_min_px);
    let scale = 1.0f32
        .min(ratio(area.w, tree_w))
        .min(ratio(area.h - p.label_row(top_px), tree_h));
    let scale = if scale.is_finite() {
        stepped(scale.max(0.0), p)
    } else {
        0.0
    };
    let label_px = (label_size * scale).max(p.label_min_px);
    let mut levels: Vec<f32> = boxes.iter().map(|(c, _)| c.y).collect();
    levels.sort_by(f32::total_cmp);
    levels.dedup();
    let pitch = levels
        .windows(2)
        .map(|l| l[1] - l[0])
        .fold(f32::INFINITY, f32::min);
    let tallest = boxes.iter().map(|(_, (_, h))| *h).fold(0.0, f32::max);
    let labels = pitch.is_infinite() || scale * (pitch - tallest) >= p.label_row(label_px);
    let content_h = scale * tree_h + if labels { p.label_row(label_px) } else { 0.0 };
    ViewFit {
        scale,
        dx: area.x + (area.w - scale * tree_w).max(0.0) / 2.0 - scale * ext.0,
        dy: area.y + (area.h - content_h).max(0.0) / 2.0 - scale * ext.2,
        label_px,
        labels,
    }
}

/// The largest `scale_ratio^k` (`k ≤ scale_steps`) not above `exact`; else `exact`.
fn stepped(exact: f32, p: &FitParams) -> f32 {
    let mut s = 1.0f32;
    for _ in 0..p.scale_steps {
        if s <= exact {
            return s;
        }
        s *= p.scale_ratio;
    }
    if s <= exact { s } else { exact }
}

/// `text` as drawn in `max` CSS px at `px`: whole when it fits, else its longest prefix that
/// fits with an ellipsis, else nothing. The cut is first guessed in proportion to the widths
/// and then moved a character at a time, so a label costs about three measures, not one per
/// character tried.
pub fn truncate(text: &str, max: f32, px: f32, measure: &dyn TextMeasure) -> Option<String> {
    let whole = measure.width(text, px);
    if whole <= max {
        return Some(text.to_owned());
    }
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let cut = |k: usize| {
        let mut s: String = chars[..k].iter().collect::<String>().trim_end().to_owned();
        s.push(ELLIPSIS);
        s
    };
    let fits = |k: usize| measure.width(&cut(k), px) <= max;
    if n < 2 {
        return None;
    }
    let guess = (n as f32 * (max / whole).clamp(0.0, 1.0)) as usize;
    let mut k = guess.clamp(1, n - 1);
    if fits(k) {
        while k + 1 < n && fits(k + 1) {
            k += 1;
        }
        return Some(cut(k));
    }
    while k > 1 {
        k -= 1;
        if fits(k) {
            return Some(cut(k));
        }
    }
    None
}

/// A node's hit rectangle: at least `min`, grown about its centre, moved inside the pane.
pub fn region((x, y, w, h): (f32, f32, f32, f32), min: (f32, f32), pane: Size) -> [f32; 4] {
    let gw = w.max(min.0).min(pane.width.max(0.0));
    let gh = h.max(min.1).min(pane.height.max(0.0));
    let gx = (x - (gw - w) / 2.0).min(pane.width - gw).max(0.0);
    let gy = (y - (gh - h) / 2.0).min(pane.height - gh).max(0.0);
    [gx, gy, gw, gh]
}
