//! All simulation tunables, serializable to/from TOML.
//!
//! `Params::from_toml` accepts a *partial* table: missing fields and sections
//! deep-merge over `Default`. Unknown keys are an error (deny_unknown_fields).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::sim::species::SpeciesId;
use crate::sim::time::Season;
use crate::sim::world::Terrain;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rainfall {
    Dry,
    Normal,
    Wet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldParams {
    pub width: usize,
    pub height: usize,
    pub water_pct: u8,
    pub forest_pct: u8,
    pub rock_pct: u8,
    pub rainfall: Rainfall,
}

impl Default for WorldParams {
    fn default() -> Self {
        WorldParams {
            width: 150,
            height: 40,
            water_pct: 20,
            forest_pct: 15,
            rock_pct: 5,
            rainfall: Rainfall::Normal,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TimeParams {
    pub season_days: u32,
    pub ticks_per_day: u32,
    pub start_hour: u32,
    pub sunrise_hour: u32,
    pub sunset_hour: u32,
}

impl Default for TimeParams {
    fn default() -> Self {
        TimeParams { season_days: 90, ticks_per_day: 24, start_hour: 6, sunrise_hour: 6, sunset_hour: 20 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScarcityThresholds {
    pub scarce: f32,
    pub strained: f32,
    pub plenty: f32,
    pub plenty_min_prey: u32,
    pub crowded_prey_per_veg: f32,
}

impl Default for ScarcityThresholds {
    fn default() -> Self {
        ScarcityThresholds {
            scarce: 0.365,
            strained: 0.40,
            plenty: 0.45,
            plenty_min_prey: 8,
            crowded_prey_per_veg: 30.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiParams {
    pub speeds: Vec<u32>,
    pub base_ticks_per_second: f32,
    pub auto_pause_on_extinction: bool,
    pub log_births: bool,
    pub pause_on_follow_death: bool,
    pub scarcity_thresholds: ScarcityThresholds,
}

impl Default for UiParams {
    fn default() -> Self {
        UiParams {
            speeds: vec![1, 2, 5, 10, 25],
            base_ticks_per_second: 2.0,
            auto_pause_on_extinction: true,
            log_births: false,
            pause_on_follow_death: true,
            scarcity_thresholds: ScarcityThresholds::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EventsParams {
    pub capacity: usize,
}

impl Default for EventsParams {
    fn default() -> Self {
        EventsParams { capacity: 5000 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StatsParams {
    pub series_days: usize,
}

impl Default for StatsParams {
    fn default() -> Self {
        StatsParams { series_days: 720 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CreaturesParams {
    pub initial_counts: BTreeMap<SpeciesId, u32>,
    pub adult_age_days: BTreeMap<SpeciesId, u32>,
    pub hunger_base: f32,
    pub hunger_per_size: f32,
    pub hunger_metabolism_k: f32,
    pub thirst_per_hour: f32,
    pub energy_awake_per_hour: f32,
    pub energy_rest_per_hour: f32,
    pub den_rest_bonus: f32,
    pub graze_per_hour: f32,
    pub graze_nutrition: f32,
    pub graze_min_vegetation: f32,
    pub drink_per_hour: f32,
    pub hp_loss_per_hour: f32,
    pub hp_regen_per_hour: f32,
    pub max_age_base: u32,
    pub max_age_per_longevity: u32,
    pub carcass_decay_days: u32,
    pub trail_len: usize,
    pub move_speed_base: f32,
    pub move_speed_per_trait: f32,
    pub move_cost_energy: f32,
    pub replan_ticks: u64,
    pub pressure_per_creature_tick: f32,
    pub pressure_decay_per_day: f32,
    pub den_create_chance_per_rest_hour: f32,
    pub max_dens_per_region: usize,
}

impl Default for CreaturesParams {
    fn default() -> Self {
        CreaturesParams {
            initial_counts: counts([240, 180, 90, 0, 0, 0]),
            adult_age_days: counts([30, 60, 180, 90, 120, 120]),
            hunger_base: 0.004,
            hunger_per_size: 0.008,
            hunger_metabolism_k: 1.0,
            thirst_per_hour: 0.012,
            energy_awake_per_hour: 0.008,
            energy_rest_per_hour: 0.05,
            den_rest_bonus: 1.5,
            graze_per_hour: 0.12,
            graze_nutrition: 1.5,
            graze_min_vegetation: 0.05,
            drink_per_hour: 0.35,
            hp_loss_per_hour: 0.02,
            hp_regen_per_hour: 0.01,
            max_age_base: 200,
            max_age_per_longevity: 900,
            carcass_decay_days: 6,
            trail_len: 12,
            move_speed_base: 0.5,
            move_speed_per_trait: 2.0,
            move_cost_energy: 0.002,
            replan_ticks: 6,
            pressure_per_creature_tick: 0.02,
            pressure_decay_per_day: 0.85,
            den_create_chance_per_rest_hour: 0.01,
            max_dens_per_region: 12,
        }
    }
}

/// Build the per-species map (same `SpeciesId::ALL` order as everywhere else).
fn counts(vals: [u32; 6]) -> BTreeMap<SpeciesId, u32> {
    SpeciesId::ALL.iter().copied().zip(vals).collect()
}

fn per_species(vals: [f32; 6]) -> BTreeMap<SpeciesId, f32> {
    SpeciesId::ALL.iter().copied().zip(vals).collect()
}

impl CreaturesParams {
    /// Hourly hunger accumulation, scaled by size, metabolism and season (FR1).
    pub fn hunger_per_hour(&self, size: f32, metabolism: f32, season_metabolism: f32) -> f32 {
        (self.hunger_base + self.hunger_per_size * size)
            * (0.5 + self.hunger_metabolism_k * metabolism)
            * season_metabolism
    }

    /// `adult = age_days >= adult_age_days[species]`.
    pub fn adult_age(&self, id: SpeciesId) -> u32 {
        self.adult_age_days.get(&id).copied().unwrap_or(0)
    }
}

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
    pub drift_every_generations: u32,
    pub lineage_keep_generations: u32,
    pub lineage_up: u32,
    pub lineage_rows_max: usize,
    /// Stored here; used by C5.
    pub predation_difficulty: Difficulty,
}

impl Default for GeneticsParams {
    fn default() -> Self {
        GeneticsParams {
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
            drift_every_generations: 2,
            lineage_keep_generations: 8,
            lineage_up: 3,
            lineage_rows_max: 400,
            predation_difficulty: Difficulty::Normal,
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
    /// Litter size for a given fertility: `1 + round(fertility × litter_max)`.
    pub fn litter_size(&self, id: SpeciesId, fertility: f32) -> u32 {
        1 + (fertility * self.litter_max(id)).round() as u32
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EcologyParams {
    /// User-facing multiplier (S09 "Regrowth rate" field maps here).
    pub regrowth_rate: f32,
    pub growth_k: f32,
    pub dieback_k: f32,
    pub evap_k: f32,
    pub rain_amount: f32,
    pub seed_sprout_chance_per_day: f32,
    pub drought_moisture: f32,
    pub drought_days: u32,
    pub drought_recover_margin: f32,
    pub water_dry_region_moisture: f32,
    pub water_refill_region_moisture: f32,
    pub water_changes_per_region_per_day: usize,
    pub max_vegetation: BTreeMap<Terrain, f32>,
    pub season_cap: BTreeMap<Season, f32>,
    pub season_regrowth: BTreeMap<Season, f32>,
    pub season_evaporation: BTreeMap<Season, f32>,
    pub season_metabolism: BTreeMap<Season, f32>,
    pub rain_chance_per_day: BTreeMap<Rainfall, f32>,
}

impl Default for EcologyParams {
    fn default() -> Self {
        use Terrain::*;
        EcologyParams {
            regrowth_rate: 1.0,
            // C4 balance lever (C2 shipped 0.08); see docs/chunks/c4-evolution.md FR1.
            growth_k: 0.18,
            dieback_k: 0.06,
            evap_k: 0.05,
            rain_amount: 0.15,
            seed_sprout_chance_per_day: 0.02,
            drought_moisture: 0.20,
            drought_days: 6,
            drought_recover_margin: 0.10,
            water_dry_region_moisture: 0.15,
            water_refill_region_moisture: 0.35,
            water_changes_per_region_per_day: 3,
            max_vegetation: BTreeMap::from([
                (Sand, 0.10),
                (Dirt, 0.30),
                (GrassSparse, 0.50),
                (Grass, 0.80),
                (GrassDense, 1.00),
                (Forest, 0.85),
            ]),
            season_cap: BTreeMap::from([
                (Season::Spring, 1.0),
                (Season::Summer, 1.0),
                (Season::Autumn, 0.7),
                (Season::Winter, 0.45),
            ]),
            season_regrowth: BTreeMap::from([
                (Season::Spring, 1.3),
                (Season::Summer, 1.0),
                (Season::Autumn, 0.7),
                (Season::Winter, 0.5),
            ]),
            season_evaporation: BTreeMap::from([
                (Season::Spring, 0.8),
                (Season::Summer, 1.2),
                (Season::Autumn, 0.9),
                (Season::Winter, 0.6),
            ]),
            season_metabolism: BTreeMap::from([
                (Season::Spring, 1.0),
                (Season::Summer, 1.0),
                (Season::Autumn, 1.1),
                (Season::Winter, 1.3),
            ]),
            rain_chance_per_day: BTreeMap::from([
                (Rainfall::Dry, 0.10),
                (Rainfall::Normal, 0.25),
                (Rainfall::Wet, 0.45),
            ]),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    pub world: WorldParams,
    pub time: TimeParams,
    pub ui: UiParams,
    pub events: EventsParams,
    pub stats: StatsParams,
    pub creatures: CreaturesParams,
    pub genetics: GeneticsParams,
    pub ecology: EcologyParams,
}

impl Params {
    /// Parse a (possibly partial) TOML table, deep-merged over defaults.
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_round_trip() {
        let p = Params::default();
        let text = p.to_toml().unwrap();
        let q = Params::from_toml(&text).unwrap();
        assert_eq!(p, q);
    }

    #[test]
    fn unknown_key_is_error() {
        let err = Params::from_toml("[world]\nrainfall = \"dry\"\nbogus = 1\n").unwrap_err();
        assert!(err.to_string().contains("bogus"), "error should name the key: {err}");
    }

    #[test]
    fn partial_table_deep_merges() {
        let p = Params::from_toml("[world]\nrainfall = \"dry\"\n").unwrap();
        assert_eq!(p.world.rainfall, Rainfall::Dry);
        // Everything else falls back to default.
        assert_eq!(p.world.width, 150);
        assert_eq!(p.time.season_days, 90);
        assert_eq!(p.events.capacity, 5000);
        assert_eq!(p.ecology.growth_k, 0.18);
    }
}
