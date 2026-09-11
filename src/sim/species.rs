//! Species identity and the trait genome, as plain data (no presentation).

use serde::{Deserialize, Serialize};

/// Trait names, indexed by the `Genome` array order.
pub const TRAIT_NAMES: [&str; 8] = [
    "Speed", "Size", "Sense", "Metabolism", "Aggression", "Camouflage", "Fertility", "Longevity",
];

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
pub fn names(id: SpeciesId) -> &'static [&'static str] {
    match id.kind() {
        Kind::Prey => PREY_NAMES,
        Kind::Predator => PRED_NAMES,
    }
}

/// Resolve a `NameId` against a species' name list.
pub fn name_for(id: SpeciesId, name_id: u32) -> &'static str {
    let list = names(id);
    list[(name_id as usize) % list.len()]
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
    pub const ALL: [SpeciesId; 6] = [
        SpeciesId::Vole,
        SpeciesId::Hare,
        SpeciesId::Deer,
        SpeciesId::Fox,
        SpeciesId::Wolf,
        SpeciesId::Lynx,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SpeciesId::Vole => "Vole",
            SpeciesId::Hare => "Hare",
            SpeciesId::Deer => "Deer",
            SpeciesId::Fox => "Fox",
            SpeciesId::Wolf => "Wolf",
            SpeciesId::Lynx => "Lynx",
        }
    }

    /// Lowercase species letter (adult/uppercase is a presentation concern).
    pub fn glyph(self) -> char {
        match self {
            SpeciesId::Vole => 'v',
            SpeciesId::Hare => 'h',
            SpeciesId::Deer => 'd',
            SpeciesId::Fox => 'f',
            SpeciesId::Wolf => 'w',
            SpeciesId::Lynx => 'l',
        }
    }

    pub fn plural(self) -> &'static str {
        match self {
            SpeciesId::Vole => "Voles",
            SpeciesId::Hare => "Hares",
            SpeciesId::Deer => "Deer",
            SpeciesId::Fox => "Foxes",
            SpeciesId::Wolf => "Wolves",
            SpeciesId::Lynx => "Lynxes",
        }
    }

    pub fn kind(self) -> Kind {
        match self {
            SpeciesId::Vole | SpeciesId::Hare | SpeciesId::Deer => Kind::Prey,
            _ => Kind::Predator,
        }
    }

    pub fn diet(self) -> &'static str {
        match self {
            SpeciesId::Vole => "seeds, roots",
            SpeciesId::Hare => "grass, bark",
            SpeciesId::Deer => "grass, leaves",
            SpeciesId::Fox => "voles, hares",
            SpeciesId::Wolf => "deer, hares",
            SpeciesId::Lynx => "hares, voles",
        }
    }

    /// Baseline genome around which individuals vary.
    pub fn base_genome(self) -> Genome {
        // speed, size, sense, metabolism, aggression, camouflage, fertility, longevity
        match self {
            SpeciesId::Vole => Genome([0.45, 0.10, 0.40, 0.75, 0.05, 0.60, 0.90, 0.20]),
            SpeciesId::Hare => Genome([0.80, 0.25, 0.65, 0.60, 0.10, 0.55, 0.75, 0.35]),
            SpeciesId::Deer => Genome([0.65, 0.80, 0.55, 0.40, 0.20, 0.35, 0.35, 0.70]),
            SpeciesId::Fox => Genome([0.70, 0.35, 0.80, 0.55, 0.60, 0.50, 0.50, 0.45]),
            SpeciesId::Wolf => Genome([0.75, 0.70, 0.70, 0.50, 0.85, 0.25, 0.40, 0.60]),
            SpeciesId::Lynx => Genome([0.72, 0.50, 0.90, 0.45, 0.75, 0.70, 0.30, 0.55]),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Genome(pub [f32; 8]);

impl Genome {
    pub fn speed(&self) -> f32 {
        self.0[0]
    }
    pub fn size(&self) -> f32 {
        self.0[1]
    }
    pub fn sense(&self) -> f32 {
        self.0[2]
    }
    pub fn metabolism(&self) -> f32 {
        self.0[3]
    }
    pub fn aggression(&self) -> f32 {
        self.0[4]
    }
    pub fn camouflage(&self) -> f32 {
        self.0[5]
    }
    pub fn fertility(&self) -> f32 {
        self.0[6]
    }
    pub fn longevity(&self) -> f32 {
        self.0[7]
    }
    /// Sense range in map cells.
    pub fn sense_cells(&self) -> u16 {
        2 + (self.sense() * 10.0) as u16
    }
}
