//! Difficulty and predation tunables.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use crate::sim::world::Terrain;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
}

/// Predation, hunting, fleeing, scavenging, migration and extinction tunables
/// (C5 FR1, the `[predation]` table).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PredationParams {
    /// Prey is hidden when `camouflage × cover ≥ sense × detect_threshold`.
    pub detect_threshold: f32,
    /// Cover bonus per terrain for the hiding rule (water/rock are absent → 0).
    pub cover_by_terrain: BTreeMap<Terrain, f32>,
    /// A resting prey on a den cell cannot be targeted.
    pub den_protects: bool,
    /// Chase clock starts at `cheb ≤ this` (not at detection).
    pub chase_trigger_cheb: usize,
    pub chase_max_ticks: u32,
    /// Extra move budget per tick while chasing.
    pub chase_speed_bonus: f32,
    pub catch_distance_cheb: usize,
    pub kill_base: f32,
    pub kill_speed_w: f32,
    pub kill_aggression_w: f32,
    pub kill_size_w: f32,
    pub kill_min: f32,
    pub kill_max: f32,
    pub eat_hours_base: f32,
    pub eat_hours_per_size: f32,
    pub hunger_per_kill_base: f32,
    pub hunger_per_kill_per_size: f32,
    /// Eating advances the carcass decay by this much.
    pub kill_consumes_decay: f32,
    pub hunt_cooldown_hours: u32,
    pub hunt_hunger_min: f32,
    pub scavenge_hunger_min: f32,
    /// `hunger −= nutrition × (1 − decay)` once per scavenge visit; prey only.
    pub carcass_nutrition: f32,
    pub scavenge_hours: u32,
    pub scavenge_consumes_decay: f32,
    /// easy | normal | hard — stored here, given meaning in C6 (never mutates `kill_base`).
    pub difficulty: Difficulty,
    pub flee_distance: f32,
    pub flee_ticks: u32,
    pub flee_energy_factor: f32,
    /// A resting prey detects predators at half its sense range.
    pub rest_detect_factor: f32,
    /// C5 `FR5b`: a prey turns Wary of a detected predator that is *not* a danger
    /// within this distance. `0.0` disables the whole wary tier.
    pub wary_distance: f32,
    /// C5 `FR5b`: cells away from the predator for the wary waypoint.
    pub wary_step: f32,
    /// C5 `FR5b`: ticks the wary state is retained after the last detection.
    pub wary_ticks: u32,
    /// C5 `FR5b`: speed multiplier while wary (the low-exertion tier).
    pub wary_speed_factor: f32,
    /// C5 `FR5b`: wary ends only past `wary_distance × this` (hysteresis).
    pub wary_release_factor: f32,
    pub migrate_veg: f32,
    pub migrate_days: u32,
    pub migrate_pressure: f32,
    pub migrate_prey_min: u32,
    pub migrate_cooldown_days: u32,
    pub local_extinction_min: u32,
}

impl Default for PredationParams {
    fn default() -> Self {
        use Terrain::{Forest, GrassDense, Grass, GrassSparse, Dirt, Sand, ShallowWater};
        Self {
            detect_threshold: 0.8,
            cover_by_terrain: BTreeMap::from([
                (Forest, 1.0),
                (GrassDense, 1.0),
                (Grass, 0.8),
                (GrassSparse, 0.6),
                (Dirt, 0.6),
                (Sand, 0.4),
                (ShallowWater, 0.4),
            ]),
            den_protects: true,
            chase_trigger_cheb: 4,
            chase_max_ticks: 30,
            chase_speed_bonus: 0.5,
            catch_distance_cheb: 1,
            kill_base: 0.35,
            kill_speed_w: 1.0,
            kill_aggression_w: 0.3,
            kill_size_w: 0.2,
            kill_min: 0.05,
            kill_max: 0.95,
            eat_hours_base: 2.0,
            eat_hours_per_size: 4.0,
            hunger_per_kill_base: 4.0,
            hunger_per_kill_per_size: 0.4,
            kill_consumes_decay: 0.6,
            hunt_cooldown_hours: 6,
            hunt_hunger_min: 0.45,
            scavenge_hunger_min: 0.7,
            carcass_nutrition: 0.5,
            scavenge_hours: 1,
            scavenge_consumes_decay: 0.2,
            difficulty: Difficulty::Normal,
            flee_distance: 8.0,
            flee_ticks: 10,
            flee_energy_factor: 2.0,
            rest_detect_factor: 0.5,
            wary_distance: 3.0,
            wary_step: 3.0,
            wary_ticks: 2,
            wary_speed_factor: 0.5,
            wary_release_factor: 1.5,
            migrate_veg: 0.25,
            migrate_days: 6,
            migrate_pressure: 0.35,
            migrate_prey_min: 10,
            migrate_cooldown_days: 30,
            local_extinction_min: 5,
        }
    }
}

impl PredationParams {
    /// `kill_base + {easy +0.10, normal 0, hard −0.10}` (C6 FR7). The stored
    /// `kill_base`/`detect_threshold` are never mutated, so save → load cannot
    /// double-apply the difficulty modifier.
    pub fn effective_kill_base(&self) -> f32 {
        let d = match self.difficulty {
            Difficulty::Easy => 0.10,
            Difficulty::Normal => 0.0,
            Difficulty::Hard => -0.10,
        };
        self.kill_base + d
    }

    /// `detect_threshold + {easy −0.1, normal 0, hard +0.1}` (C6 FR7).
    pub fn effective_detect_threshold(&self) -> f32 {
        let d = match self.difficulty {
            Difficulty::Easy => -0.1,
            Difficulty::Normal => 0.0,
            Difficulty::Hard => 0.1,
        };
        self.detect_threshold + d
    }

    /// `clamp(kill_base + kill_speed_w × (pred.speed − prey.speed) + kill_aggression_w
    /// × pred.aggression − kill_size_w × prey.size, kill_min, kill_max)` (FR1).
    pub fn kill_chance(&self, pred_speed: f32, prey_speed: f32, pred_aggression: f32, prey_size: f32) -> f32 {
        (self.effective_kill_base() + self.kill_speed_w * (pred_speed - prey_speed) + self.kill_aggression_w * pred_aggression
            - self.kill_size_w * prey_size)
            .clamp(self.kill_min, self.kill_max)
    }

    /// `ceil(eat_hours_base + eat_hours_per_size × prey.size)` hours (ticks).
    pub fn eat_hours(&self, prey_size: f32) -> u32 {
        crate::cast!((self.eat_hours_base + self.eat_hours_per_size * prey_size).ceil() => u32)
    }

    /// `hunger_per_kill_base + hunger_per_kill_per_size × prey.size`.
    pub fn hunger_per_kill(&self, prey_size: f32) -> f32 {
        self.hunger_per_kill_base + self.hunger_per_kill_per_size * prey_size
    }
}
