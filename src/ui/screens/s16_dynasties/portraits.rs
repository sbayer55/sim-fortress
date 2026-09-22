//! The 15 × 7 block-glyph portraits: fox, wolf and lynx by the roster's adult
//! glyph, with a generic predator and prey for any other roster.

use ratatui::buffer::Buffer;
use ratatui::style::Color;

use crate::sim::{Kind, Roster, SpeciesId};
use crate::ui::style::SpeciesStyle;
use crate::{glyphs, theme};

use super::{bold, put};

#[cfg(test)]
pub(super) const W: u16 = 15;
pub(super) const H: usize = 7;
pub(super) type Portrait = [&'static str; H];

const FOX: Portrait = [
    "  ▄▄▄     ▄▄▄  ",
    "  █ ▀█▄▄▄█▀ █  ",
    "  ▐▄ ■   ■ ▄▌  ",
    "   ▀█▄ ▀ ▄█▀   ",
    "     ▀█▄█▀     ",
    "      ▐█▌      ",
    "       ▀       ",
];
const WOLF: Portrait = [
    "   ▄▄     ▄▄   ",
    "   █▀█▄▄▄█▀█   ",
    "  ▐█ ■ ▄ ■ █▌  ",
    "   █▄▄▄▀▄▄▄█   ",
    "    ▀██▄▄██▀   ",
    "      ▀██▀     ",
    "       ▀▀      ",
];
const LYNX: Portrait = [
    "  ▲         ▲  ",
    "  █▀▄▄▄▄▄▄▄▀█  ",
    " ▐█ ■  ▄  ■ █▌ ",
    "  ▀▄▄ ▀▀▀ ▄▄▀  ",
    "   ▀▀█▄▄▄█▀▀   ",
    "     ▐███▌     ",
    "      ▀▀▀      ",
];
const PREDATOR: Portrait = [
    "   ▄▄▄▄▄▄▄▄▄   ",
    "  ▐█▀     ▀█▌  ",
    "  █  ■   ■  █  ",
    "  ▐▄   ▄   ▄▌  ",
    "   ▀█▄▄▄▄▄█▀   ",
    "     ▐███▌     ",
    "      ▀▀▀      ",
];
const PREY: Portrait = [
    "   ▄▄     ▄▄   ",
    "   ▐▌     ▐▌   ",
    "   ▄█▄▄▄▄▄█▄   ",
    "  █  ■   ■  █  ",
    "  ▐▄   ▄   ▄▌  ",
    "   ▀█▄▄▄▄▄█▀   ",
    "      ▀▀▀      ",
];

/// The portrait for a roster species: by its adult glyph letter when one is
/// drawn, else by kind.
pub(super) fn for_species(roster: &Roster, id: SpeciesId) -> &'static Portrait {
    match roster.adult_glyph(id) {
        'F' => &FOX,
        'W' => &WOLF,
        'L' => &LYNX,
        _ => match roster.kind(id) {
            Kind::Predator => &PREDATOR,
            Kind::Prey => &PREY,
        },
    }
}

/// Draw `p` with its top-left at `(x, y)` in `color`, the eyes in the key colour.
pub(super) fn draw(buf: &mut Buffer, x: u16, y: u16, p: &Portrait, color: Color) {
    for (j, row) in p.iter().enumerate() {
        let yy = y + crate::cast!(j => u16);
        for (i, ch) in row.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            let xx = x + crate::cast!(i => u16);
            let style = if ch == glyphs::SQUARE { bold(theme::KEY) } else { bold(color) };
            put(buf, xx, yy, 1, &ch.to_string(), style);
        }
    }
}

#[cfg(test)]
pub(super) const fn all() -> [&'static Portrait; 5] {
    [&FOX, &WOLF, &LYNX, &PREDATOR, &PREY]
}
