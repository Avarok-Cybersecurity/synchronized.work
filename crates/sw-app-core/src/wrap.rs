//! Text wrapped to a width with the host's measure: the one wrap every screen uses. Breaks at
//! spaces; a word wider than the width alone is broken between characters, so nothing a node
//! or host reports can push a line past the pane.

use crate::painter::TextMeasure;

/// `text` as lines no wider than `max` at `px` (one line when it fits; none when empty).
pub fn wrap(text: &str, px: f32, max: f32, measure: &dyn TextMeasure) -> Vec<String> {
    let fits = |s: &str| measure.width(s, px) <= max;
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let joined = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if fits(&joined) {
            line = joined;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        if fits(word) {
            line = word.to_owned();
            continue;
        }
        for c in word.chars() {
            line.push(c);
            if !fits(&line) && line.chars().count() > 1 {
                line.pop();
                lines.push(std::mem::take(&mut line));
                line.push(c);
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
