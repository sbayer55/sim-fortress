//! The species roster (`[[species]]`): every per-species fact lives here, so a
//! new animal is a TOML block, not a code change.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use crate::sim::species::{Genome, Kind, SpeciesId, N_TRAITS, PRED_NAMES, PREY_NAMES};

/// Baseline genome as named fields, in `TRAIT_NAMES` order.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BaseGenome {
    pub speed: f32,
    pub size: f32,
    pub sense: f32,
    pub metabolism: f32,
    pub aggression: f32,
    pub camouflage: f32,
    pub fertility: f32,
    pub longevity: f32,
    pub resistance: f32,
    pub sociality: f32,
    pub maturity: f32,
    pub mutability: f32,
    pub diet_breadth: f32,
}

impl Default for BaseGenome {
    fn default() -> Self {
        Self::from_genome(Genome([0.5; N_TRAITS]))
    }
}

impl BaseGenome {
    /// The trait array in `Genome` order.
    pub const fn genome(&self) -> Genome {
        Genome([
            self.speed,
            self.size,
            self.sense,
            self.metabolism,
            self.aggression,
            self.camouflage,
            self.fertility,
            self.longevity,
            self.resistance,
            self.sociality,
            self.maturity,
            self.mutability,
            self.diet_breadth,
        ])
    }

    pub const fn from_genome(g: Genome) -> Self {
        Self {
            speed: g.0[0],
            size: g.0[1],
            sense: g.0[2],
            metabolism: g.0[3],
            aggression: g.0[4],
            camouflage: g.0[5],
            fertility: g.0[6],
            longevity: g.0[7],
            resistance: g.0[8],
            sociality: g.0[9],
            maturity: g.0[10],
            mutability: g.0[11],
            diet_breadth: g.0[12],
        }
    }
}

/// One species of the roster. Roster position is the `SpeciesId`, so the order
/// of `[[species]]` blocks is load-bearing for determinism (founders are placed
/// in roster order).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpeciesParams {
    /// Lowercase singular key (`"vole"`); referenced by `prey_preference` and pathogen hosts.
    pub name: String,
    /// Display plural (`"Voles"`).
    pub plural: String,
    /// Juvenile map glyph; adults use the uppercase form. Must be an ASCII letter.
    pub glyph: char,
    /// Map/legend colour as RGB.
    pub color: [u8; 3],
    pub kind: Kind,
    /// Display-only diet description.
    pub diet: String,
    /// Founding population; 0 = never introduced.
    pub initial_count: u32,
    pub adult_age_days: u32,
    pub gestation_days: u32,
    /// `litter = 1 + round(fertility × litter_max)`.
    pub litter_max: f32,
    pub mate_cooldown_days: u32,
    pub nocturnal: bool,
    /// Prey preference shares keyed by prey name; empty for prey. 0 = never targeted.
    pub prey_preference: BTreeMap<String, f32>,
    /// Individual name pool; empty = the built-in pool for `kind`.
    pub names: Vec<String>,
    pub base_genome: BaseGenome,
}

impl Default for SpeciesParams {
    fn default() -> Self {
        Self {
            name: String::new(),
            plural: String::new(),
            glyph: 'x',
            color: [200, 200, 200],
            kind: Kind::Prey,
            diet: String::new(),
            initial_count: 0,
            adult_age_days: 60,
            gestation_days: 10,
            litter_max: 1.0,
            mate_cooldown_days: 60,
            nocturnal: false,
            prey_preference: BTreeMap::new(),
            names: Vec::new(),
            base_genome: BaseGenome::default(),
        }
    }
}

impl SpeciesParams {
    /// Number of individual names available to this species.
    pub fn name_pool_len(&self) -> usize {
        if self.names.is_empty() {
            match self.kind {
                Kind::Prey => PREY_NAMES.len(),
                Kind::Predator => PRED_NAMES.len(),
            }
        } else {
            self.names.len()
        }
    }

    /// Resolve a `NameId` against this species' name pool.
    pub fn name_for(&self, name_id: u32) -> &str {
        let i = crate::cast!(name_id => usize);
        if self.names.is_empty() {
            let pool = match self.kind {
                Kind::Prey => PREY_NAMES,
                Kind::Predator => PRED_NAMES,
            };
            pool[i % pool.len()]
        } else {
            &self.names[i % self.names.len()]
        }
    }
}

/// The roster: exactly the `[[species]]` array.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Roster(pub Vec<SpeciesParams>);

impl Default for Roster {
    fn default() -> Self {
        Self(default_species())
    }
}

impl Roster {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every species id in roster order.
    pub fn ids(&self) -> impl Iterator<Item = SpeciesId> + '_ {
        (0..self.0.len()).map(SpeciesId::from_index)
    }

    pub fn get(&self, id: SpeciesId) -> &SpeciesParams {
        &self.0[id.index()]
    }

    pub fn get_mut(&mut self, id: SpeciesId) -> &mut SpeciesParams {
        &mut self.0[id.index()]
    }

    /// Look a species up by its lowercase name.
    pub fn id(&self, name: &str) -> Option<SpeciesId> {
        self.position(name).map(SpeciesId::from_index)
    }

    /// Roster position of a name.
    pub fn position(&self, name: &str) -> Option<usize> {
        self.0.iter().position(|s| s.name == name)
    }

    pub fn kind(&self, id: SpeciesId) -> Kind {
        self.get(id).kind
    }

    pub fn name(&self, id: SpeciesId) -> &str {
        &self.get(id).name
    }

    /// Capitalised singular for display (`"Vole"`).
    pub fn display_name(&self, id: SpeciesId) -> String {
        let mut chars = self.get(id).name.chars();
        match chars.next() {
            Some(c) => c.to_uppercase().chain(chars).collect(),
            None => String::new(),
        }
    }

    pub fn plural(&self, id: SpeciesId) -> &str {
        &self.get(id).plural
    }

    pub fn base_genome(&self, id: SpeciesId) -> Genome {
        self.get(id).base_genome.genome()
    }

    pub fn name_for(&self, id: SpeciesId, name_id: u32) -> &str {
        self.get(id).name_for(name_id)
    }

    /// Preference share of `prey` for `pred` (0 = never targeted; absent = 0).
    pub fn preference(&self, pred: SpeciesId, prey: SpeciesId) -> f32 {
        self.get(pred).prey_preference.get(self.name(prey)).copied().unwrap_or(0.0)
    }

    pub fn is_nocturnal(&self, id: SpeciesId) -> bool {
        self.get(id).nocturnal
    }

    pub fn prey_ids(&self) -> impl Iterator<Item = SpeciesId> + '_ {
        self.ids().filter(|&id| self.kind(id) == Kind::Prey)
    }

    pub fn predator_ids(&self) -> impl Iterator<Item = SpeciesId> + '_ {
        self.ids().filter(|&id| self.kind(id) == Kind::Predator)
    }

    /// Set a species' founding population by name; unknown names are ignored.
    pub fn set_initial_count(&mut self, name: &str, n: u32) {
        if let Some(pos) = self.position(name) {
            self.0[pos].initial_count = n;
        }
    }

    /// Founding population summed over one kind.
    pub fn initial_total(&self, kind: Kind) -> u32 {
        self.0.iter().filter(|s| s.kind == kind).map(|s| s.initial_count).sum()
    }

    /// No founders for any species (tests that want an empty world).
    pub fn clear_initial_counts(&mut self) {
        for s in &mut self.0 {
            s.initial_count = 0;
        }
    }
}

fn pref(vals: &[(&str, f32)]) -> BTreeMap<String, f32> {
    vals.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
}

const fn base(g: [f32; N_TRAITS]) -> BaseGenome {
    BaseGenome::from_genome(Genome(g))
}

/// The six built-in species with the balance table they have always had:
/// three prey, then three predators.
fn default_species() -> Vec<SpeciesParams> {
    let mut out = default_prey();
    out.extend(default_predators());
    out
}

fn default_prey() -> Vec<SpeciesParams> {
    vec![
        SpeciesParams {
            name: "vole".into(),
            plural: "Voles".into(),
            glyph: 'v',
            color: [188, 156, 116],
            kind: Kind::Prey,
            diet: "seeds, roots".into(),
            initial_count: 240,
            adult_age_days: 30,
            gestation_days: 3,
            litter_max: 0.0,
            mate_cooldown_days: 60,
            base_genome: base([0.45, 0.10, 0.40, 0.75, 0.05, 0.60, 0.90, 0.20, 0.30, 0.35, 0.5, 0.5, 0.35]),
            ..SpeciesParams::default()
        },
        SpeciesParams {
            name: "hare".into(),
            plural: "Hares".into(),
            glyph: 'h',
            color: [228, 220, 196],
            kind: Kind::Prey,
            diet: "grass, bark".into(),
            initial_count: 180,
            adult_age_days: 60,
            gestation_days: 6,
            litter_max: 1.0,
            mate_cooldown_days: 75,
            base_genome: base([0.80, 0.25, 0.65, 0.60, 0.10, 0.55, 0.75, 0.35, 0.35, 0.25, 0.5, 0.5, 0.50]),
            ..SpeciesParams::default()
        },
        SpeciesParams {
            name: "deer".into(),
            plural: "Deer".into(),
            glyph: 'd',
            color: [214, 160, 92],
            kind: Kind::Prey,
            diet: "grass, leaves".into(),
            initial_count: 90,
            adult_age_days: 180,
            gestation_days: 30,
            litter_max: 8.0,
            mate_cooldown_days: 30,
            base_genome: base([0.65, 0.80, 0.55, 0.40, 0.20, 0.35, 0.35, 0.70, 0.45, 0.70, 0.5, 0.5, 0.85]),
            ..SpeciesParams::default()
        },
    ]
}

fn default_predators() -> Vec<SpeciesParams> {
    vec![
        SpeciesParams {
            name: "fox".into(),
            plural: "Foxes".into(),
            glyph: 'f',
            color: [246, 128, 42],
            kind: Kind::Predator,
            diet: "voles, hares".into(),
            initial_count: 8,
            adult_age_days: 90,
            gestation_days: 20,
            litter_max: 3.0,
            mate_cooldown_days: 120,
            nocturnal: true,
            prey_preference: pref(&[("vole", 0.6), ("hare", 0.4), ("deer", 0.0)]),
            base_genome: base([0.70, 0.35, 0.80, 0.55, 0.60, 0.50, 0.50, 0.45, 0.40, 0.15, 0.5, 0.5, 0.5]),
            ..SpeciesParams::default()
        },
        SpeciesParams {
            name: "wolf".into(),
            plural: "Wolves".into(),
            glyph: 'w',
            color: [224, 66, 66],
            kind: Kind::Predator,
            diet: "deer, hares".into(),
            initial_count: 6,
            adult_age_days: 120,
            gestation_days: 30,
            litter_max: 2.0,
            mate_cooldown_days: 180,
            prey_preference: pref(&[("deer", 0.5), ("hare", 0.4), ("vole", 0.1)]),
            base_genome: base([0.75, 0.70, 0.70, 0.50, 0.85, 0.25, 0.40, 0.60, 0.50, 0.70, 0.5, 0.5, 0.5]),
            ..SpeciesParams::default()
        },
        SpeciesParams {
            name: "lynx".into(),
            plural: "Lynxes".into(),
            glyph: 'l',
            color: [236, 110, 150],
            kind: Kind::Predator,
            diet: "hares, voles".into(),
            initial_count: 4,
            adult_age_days: 120,
            gestation_days: 30,
            litter_max: 1.0,
            mate_cooldown_days: 180,
            nocturnal: true,
            prey_preference: pref(&[("hare", 0.6), ("vole", 0.4), ("deer", 0.0)]),
            base_genome: base([0.72, 0.50, 0.90, 0.45, 0.75, 0.70, 0.30, 0.55, 0.45, 0.10, 0.5, 0.5, 0.5]),
            ..SpeciesParams::default()
        },
    ]
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::sim::species::TRAIT_NAMES;

    #[test]
    fn base_genome_fields_follow_trait_order() {
        let v = toml::Value::try_from(BaseGenome::default()).unwrap();
        let keys: Vec<String> = match v {
            toml::Value::Table(t) => t.keys().cloned().collect(),
            _ => Vec::new(),
        };
        // BTreeMap sorts keys, so compare as sets against the trait names.
        let mut want: Vec<String> = TRAIT_NAMES.iter().map(|s| s.to_lowercase().replace(' ', "_")).collect();
        want.sort();
        assert_eq!(keys, want);
        let g = Genome([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.11, 0.12, 0.13, 0.14]);
        assert_eq!(BaseGenome::from_genome(g).genome(), g);
    }

    #[test]
    fn default_roster_lookups() {
        let r = Roster::default();
        assert_eq!(r.len(), 6);
        let fox = r.id("fox").unwrap();
        let vole = r.id("vole").unwrap();
        assert_eq!(r.preference(fox, vole), 0.6);
        assert_eq!(r.preference(vole, fox), 0.0);
        assert!(r.is_nocturnal(fox));
        assert_eq!(r.display_name(vole), "Vole");
        assert_eq!(r.prey_ids().count(), 3);
        assert_eq!(r.predator_ids().count(), 3);
        assert_eq!(r.name_for(vole, 0), "Clover");
        assert_eq!(r.name_for(fox, 1), "Ember");
        assert!(r.id("boar").is_none());
    }
}
