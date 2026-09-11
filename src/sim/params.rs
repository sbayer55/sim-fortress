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
}

impl Default for CreaturesParams {
    fn default() -> Self {
        let mut counts = BTreeMap::new();
        for (id, n) in [
            (SpeciesId::Vole, 240u32),
            (SpeciesId::Hare, 180),
            (SpeciesId::Deer, 90),
            (SpeciesId::Fox, 30),
            (SpeciesId::Wolf, 24),
            (SpeciesId::Lynx, 12),
        ] {
            counts.insert(id, n);
        }
        CreaturesParams { initial_counts: counts }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EvolutionParams {
    pub mutation_rate: f32,
    pub mutation_strength: f32,
    pub predation_difficulty: Difficulty,
}

impl Default for EvolutionParams {
    fn default() -> Self {
        EvolutionParams {
            mutation_rate: 0.04,
            mutation_strength: 0.06,
            predation_difficulty: Difficulty::Normal,
        }
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
            growth_k: 0.08,
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
    pub evolution: EvolutionParams,
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
        assert_eq!(p.ecology.growth_k, 0.08);
    }
}
