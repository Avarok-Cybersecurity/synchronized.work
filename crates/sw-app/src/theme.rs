//! The host's theme tokens (`AppContext.theme`: token name -> CSS colour) to the core's
//! [`AppTheme`]. Which token paints what is decided here, once; a token the host leaves out or
//! sends in a form this does not read takes its stated fallback (`config::Token::fallback`).

use std::collections::BTreeMap;

use sw_app_core::AppTheme;
use sw_graph::{Motion, Theme};

use crate::config::{self, Token};

/// An `hsl()` colour: hue in degrees, saturation and lightness in percent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsl {
    pub h: f32,
    pub s: f32,
    pub l: f32,
}

impl Hsl {
    pub const fn new(h: f32, s: f32, l: f32) -> Self {
        Self { h, s, l }
    }

    /// `0xRRGGBBAA`, `alpha` in 0..=1.
    pub fn rgba(self, alpha: f32) -> u32 {
        let (s, l) = (
            (self.s / 100.0).clamp(0.0, 1.0),
            (self.l / 100.0).clamp(0.0, 1.0),
        );
        let h = self.h.rem_euclid(360.0) / 30.0;
        let a = s * l.min(1.0 - l);
        let f = |n: f32| {
            let k = (n + h) % 12.0;
            l - a * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
        };
        pack(f(0.0), f(8.0), f(4.0), alpha)
    }
}

fn pack(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    (byte(r) << 24) | (byte(g) << 16) | (byte(b) << 8) | byte(a)
}

/// A number, or a percentage of `percent_of` when it ends in `%`.
fn number(text: &str, percent_of: f32) -> Option<f32> {
    let v = match text.strip_suffix('%') {
        Some(p) => p.trim().parse::<f32>().ok()? / 100.0 * percent_of,
        None => text.trim().parse::<f32>().ok()?,
    };
    v.is_finite().then_some(v)
}

fn hsl_function(args: &str) -> Option<u32> {
    let (color, alpha) = match args.split_once('/') {
        Some((c, a)) => (c, Some(a)),
        None => (args, None),
    };
    let parts: Vec<&str> = color
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    let (h, s, l, comma_alpha) = match parts.as_slice() {
        [h, s, l] => (h, s, l, None),
        [h, s, l, a] if alpha.is_none() => (h, s, l, Some(*a)),
        _ => return None,
    };
    if !s.ends_with('%') || !l.ends_with('%') {
        return None;
    }
    let alpha = match alpha.or(comma_alpha) {
        Some(a) => number(a, 1.0)?,
        None => 1.0,
    };
    let hue = number(h.trim_end_matches("deg"), 1.0)?;
    Some(Hsl::new(hue, number(s, 100.0)?, number(l, 100.0)?).rgba(alpha))
}

fn hex(digits: &str) -> Option<u32> {
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(digits, 16).ok()?;
    let short = |n: u32| (n & 0xF) * 0x11;
    match digits.len() {
        3 => Some((short(v >> 8) << 24) | (short(v >> 4) << 16) | (short(v) << 8) | 0xFF),
        6 => Some((v << 8) | 0xFF),
        8 => Some(v),
        _ => None,
    }
}

/// `hsl(...)` / `hsla(...)` (space or comma separated, optional alpha) or `#rgb`, `#rrggbb`,
/// `#rrggbbaa`, as `0xRRGGBBAA`; anything else is `None`.
pub fn parse_color(css: &str) -> Option<u32> {
    let css = css.trim();
    if let Some(digits) = css.strip_prefix('#') {
        return hex(digits);
    }
    let lower = css.to_ascii_lowercase();
    let args = lower
        .strip_prefix("hsla(")
        .or_else(|| lower.strip_prefix("hsl("))?
        .strip_suffix(')')?;
    hsl_function(args)
}

/// The colour of host token `token`, or its fallback.
pub fn token(tokens: &BTreeMap<String, String>, token: Token) -> u32 {
    tokens
        .get(token.name())
        .and_then(|v| parse_color(v))
        .unwrap_or_else(|| token.fallback().rgba(1.0))
}

/// The App's theme from the host's tokens.
pub fn theme_of(tokens: &BTreeMap<String, String>, motion: Motion) -> AppTheme {
    let t = |which: Token| token(tokens, which);
    AppTheme {
        graph: Theme {
            background: t(Token::Background),
            edge: t(Token::Border),
            edge_width: config::EDGE_WIDTH,
            project: t(Token::Primary),
            task: t(Token::MutedForeground),
            person: t(Token::Foreground),
            agent: config::BRAND_GREEN,
            progress: t(Token::MutedForeground),
            label: t(Token::Foreground),
            label_size: config::LABEL_PX,
        },
        view: config::VIEW,
        margin: config::MARGIN,
        text: t(Token::Foreground),
        muted: t(Token::MutedForeground),
        error: t(Token::Destructive),
        button: t(Token::Primary),
        button_text: t(Token::PrimaryForeground),
        field: t(Token::Muted),
        title_px: config::TITLE_PX,
        body_px: config::BODY_PX,
        line_height: config::LINE_HEIGHT,
        button_size: config::BUTTON_SIZE,
        motion,
    }
}
