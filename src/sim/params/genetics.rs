//! Genetics tunables.

use serde::{Deserialize, Serialize};
use crate::sim::time::Season;

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
    /// Mutation rate × `1 + (mutability − 0.5) × 2 × span` (the parents' mean
    /// Mutability), clamped to `0..=1`.
    pub mutability_rate_span: f32,
    /// Mutation strength × the same mutability factor.
    pub mutability_strength_span: f32,
    /// Mutability at or below which a newborn has no sterility risk.
    pub sterility_onset: f32,
    /// Sterility chance at the 0.98 trait cap; quadratic ramp from the onset.
    pub sterility_max: f32,
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
            mutability_rate_span: 0.8,
            mutability_strength_span: 0.6,
            sterility_onset: 0.75,
            sterility_max: 0.6,
            drift_every_generations: 2,
            lineage_keep_generations: 8,
            lineage_up: 3,
            lineage_rows_max: 400,
        }
    }
}

impl GeneticsParams {
    /// Litter size for a species' `litter_max` (from the roster), a fertility
    /// and a maturity: `1 + round(fertility × litter_max × maturity_factor)`,
    /// floored at one pup so a slow, small litter is never empty.
    pub fn litter_size(&self, litter_max: f32, fertility: f32, maturity: f32) -> u32 {
        let factor = Self::maturity_factor(maturity, self.maturity_litter_span);
        (1 + crate::cast!((fertility * litter_max * factor).round() => u32)).max(1)
    }

    /// The shared maturity multiplier `1 + (maturity − 0.5) × 2 × span`:
    /// 1.0 at maturity 0.5, so the trait is balance-neutral where it starts.
    pub fn maturity_factor(maturity: f32, span: f32) -> f32 {
        Self::trait_factor(maturity, span)
    }

    /// The neutral-at-0.5 multiplier every scaling trait shares:
    /// `1 + (v − 0.5) × 2 × span`, so a trait at 0.5 changes nothing.
    pub fn trait_factor(v: f32, span: f32) -> f32 {
        1.0 + (v - 0.5) * 2.0 * span
    }

    /// The `(rate, sd)` a birth mutates with, given the parents' mean
    /// Mutability: the global numbers scaled by `trait_factor` with the two
    /// mutability spans. Neutral at 0.5; the rate is clamped to a probability
    /// and the sd floored at zero.
    pub fn effective_mutation(&self, mutability: f32) -> (f32, f32) {
        let rate = (self.mutation_rate * Self::trait_factor(mutability, self.mutability_rate_span)).clamp(0.0, 1.0);
        let sd = (self.mutation_strength * Self::trait_factor(mutability, self.mutability_strength_span)).max(0.0);
        (rate, sd)
    }

    /// Chance a newborn with this Mutability is sterile: zero at or below
    /// `sterility_onset`, rising quadratically to `sterility_max` at the 0.98
    /// trait cap. The cost that stops evolvability ratcheting up for free.
    pub fn sterility_chance(&self, mutability: f32) -> f32 {
        let cap = crate::sim::species::TRAIT_MAX;
        let span = cap - self.sterility_onset;
        if span <= 0.0 {
            return if mutability >= cap { self.sterility_max } else { 0.0 };
        }
        let t = ((mutability - self.sterility_onset) / span).clamp(0.0, 1.0);
        self.sterility_max * t * t
    }
}
