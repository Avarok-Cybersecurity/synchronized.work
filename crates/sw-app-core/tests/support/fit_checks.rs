//! What a drawn tree shows, read back from its commands: each region's rectangle and name, and
//! each tree label's rectangle (its measured width, from `px` above the baseline to its
//! descent below), so a test can check the tree against the pane it was fitted to.

use sw_app_core::{DrawCommand, Size, TextMeasure};

/// `[x, y, w, h]`.
pub type Rect = [f32; 4];

#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub regions: Vec<(String, Rect)>,
    /// Text, rectangle, px.
    pub labels: Vec<(String, Rect, f32)>,
}

/// The tree's regions and labels; the last `footer_lines` texts are the footer's, not labels.
pub fn shown(
    c: &[DrawCommand],
    footer_lines: usize,
    descent_em: f32,
    m: &dyn TextMeasure,
) -> Shown {
    let regions = c
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Region {
                label, x, y, w, h, ..
            } => Some((label.clone(), [*x, *y, *w, *h])),
            _ => None,
        })
        .collect();
    let mut texts: Vec<(String, Rect, f32)> = c
        .iter()
        .filter_map(|c| match c {
            DrawCommand::FillText { x, y, text, px, .. } => {
                let w = m.width(text, *px);
                Some((text.clone(), [*x, y - px, w, px * (1.0 + descent_em)], *px))
            }
            _ => None,
        })
        .collect();
    texts.truncate(texts.len().saturating_sub(footer_lines));
    Shown {
        regions,
        labels: texts,
    }
}

/// Whether two rectangles share more than an edge.
pub fn overlap(a: Rect, b: Rect) -> bool {
    const EDGE: f32 = 0.01;
    a[0] + a[2] > b[0] + EDGE
        && b[0] + b[2] > a[0] + EDGE
        && a[1] + a[3] > b[1] + EDGE
        && b[1] + b[3] > a[1] + EDGE
}

pub fn inside(r: Rect, pane: Size) -> bool {
    const EDGE: f32 = 0.01;
    r[0] >= -EDGE
        && r[1] >= -EDGE
        && r[0] + r[2] <= pane.width + EDGE
        && r[1] + r[3] <= pane.height + EDGE
}

/// Every check of a fitted tree; the first one broken, described.
pub fn check(
    s: &Shown,
    pane: Size,
    region_min: (f32, f32),
    label_min_px: f32,
) -> Result<(), String> {
    for (name, r) in &s.regions {
        if !inside(*r, pane) {
            return Err(format!(
                "region {name:?} {r:?} is outside the {pane:?} pane"
            ));
        }
        if r[2] < region_min.0 - 0.01 || r[3] < region_min.1 - 0.01 {
            return Err(format!(
                "region {name:?} {r:?} is smaller than {region_min:?}"
            ));
        }
    }
    for (i, (a, ra, px)) in s.labels.iter().enumerate() {
        if *px < label_min_px {
            return Err(format!("label {a:?} at {px} px, under {label_min_px}"));
        }
        if !inside(*ra, pane) {
            return Err(format!("label {a:?} {ra:?} is outside the pane"));
        }
        for (b, rb, _) in &s.labels[i + 1..] {
            if overlap(*ra, *rb) {
                return Err(format!("labels {a:?} {ra:?} and {b:?} {rb:?} overlap"));
            }
        }
    }
    Ok(())
}
