//! CP437 filter for model text drawn on screen (ai-requirements R8).

use crate::glyphs;

/// Map text onto the CP437 repertoire.
///
/// Common typographic characters become their ASCII cousins, newlines and tabs
/// survive, carriage returns vanish and anything else outside CP437 becomes `?`
/// rather than a wide cell.
pub fn to_cp437(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\n' | '\t' => out.push(c),
            '\r' => {}
            '\u{2018}' | '\u{2019}' | '\u{201a}' => out.push('\''),
            '\u{201c}' | '\u{201d}' | '\u{201e}' => out.push('"'),
            '\u{2013}' | '\u{2014}' | '\u{2212}' => out.push('-'),
            '\u{2026}' => out.push_str("..."),
            '\u{a0}' => out.push(' '),
            c if glyphs::is_cp437(c) => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}
