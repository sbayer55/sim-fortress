//! Named CP437 glyph constants. Every glyph here must exist in code page 437
//! and render one cell wide; `tests::all_glyphs_are_cp437` enforces both.
#![allow(dead_code)]

// ---- terrain
pub const DEEP_WATER: char = '≈';
pub const SHALLOW_WATER: char = '~';
pub const SAND: char = '·';
pub const DIRT: char = '.';
pub const GRASS_SPARSE: char = ',';
pub const GRASS: char = '"';
pub const GRASS_DENSE: char = '♣';
pub const FOREST: char = '♠';
pub const ROCK: char = '▲';
pub const HILL: char = '^';
pub const SNOW: char = '*';

// ---- shading ramp (used for heatmaps and bars)
pub const SHADE_0: char = ' ';
pub const SHADE_1: char = '░';
pub const SHADE_2: char = '▒';
pub const SHADE_3: char = '▓';
pub const SHADE_4: char = '█';
pub const SHADES: [char; 5] = [SHADE_0, SHADE_1, SHADE_2, SHADE_3, SHADE_4];
pub const HALF_UPPER: char = '▀';
pub const HALF_LOWER: char = '▄';
pub const FULL_BLOCK: char = '█';
pub const SQUARE: char = '■';

// ---- resources
pub const CARCASS: char = '%';
pub const DEN: char = 'Ω';
pub const SEED: char = '*';

// ---- creatures (lowercase = juvenile, uppercase = adult)
pub const VOLE: char = 'v';
pub const HARE: char = 'h';
pub const DEER: char = 'd';
pub const FOX: char = 'f';
pub const WOLF: char = 'w';
pub const LYNX: char = 'l';

// ---- cursor / selection / marks
pub const CURSOR: char = 'X';
pub const CORNER: char = '╬';
pub const TRAIL: char = '∙';
pub const RING: char = '°';
pub const DOT: char = '·';
pub const BULLET: char = '•';

// ---- bars
pub const BAR_L: char = '[';
pub const BAR_R: char = ']';
pub const BAR_FILL: char = '█';
pub const BAR_EMPTY: char = '░';

// ---- axes
pub const H_LINE: char = '─';
pub const V_LINE: char = '│';
pub const CROSS: char = '┼';

// ---- seasons / time
pub const SPRING: char = '♪';
pub const SUMMER: char = '☼';
pub const AUTUMN: char = '♫';
pub const WINTER: char = '*';
pub const SUN: char = '☼';
pub const MOON: char = '○';

// ---- events
pub const BIRTH: char = '♥';
pub const DEATH: char = 'x';
pub const MUTATION: char = '§';
pub const MIGRATION: char = '→';
pub const EXTINCTION: char = '‼';
pub const DROUGHT: char = '¡';
pub const ALERT: char = '!';
pub const NOTE: char = '¶';
pub const DISEASE: char = '☻';
pub const IMMUNE: char = '☺';
pub const PARASITE: char = '∩';

// ---- controls
pub const PLAY: char = '►';
pub const PAUSE_STR: &str = "││";
pub const FAST_STR: &str = "►►";
pub const STEP_STR: &str = "→│";
pub const REWIND: char = '◄';

// ---- misc
pub const HAPPY: char = '☺';
pub const UNHAPPY: char = '☻';
pub const MALE: char = '♂';
pub const FEMALE: char = '♀';
pub const UP: char = '↑';
pub const DOWN: char = '↓';
pub const FLAT: char = '↔';
pub const LEFT: char = '←';
pub const RIGHT: char = '→';
pub const HOUSE: char = '⌂';
pub const INFINITY: char = '∞';
pub const DIAMOND: char = '♦';
pub const PLUS_MINUS: char = '±';

/// Pick a shade glyph for `t` in 0..=1 (0 = empty, 1 = full block).
pub fn shade(t: f32) -> char {
    let i = crate::cast!((t.clamp(0.0, 1.0) * 4.0).round() => usize);
    SHADES[i.min(4)]
}

/// The full CP437 repertoire as Unicode (positions 0x01..=0xFE plus ASCII).
pub const CP437: &str = concat!(
    "☺☻♥♦♣♠•◘○◙♂♀♪♫☼►◄↕‼¶§▬↨↑↓→←∟↔▲▼",
    " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~⌂",
    "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀",
    "αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■",
);

pub fn is_cp437(c: char) -> bool {
    CP437.contains(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthChar;

    const ALL: &[char] = &[
        DEEP_WATER, SHALLOW_WATER, SAND, DIRT, GRASS_SPARSE, GRASS, GRASS_DENSE, FOREST, ROCK,
        HILL, SNOW, SHADE_1, SHADE_2, SHADE_3, SHADE_4, HALF_UPPER, HALF_LOWER, FULL_BLOCK,
        SQUARE, CARCASS, DEN, SEED, VOLE, HARE, DEER, FOX, WOLF, LYNX, CURSOR, CORNER, TRAIL,
        RING, DOT, BULLET, BAR_L, BAR_R, BAR_FILL, BAR_EMPTY, H_LINE, V_LINE, CROSS, SPRING,
        SUMMER, AUTUMN, WINTER, SUN, MOON, BIRTH, DEATH, MUTATION, MIGRATION, EXTINCTION,
        DROUGHT, ALERT, NOTE, PLAY, REWIND, HAPPY, UNHAPPY, MALE, FEMALE, UP, DOWN, FLAT, LEFT,
        RIGHT, HOUSE, INFINITY, DIAMOND, PLUS_MINUS,
    ];

    #[test]
    fn all_glyphs_are_cp437() {
        for &c in ALL {
            assert!(is_cp437(c), "{c:?} (U+{:04X}) is not in CP437", crate::cast!(c => u32));
            assert_eq!(c.width(), Some(1), "{c:?} is not one cell wide");
        }
        for s in [PAUSE_STR, FAST_STR, STEP_STR] {
            for c in s.chars() {
                assert!(is_cp437(c), "{c:?} in {s:?} is not in CP437");
            }
        }
    }

    #[test]
    fn cp437_table_is_complete() {
        // 31 control-picture glyphs + 95 printable ASCII + ⌂ + 127 high glyphs (0x80..=0xFE) = 254
        assert_eq!(CP437.chars().count(), 254);
    }
}
