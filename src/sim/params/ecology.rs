//! Ecology tunables.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use crate::sim::time::Season;
use crate::sim::world::Terrain;
use super::world::Rainfall;

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
        use Terrain::{Sand, Dirt, GrassSparse, Grass, GrassDense, Forest};
        Self {
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
