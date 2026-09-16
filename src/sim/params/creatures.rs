//! Creature, predation, genetics, social and ecology tunables.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CreaturesParams {
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
        Self {
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

impl CreaturesParams {
    /// Hourly hunger accumulation, scaled by size, metabolism and season (FR1).
    pub fn hunger_per_hour(&self, size: f32, metabolism: f32, season_metabolism: f32) -> f32 {
        (self.hunger_base + self.hunger_per_size * size)
            * (0.5 + self.hunger_metabolism_k * metabolism)
            * season_metabolism
    }}
