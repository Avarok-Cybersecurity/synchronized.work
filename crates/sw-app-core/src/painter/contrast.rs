//! WCAG 2.x contrast for `0xRRGGBBAA` colours (alpha is ignored: labels are opaque).

/// WCAG AA for body text: the ratio every label must reach against what it is drawn on.
pub const MIN_TEXT_CONTRAST: f32 = 4.5;
pub const BLACK: u32 = 0x0000_00FF;
pub const WHITE: u32 = 0xFFFF_FFFF;

fn channel(c: u32, shift: u32) -> f32 {
    let v = ((c >> shift) & 0xFF) as f32 / 255.0;
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Relative luminance, 0 (black) to 1 (white).
pub fn luminance(color: u32) -> f32 {
    0.2126 * channel(color, 24) + 0.7152 * channel(color, 16) + 0.0722 * channel(color, 8)
}

/// `(lighter + 0.05) / (darker + 0.05)`, 1 to 21.
pub fn ratio(a: u32, b: u32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `text` when it reaches [`MIN_TEXT_CONTRAST`] on `background`, otherwise whichever of black
/// and white contrasts more.
pub fn legible(text: u32, background: u32) -> u32 {
    if ratio(text, background) >= MIN_TEXT_CONTRAST {
        return text;
    }
    if ratio(BLACK, background) >= ratio(WHITE, background) {
        BLACK
    } else {
        WHITE
    }
}
