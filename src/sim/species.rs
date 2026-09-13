//! Species identity and the trait genome, as plain data (no presentation).

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

/// Prey name pool (shared by Vole, Hare and Deer; a `NameId` indexes into it).
pub const PREY_NAMES: &[&str] = &[
    "Clover", "Moss", "Fern", "Sorrel", "Rowan", "Willow", "Hazel", "Birch",
    "Tansy", "Yarrow", "Nettle", "Sedge", "Rush", "Burdock", "Mallow", "Vetch", "Cress", "Dill",
];

/// Predator name pool (shared by Fox, Wolf and Lynx; a `NameId` indexes into it).
pub const PRED_NAMES: &[&str] = &[
    "Greymaw", "Ember", "Sable", "Rook", "Cinder", "Fenrir", "Shade", "Talon", "Brindle",
    "Scorch", "Howl", "Umber", "Flint", "Gloam", "Rime", "Vex", "Snarl", "Dusk", "Kestrel",
];

/// The name list for a species (prey share one pool, predators another).
pub const fn names(id: SpeciesId) -> &'static [&'static str] {
    match id.kind() {
        Kind::Prey => PREY_NAMES,
        Kind::Predator => PRED_NAMES,
    }
}

/// Resolve a `NameId` against a species' name list.
pub fn name_for(id: SpeciesId, name_id: u32) -> &'static str {
    let list = names(id);
    list[(crate::cast!(name_id => usize)) % list.len()]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpeciesId {
    Vole,
    Hare,
    Deer,
    Fox,
    Wolf,
    Lynx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Prey,
    Predator,
}

impl SpeciesId {
    pub const ALL: [Self; 6] = [
        Self::Vole,
        Self::Hare,
        Self::Deer,
        Self::Fox,
        Self::Wolf,
        Self::Lynx,
    ];

    /// Position in `SpeciesId::ALL` (the index used by every per-species array).
    pub const fn index(self) -> usize {
        crate::cast!(self => usize)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Vole => "Vole",
            Self::Hare => "Hare",
            Self::Deer => "Deer",
            Self::Fox => "Fox",
            Self::Wolf => "Wolf",
            Self::Lynx => "Lynx",
        }
    }

    /// Lowercase species letter (adult/uppercase is a presentation concern).
    pub const fn glyph(self) -> char {
        match self {
            Self::Vole => 'v',
            Self::Hare => 'h',
            Self::Deer => 'd',
            Self::Fox => 'f',
            Self::Wolf => 'w',
            Self::Lynx => 'l',
        }
    }

    pub const fn plural(self) -> &'static str {
        match self {
            Self::Vole => "Voles",
            Self::Hare => "Hares",
            Self::Deer => "Deer",
            Self::Fox => "Foxes",
            Self::Wolf => "Wolves",
            Self::Lynx => "Lynxes",
        }
    }

    pub const fn kind(self) -> Kind {
        match self {
            Self::Vole | Self::Hare | Self::Deer => Kind::Prey,
            _ => Kind::Predator,
        }
    }

    pub const fn diet(self) -> &'static str {
        match self {
            Self::Vole => "seeds, roots",
            Self::Hare => "grass, bark",
            Self::Deer => "grass, leaves",
            Self::Fox => "voles, hares",
            Self::Wolf => "deer, hares",
            Self::Lynx => "hares, voles",
        }
    }

    /// Baseline genome around which individuals vary.
    ///
    /// Order: speed, size, sense, metabolism, aggression, camouflage, fertility,
    /// longevity, resistance, sociality, maturity. Maturity 0.5 everywhere keeps
    /// the starting balance identical to the pre-maturity numbers.
    pub const fn base_genome(self) -> Genome {
        match self {
            Self::Vole => Genome([0.45, 0.10, 0.40, 0.75, 0.05, 0.60, 0.90, 0.20, 0.30, 0.35, 0.5]),
            Self::Hare => Genome([0.80, 0.25, 0.65, 0.60, 0.10, 0.55, 0.75, 0.35, 0.35, 0.25, 0.5]),
            Self::Deer => Genome([0.65, 0.80, 0.55, 0.40, 0.20, 0.35, 0.35, 0.70, 0.45, 0.70, 0.5]),
            Self::Fox => Genome([0.70, 0.35, 0.80, 0.55, 0.60, 0.50, 0.50, 0.45, 0.40, 0.15, 0.5]),
            Self::Wolf => Genome([0.75, 0.70, 0.70, 0.50, 0.85, 0.25, 0.40, 0.60, 0.50, 0.70, 0.5]),
            Self::Lynx => Genome([0.72, 0.50, 0.90, 0.45, 0.75, 0.70, 0.30, 0.55, 0.45, 0.10, 0.5]),
        }
    }
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

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {

    use super::*;

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
        for id in SpeciesId::ALL {
            let g = id.base_genome();
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
