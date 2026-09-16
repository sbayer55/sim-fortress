//! Species identity and the trait genome, as plain data (no presentation).
//!
//! Every per-species fact (name, glyph, colour, kind, base genome, life-history
//! numbers) lives in the `[[species]]` roster (`sim::params::Roster`); a
//! `SpeciesId` is only a position in it.

use serde::{Deserialize, Serialize};

/// Number of genome traits (C7 added Resistance; Sociality and Maturity are 9 and 10).
pub const N_TRAITS: usize = 11;

/// Trait indices that code outside this module refers to by name.
pub const IDX_RESISTANCE: usize = 8;
pub const IDX_SOCIALITY: usize = 9;
pub const IDX_MATURITY: usize = 10;

/// Trait names, indexed by the `Genome` array order.
pub const TRAIT_NAMES: [&str; N_TRAITS] = [
    "Speed", "Size", "Sense", "Metabolism", "Aggression", "Camouflage", "Fertility", "Longevity", "Resistance", "Sociality",
    "Maturity",
];

/// Three-letter trait abbreviations for the table headers, in `Genome` order.
/// Headers are built from this, never hand-typed.
pub const TRAIT_ABBR: [&str; N_TRAITS] = ["Spd", "Siz", "Sen", "Met", "Agg", "Cam", "Fer", "Lon", "Res", "Soc", "Mat"];

/// Built-in prey name pool, used by prey species with an empty `names` list.
pub const PREY_NAMES: &[&str] = &[
    "Clover", "Moss", "Fern", "Sorrel", "Rowan", "Willow", "Hazel", "Birch",
    "Tansy", "Yarrow", "Nettle", "Sedge", "Rush", "Burdock", "Mallow", "Vetch", "Cress", "Dill",
];

/// Built-in predator name pool, used by predator species with an empty `names` list.
pub const PRED_NAMES: &[&str] = &[
    "Greymaw", "Ember", "Sable", "Rook", "Cinder", "Fenrir", "Shade", "Talon", "Brindle",
    "Scorch", "Howl", "Umber", "Flint", "Gloam", "Rime", "Vex", "Snarl", "Dusk", "Kestrel",
];

/// A species: its position in the roster. Every per-species `Vec` in the
/// simulation is indexed by it, so roster order is load-bearing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpeciesId(pub u8);

impl SpeciesId {
    /// Position in the roster (the index used by every per-species `Vec`).
    pub const fn index(self) -> usize {
        crate::cast!(self.0 => usize)
    }

    /// The species at a roster position.
    pub const fn from_index(i: usize) -> Self {
        Self(crate::cast!(i => u8))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Prey,
    Predator,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Genome(pub [f32; N_TRAITS]);

impl Genome {
    /// Number of traits (kept as an alias of `N_TRAITS` for the many loops that
    /// already spell it this way).
    pub const LEN: usize = N_TRAITS;
    pub const fn speed(&self) -> f32 {
        self.0[0]
    }
    pub const fn size(&self) -> f32 {
        self.0[1]
    }
    pub const fn sense(&self) -> f32 {
        self.0[2]
    }
    pub const fn metabolism(&self) -> f32 {
        self.0[3]
    }
    pub const fn aggression(&self) -> f32 {
        self.0[4]
    }
    pub const fn camouflage(&self) -> f32 {
        self.0[5]
    }
    pub const fn fertility(&self) -> f32 {
        self.0[6]
    }
    pub const fn longevity(&self) -> f32 {
        self.0[7]
    }
    /// Disease resistance (C7): lowers susceptibility, lethality and duration; costs hunger.
    pub const fn resistance(&self) -> f32 {
        self.0[IDX_RESISTANCE]
    }
    /// Preferred group size (sociality): herds for prey, packs for predators.
    pub const fn sociality(&self) -> f32 {
        self.0[IDX_SOCIALITY]
    }
    /// Life-history pace (maturity): adult age, litter size and max lifespan all
    /// scale with it — low breeds early and small, high breeds late and large.
    pub const fn maturity(&self) -> f32 {
        self.0[IDX_MATURITY]
    }
    /// Sense range in map cells.
    pub fn sense_cells(&self) -> u16 {
        2 + crate::cast!((self.sense() * 10.0) => u16)
    }
    /// Trait values live in `0.02..=0.98` (founders and inheritance alike).
    pub const fn clamp_trait(v: f32) -> f32 {
        v.clamp(0.02, 0.98)
    }
}

/// The default roster's species by name, for tests that build creatures by hand.
#[cfg(test)]
pub mod testing {
    use super::{Genome, SpeciesId};
    use crate::sim::params::Roster;

    pub const VOLE: SpeciesId = SpeciesId(0);
    pub const HARE: SpeciesId = SpeciesId(1);
    pub const DEER: SpeciesId = SpeciesId(2);
    pub const FOX: SpeciesId = SpeciesId(3);
    pub const WOLF: SpeciesId = SpeciesId(4);
    pub const LYNX: SpeciesId = SpeciesId(5);
    /// Size of the default roster.
    pub const N_SPECIES: usize = 6;

    /// The default roster's base genome for `id`.
    pub fn genome(id: SpeciesId) -> Genome {
        roster().base_genome(id)
    }

    /// The default roster, shared by tests that build creatures by hand.
    pub fn roster() -> &'static Roster {
        static ROSTER: std::sync::OnceLock<Roster> = std::sync::OnceLock::new();
        ROSTER.get_or_init(Roster::default)
    }

    #[test]
    fn constants_match_default_roster() {
        let r = Roster::default();
        assert_eq!(r.len(), N_SPECIES);
        for (id, name) in [(VOLE, "vole"), (HARE, "hare"), (DEER, "deer"), (FOX, "fox"), (WOLF, "wolf"), (LYNX, "lynx")] {
            assert_eq!(r.name(id), name);
            assert_eq!(r.id(name), Some(id));
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {

    use super::*;
    use crate::sim::params::Roster;

    #[test]
    fn trait_tables_agree() {
        assert_eq!(TRAIT_NAMES.len(), N_TRAITS);
        assert_eq!(TRAIT_ABBR.len(), N_TRAITS);
        assert_eq!(Genome::LEN, N_TRAITS);
        // Abbreviations are exactly three cells: the S04 columns are sized on that.
        assert!(TRAIT_ABBR.iter().all(|a| a.len() == 3), "abbreviations must be 3 chars");
    }

    #[test]
    fn base_genomes_are_clamped_and_named() {
        let r = Roster::default();
        for id in r.ids() {
            let g = r.base_genome(id);
            for (t, &v) in g.0.iter().enumerate() {
                assert!((0.02..=0.98).contains(&v), "{id:?} trait {t} = {v} out of range");
            }
            // Accessors agree with the array slots.
            assert_eq!(g.resistance(), g.0[IDX_RESISTANCE]);
            assert_eq!(g.sociality(), g.0[IDX_SOCIALITY]);
            assert_eq!(g.maturity(), g.0[IDX_MATURITY]);
            // Maturity 0.5 everywhere is what keeps the starting balance unchanged.
            assert_eq!(g.maturity(), 0.5, "{id:?} must start at maturity 0.5");
        }
    }
}
