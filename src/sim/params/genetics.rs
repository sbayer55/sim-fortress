//! Genetics tunables.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use crate::sim::species::SpeciesId;
use crate::sim::time::Season;
use super::creatures::{counts, per_species};

/// Reproduction, inheritance and lineage tunables (C4 FR1, the `[genetics]` table).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GeneticsParams {
    /// Per trait per birth.
    pub mutation_rate: f32,
    /// Gaussian sd of a mutation.
    pub mutation_strength: f32,
    /// `|Δ| ≥ this` emits a `Mutation` event.
    pub mutation_notable: f32,
    pub gestation_days: BTreeMap<SpeciesId, u32>,
    /// `litter = 1 + round(fertility × litter_max)`; fractional values give a
    /// graded litter (only high-fertility mothers reach the next pup).
    pub litter_max: BTreeMap<SpeciesId, f32>,
    pub mate_cooldown_days: BTreeMap<SpeciesId, u32>,
    pub mate_hunger_max: f32,
    pub mate_thirst_max: f32,
    pub mate_energy_min: f32,
    /// Density dependence for prey only: no mating on bare ground.
    pub mate_cell_vegetation_min: f32,
    pub breeding_seasons: Vec<Season>,
    pub follow_mother_days: u32,
    pub newborn_hp: f32,
    pub pregnancy_hunger_factor: f32,
    /// Safety: no new pregnancies above this; a Note is logged once per crossing.
    pub max_population_soft_cap: u32,
    /// Adult age × maturity factor: `1 + (maturity − 0.5) × 2 × span`.
    pub maturity_age_span: f32,
    /// Litter size × the same maturity factor.
    pub maturity_litter_span: f32,
    /// Maximum lifespan × the same maturity factor.
    pub maturity_lifespan_span: f32,
    pub drift_every_generations: u32,
    pub lineage_keep_generations: u32,
    pub lineage_up: u32,
    pub lineage_rows_max: usize,
}

impl Default for GeneticsParams {
    fn default() -> Self {
        Self {
            mutation_rate: 0.04,
            mutation_strength: 0.06,
            mutation_notable: 0.10,
            gestation_days: counts([3, 6, 30, 20, 30, 30]),
            // Balance table (C4 acceptance): see docs/chunks/c4-evolution.md FR1.
            litter_max: per_species([0.0, 1.0, 8.0, 3.0, 2.0, 1.0]),
            mate_cooldown_days: counts([60, 75, 30, 120, 180, 180]),
            mate_hunger_max: 0.45,
            mate_thirst_max: 0.5,
            mate_energy_min: 0.4,
            mate_cell_vegetation_min: 0.6,
            breeding_seasons: vec![Season::Spring, Season::Summer, Season::Autumn],
            follow_mother_days: 20,
            newborn_hp: 0.6,
            pregnancy_hunger_factor: 1.3,
            max_population_soft_cap: 4000,
            maturity_age_span: 0.5,
            maturity_litter_span: 0.5,
            maturity_lifespan_span: 0.25,
            drift_every_generations: 2,
            lineage_keep_generations: 8,
            lineage_up: 3,
            lineage_rows_max: 400,
        }
    }
}

impl GeneticsParams {
    pub fn gestation(&self, id: SpeciesId) -> u32 {
        self.gestation_days.get(&id).copied().unwrap_or(1)
    }
    pub fn litter_max(&self, id: SpeciesId) -> f32 {
        self.litter_max.get(&id).copied().unwrap_or(1.0)
    }
    pub fn cooldown(&self, id: SpeciesId) -> u32 {
        self.mate_cooldown_days.get(&id).copied().unwrap_or(1)
    }
    /// Litter size for a given fertility and maturity:
    /// `1 + round(fertility × litter_max × maturity_factor(maturity, litter_span))`,
    /// floored at one pup so a slow, small litter is never empty.
    pub fn litter_size(&self, id: SpeciesId, fertility: f32, maturity: f32) -> u32 {
        let factor = Self::maturity_factor(maturity, self.maturity_litter_span);
        (1 + crate::cast!((fertility * self.litter_max(id) * factor).round() => u32)).max(1)
    }

    /// The shared maturity multiplier `1 + (maturity − 0.5) × 2 × span`:
    /// 1.0 at maturity 0.5, so the trait is balance-neutral where it starts.
    pub fn maturity_factor(maturity: f32, span: f32) -> f32 {
        1.0 + (maturity - 0.5) * 2.0 * span
    }
}
