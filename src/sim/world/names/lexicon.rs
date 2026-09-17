//! Syllable tables and the name templates. One style per world; a stem is
//! two or three syllables of onset + nucleus + coda, and after a coda the
//! next onset is a simple one so the seams stay pronounceable.

use std::collections::BTreeSet;

use crate::sim::rng::Rng;

use super::FeatureKind;

pub(super) const STYLES: usize = 3;
/// The first onsets of every table are empty or a single consonant: the
/// only ones drawn after a non-empty coda.
const SIMPLE_ONSETS: usize = 12;
pub(super) const STEM_MIN: usize = 3;
/// `the Xxxxxxxx Mountains` is exactly `NAME_FULL_MAX` at eight letters.
pub(super) const STEM_MAX: usize = 8;
/// Longest display name, including the article and the noun.
pub const NAME_FULL_MAX: usize = 22;
const STEM_TRIES: usize = 8;

pub(super) struct Style {
    pub(super) onsets: &'static [&'static str],
    pub(super) nuclei: &'static [&'static str],
    pub(super) codas: &'static [&'static str],
}

/// Hard northern, soft southern, and a western table with double letters.
pub(super) const STYLE: [Style; STYLES] = [
    Style {
        onsets: &["", "b", "d", "f", "g", "h", "k", "l", "m", "n", "r", "s", "t", "v", "th", "br", "gr", "sk", "st", "kr"],
        nuclei: &["a", "e", "i", "o", "u", "y", "ei", "au", "ae", "oa"],
        codas: &["", "", "l", "r", "n", "m", "d", "k", "s", "t", "ld", "nd", "rn", "st", "rd", "lm", "ng", "th", "sk", "g"],
    },
    Style {
        onsets: &["", "l", "m", "n", "r", "s", "v", "c", "d", "t", "f", "p", "y", "sl", "fl", "pr", "tr", "cl", "br", "gl"],
        nuclei: &["a", "e", "i", "o", "u", "ia", "ie", "eo", "ai", "ea"],
        codas: &["", "", "", "l", "n", "r", "s", "m", "ll", "rr", "nn", "th", "ss", "st", "nd", "rd", "ld", "x", "z", "lm"],
    },
    Style {
        onsets: &["", "b", "c", "d", "g", "h", "k", "m", "t", "w", "p", "r", "ch", "gw", "dr", "br", "cr", "gr", "tr", "rh"],
        nuclei: &["a", "e", "i", "o", "u", "y", "aw", "wy", "oe", "ae"],
        codas: &["", "", "n", "r", "l", "ch", "dd", "th", "g", "c", "ck", "rn", "ll", "nt", "rd", "lt", "gh", "s", "m", "w"],
    },
];

/// One capitalised stem, unused so far in this world.
pub(super) fn stem(rng: &mut Rng, style: usize, used: &mut BTreeSet<String>) -> String {
    let st = STYLE.get(style).unwrap_or(&STYLE[0]);
    let mut last = String::new();
    for _ in 0..STEM_TRIES {
        let s = draw(rng, st);
        if (STEM_MIN..=STEM_MAX).contains(&s.len()) && used.insert(s.clone()) {
            return s;
        }
        last = s;
    }
    // Every try clashed or missed the length: suffix a letter instead.
    let mut base: String = last.chars().take(STEM_MAX - 1).collect();
    if base.len() < STEM_MIN - 1 {
        base = "Ar".to_string();
    }
    for c in 'a'..='z' {
        let s = format!("{base}{c}");
        if used.insert(s.clone()) {
            return s;
        }
    }
    base
}

fn draw(rng: &mut Rng, st: &Style) -> String {
    let syllables = 2 + rng.below(2);
    let mut s = String::new();
    let mut open = true;
    for _ in 0..syllables {
        // After a coda, half the seams take no onset at all, and the rest a
        // simple one, so two consonants never pile onto a third.
        let onset = if open {
            st.onsets.get(rng.below(st.onsets.len())).copied().unwrap_or("")
        } else if rng.below(2) == 0 {
            ""
        } else {
            st.onsets.get(rng.below(SIMPLE_ONSETS.min(st.onsets.len()))).copied().unwrap_or("")
        };
        s.push_str(onset);
        s.push_str(st.nuclei.get(rng.below(st.nuclei.len())).copied().unwrap_or("a"));
        let coda = st.codas.get(rng.below(st.codas.len())).copied().unwrap_or("");
        s.push_str(coda);
        open = coda.is_empty();
    }
    capitalise(&s)
}

/// The full display name for a feature of `kind`; `noun` is a range's
/// "Hills" / "Mountains" / "Fells".
pub(super) fn full_name(rng: &mut Rng, style: usize, used: &mut BTreeSet<String>, kind: FeatureKind, noun: &str) -> String {
    let stem = stem(rng, style, used);
    match kind {
        FeatureKind::Ocean => format!("the {stem} Sea"),
        FeatureKind::Lake => format!("Lake {stem}"),
        FeatureKind::River => {
            if rng.below(2) == 0 {
                format!("River {stem}")
            } else {
                format!("the {stem}water")
            }
        }
        FeatureKind::Tributary => {
            if rng.below(2) == 0 {
                format!("{stem} Brook")
            } else {
                format!("{stem} Water")
            }
        }
        FeatureKind::Range => format!("the {stem} {noun}"),
    }
}

/// Upper-case the first letter.
pub(super) fn capitalise(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}
