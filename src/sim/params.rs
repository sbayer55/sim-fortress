//! All simulation tunables, serializable to/from TOML.
//!
//! `Params::from_toml` accepts a *partial* table: missing fields and sections
//! deep-merge over `Default`. Unknown keys are an error (deny_unknown_fields).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::sim::species::SpeciesId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
pub struct UiParams {
    pub speeds: Vec<u32>,
    pub base_ticks_per_second: f32,
    pub auto_pause_on_extinction: bool,
    pub log_births: bool,
    pub pause_on_follow_death: bool,
}

impl Default for UiParams {
    fn default() -> Self {
        UiParams {
            speeds: vec![1, 2, 5, 10, 25],
            base_ticks_per_second: 2.0,
            auto_pause_on_extinction: true,
            log_births: false,
            pause_on_follow_death: true,
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
    pub regrowth_rate: f32,
}

impl Default for EvolutionParams {
    fn default() -> Self {
        EvolutionParams {
            mutation_rate: 0.04,
            mutation_strength: 0.06,
            predation_difficulty: Difficulty::Normal,
            regrowth_rate: 1.0,
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
    pub creatures: CreaturesParams,
    pub evolution: EvolutionParams,
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
    }
}
