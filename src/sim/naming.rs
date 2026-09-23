//! Player-given names for animals and dynasties.
//!
//! A rename is a command from the UI, like pause or select: it writes
//! `Creature::nickname`, the mirrored `LineageNode::nickname` and
//! `Dynasty::name`, all display-only. No step reads them, no RNG is drawn and
//! the checksum does not hash them, so a named run stays tick-for-tick
//! identical to an unnamed one. Event text written after a rename uses the new
//! name; text already in the log keeps the old one.

use crate::sim::{CreatureId, Sim};

#[cfg(test)]
mod tests;

/// Longest animal name, in characters (the tables give names 8–12 cells).
pub const ANIMAL_NAME_MAX: usize = 12;
/// Longest dynasty name, in characters (the S16 banner and status bar).
pub const DYNASTY_NAME_MAX: usize = 20;

/// Normalise typed text into a name, or `None` (which clears it) when empty.
///
/// Printable ASCII only (every glyph must be CP437 and one cell wide), runs
/// of spaces collapsed, trimmed, cut to `max` characters.
pub fn clean(raw: &str, max: usize) -> Option<String> {
    let kept: String = raw.chars().map(|c| if c.is_ascii_graphic() { c } else { ' ' }).collect();
    let words: Vec<&str> = kept.split_whitespace().collect();
    let joined: String = words.join(" ").chars().take(max).collect();
    let name = joined.trim_end().to_string();
    (!name.is_empty()).then_some(name)
}

impl Sim {
    /// Name the animal `id` (living or still in the lineage), or clear its
    /// name when `raw` cleans to nothing. A founder's dynasty is re-labelled
    /// too. Returns false when the animal is known to neither store.
    pub fn rename_creature(&mut self, id: CreatureId, raw: &str) -> bool {
        let nickname = clean(raw, ANIMAL_NAME_MAX);
        let living = self.creatures.get_mut(id).map(|c| c.nickname.clone_from(&nickname)).is_some();
        let recorded = self.lineage.set_nickname(id, nickname);
        if !(living || recorded) {
            return false;
        }
        let roster = &self.params.species;
        let label = self
            .creatures
            .get(id)
            .map(|c| c.label(roster))
            .or_else(|| self.lineage.get(id).map(|n| format!("{} {}", n.name_str(roster), n.tag)));
        if let Some(label) = label {
            self.lineage.dynasties_mut().relabel_founder(id, label);
        }
        self.names_rev = self.names_rev.wrapping_add(1);
        true
    }

    /// Name the dynasty rooted at `root`, or clear its name when `raw` cleans
    /// to nothing. Returns false when no such line is kept.
    pub fn rename_dynasty(&mut self, root: CreatureId, raw: &str) -> bool {
        let renamed = self.lineage.dynasties_mut().rename(root, clean(raw, DYNASTY_NAME_MAX));
        if renamed {
            self.names_rev = self.names_rev.wrapping_add(1);
        }
        renamed
    }
}
