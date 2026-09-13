//! All simulation tunables, serializable to/from TOML.
//!
//! `Params::from_toml` accepts a *partial* table: missing fields and sections
//! deep-merge over `Default`. Unknown keys are an error (`deny_unknown_fields`).

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
        Self {
            width: 150,
            height: 40,
            water_pct: 20,
            forest_pct: 15,
            rock_pct: 5,
            rainfall: Rainfall::Normal,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
        Self { season_days: 90, ticks_per_day: 24, start_hour: 6, sunrise_hour: 6, sunset_hour: 20 }
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
        Self {
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
    /// Autosave every N days (0 = off), C6 FR1/FR5.
    pub autosave_days: u32,
    /// Apply the blue night tint to the map (C6 FR5).
    pub day_night_tint: bool,
    /// Pause when a pathogen becomes epidemic (C7 FR9).
    pub auto_pause_on_epidemic: bool,
    pub scarcity_thresholds: ScarcityThresholds,
}

impl Default for UiParams {
    fn default() -> Self {
        Self {
            speeds: vec![1, 2, 5, 10, 25],
            base_ticks_per_second: 2.0,
            auto_pause_on_extinction: true,
            log_births: false,
            pause_on_follow_death: true,
            autosave_days: 0,
            day_night_tint: true,
            auto_pause_on_epidemic: true,
            scarcity_thresholds: ScarcityThresholds::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EventsParams {
    pub capacity: usize,
}

impl Default for EventsParams {
    fn default() -> Self {
        Self { capacity: 5000 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StatsParams {
    pub series_days: usize,
}

impl Default for StatsParams {
    fn default() -> Self {
        Self { series_days: 720 }
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
        Self {
            initial_counts: counts([240, 180, 90, 8, 6, 4]),
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
    /// Per-predator prey preference shares (preference 0 = never targeted).
    pub prey_preference: BTreeMap<SpeciesId, BTreeMap<SpeciesId, f32>>,
    pub nocturnal: Vec<SpeciesId>,
    pub flee_distance: f32,
    pub flee_ticks: u32,
    pub flee_energy_factor: f32,
    /// A resting prey detects predators at half its sense range.
    pub rest_detect_factor: f32,
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
        let prey = |vals: [(SpeciesId, f32); 3]| -> BTreeMap<SpeciesId, f32> { vals.into_iter().collect() };
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
            prey_preference: BTreeMap::from([
                (SpeciesId::Fox, prey([(SpeciesId::Vole, 0.6), (SpeciesId::Hare, 0.4), (SpeciesId::Deer, 0.0)])),
                (SpeciesId::Wolf, prey([(SpeciesId::Deer, 0.5), (SpeciesId::Hare, 0.4), (SpeciesId::Vole, 0.1)])),
                (SpeciesId::Lynx, prey([(SpeciesId::Hare, 0.6), (SpeciesId::Vole, 0.4), (SpeciesId::Deer, 0.0)])),
            ]),
            nocturnal: vec![SpeciesId::Fox, SpeciesId::Lynx],
            flee_distance: 8.0,
            flee_ticks: 10,
            flee_energy_factor: 2.0,
            rest_detect_factor: 0.5,
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

    /// Preference share of `prey` for `pred` (0 = never targeted; absent = 0).
    pub fn preference(&self, pred: SpeciesId, prey: SpeciesId) -> f32 {
        self.prey_preference.get(&pred).and_then(|m| m.get(&prey)).copied().unwrap_or(0.0)
    }

    /// Whether a species is nocturnal (fox and lynx).
    pub fn is_nocturnal(&self, id: SpeciesId) -> bool {
        self.nocturnal.contains(&id)
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

/// Herd and pack behaviour tunables (C8 FR1, the `[social]` table).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SocialParams {
    /// Preferred group size = `sociality × this`, for herds and packs alike.
    pub group_size_max: f32,
    /// Below this sociality a creature never herds: it wanders as before.
    pub cohesion_min: f32,
    /// Graze score ÷ `(1 + sociality × w × dist_to_kin_centroid / 8)` when herding.
    pub graze_cohesion_w: f32,
    /// A fleeing prey alerts same-species kin within `sociality × this` cells.
    pub alarm_cells: f32,
    /// Hunt score × `(1 + sociality × bonus × packmates_on_target(≤ 3))`.
    pub pack_join_bonus: f32,
    /// Kill chance `+= bonus × extra participants (≤ 3)`.
    pub pack_kill_bonus: f32,
    /// Hunger relief a non-killer participant gets, as a share of a full kill.
    pub pack_share: f32,
    /// A participant is a same-species hunter on that prey within this Chebyshev distance.
    pub pack_share_cheb: usize,
}

impl Default for SocialParams {
    fn default() -> Self {
        Self {
            group_size_max: 12.0,
            cohesion_min: 0.30,
            graze_cohesion_w: 1.0,
            alarm_cells: 6.0,
            pack_join_bonus: 1.5,
            pack_kill_bonus: 0.08,
            pack_share: 0.5,
            pack_share_cheb: 6,
        }
    }
}

impl SocialParams {
    /// Preferred group size for a sociality value.
    pub fn preferred_group(&self, sociality: f32) -> f32 {
        sociality * self.group_size_max
    }

    /// Whether a creature with this sociality and this many visible kin is
    /// herding (C8 FR2): social enough, not alone, and not over the dispersal
    /// threshold. One rule, shared by cohesion, the graze bias and the inspector.
    pub fn herding(&self, sociality: f32, kin_count: u8) -> bool {
        sociality >= self.cohesion_min && kin_count > 0 && f32::from(kin_count) <= 1.5 * self.preferred_group(sociality)
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

/// One contagious pathogen of the roster (C7 FR2). Runtime strains (`FR8b`) are
/// copies of these records with a single host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PathogenParams {
    pub name: String,
    /// Host multiplier on transmissibility and lethality; an absent species is immune.
    pub hosts: BTreeMap<SpeciesId, f32>,
    /// Infection chance per contact per tick.
    pub transmissibility: f32,
    pub incubation_days: u32,
    pub infectious_days: u32,
    pub lethality_per_day: f32,
    /// Days of immunity after recovery; 0 = lifelong.
    pub immunity_days: u32,
    /// Drives the speed, rest and kill-bonus effects.
    pub severity: f32,
    /// Contact multiplier when both animals are on a den cell.
    pub den_bonus: f32,
    /// `transmissibility += bonus × cell.moisture` of the contact's cell.
    pub moisture_bonus: f32,
}

impl Default for PathogenParams {
    fn default() -> Self {
        Self {
            name: String::new(),
            hosts: BTreeMap::new(),
            transmissibility: 0.01,
            incubation_days: 3,
            infectious_days: 10,
            lethality_per_day: 0.02,
            immunity_days: 360,
            severity: 0.5,
            den_bonus: 1.0,
            moisture_bonus: 0.0,
        }
    }
}

/// Disease and parasite tunables (C7 FR2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DiseaseParams {
    pub enabled: bool,
    /// Hunger rate × (1 + cost × resistance): the price of resistance.
    pub resist_hunger_cost: f32,
    /// Infection chance × (1 − w × resistance).
    pub susceptibility_w: f32,
    /// Daily death hazard × (1 − w × resistance).
    pub lethality_resist_w: f32,
    /// Infectious days × (1 − w × resistance), min 2.
    pub duration_resist_w: f32,
    /// Neighbours within this Chebyshev distance are contacts.
    pub contact_cheb: usize,
    /// Chance a newborn of an infectious mother starts incubating.
    pub vertical_transmission: f32,
    /// Chance of infection from eating a carcass that died infectious.
    pub carcass_transmission: f32,
    /// Move budget × (1 − penalty × severity) while infectious.
    pub sick_speed_penalty: f32,
    /// Hunger rate multiplier while infectious (fever).
    pub sick_hunger_factor: f32,
    /// Infectious creatures rest below this energy.
    pub sick_rest_energy: f32,
    pub sick_blocks_mating: bool,
    /// `kill_chance += bonus × severity` against infectious prey.
    pub kill_sick_bonus: f32,
    /// Active cases / living hosts at or above this is an epidemic.
    pub epidemic_share: f32,
    pub epidemic_min_cases: u32,
    /// Days after a pathogen's last case before it can re-emerge.
    pub reservoir_days: u32,
    /// Base daily emergence hazard at `emergence_host_ref` hosts.
    pub emergence_per_day: f32,
    pub emergence_host_ref: u32,
    /// No emergence below this many living hosts.
    pub emergence_host_min: u32,
    /// A Recovery event is emitted only for cases at least this severe.
    pub recovery_notable_min_severity: f32,
    /// Chance per infected meal that the pathogen mutates into the eater's species (`FR8b`).
    pub spillover_chance: f32,
    /// Strain parameters × N(1, jitter), clamped 0.25..2.
    pub spillover_jitter: f32,
    /// Susceptibility to a strain × (1 − this) for creatures immune to its parent.
    pub spillover_cross_immunity: f32,
    /// Roster plus live strains; at most 8.
    pub max_pathogens: usize,
    /// Founder spread of the Resistance trait (the other traits use 0.12): a
    /// wider standing variation is what an epidemic selects on.
    pub resistance_founder_sd: f32,
    // ---- parasites (one continuous load per creature)
    pub parasite_uptake: f32,
    pub parasite_shed: f32,
    pub parasite_cell_decay: f32,
    pub parasite_clearance: f32,
    pub parasite_carcass_transfer: f32,
    pub parasite_birth_transfer: f32,
    pub parasite_hunger_w: f32,
    pub parasite_fertility_w: f32,
    pub parasite_hp_threshold: f32,
    pub parasite_hp_loss: f32,
    /// Shedding/uptake multiplier on shallow-water cells.
    pub parasite_water_bonus: f32,
    /// Cell load added per carcass per day: carcasses are where parasites enter
    /// the world (nothing else seeds an empty field).
    pub parasite_carcass_seed: f32,
    /// Every founder and newborn carries at least this load: worms are endemic,
    /// and this is what lets crowded ground accumulate them.
    pub parasite_baseline: f32,
    /// Daily cell load growth × the cell's traffic (`prey_pressure + pred_pressure`):
    /// crowded ground fouls, quiet ground stays clean.
    pub parasite_ground_rate: f32,
    pub pathogens: Vec<PathogenParams>,
}

impl Default for DiseaseParams {
    fn default() -> Self {
        let hosts = |vals: &[(SpeciesId, f32)]| -> BTreeMap<SpeciesId, f32> { vals.iter().copied().collect() };
        Self {
            enabled: true,
            resist_hunger_cost: 0.10,
            susceptibility_w: 1.2,
            lethality_resist_w: 1.4,
            duration_resist_w: 0.4,
            contact_cheb: 1,
            vertical_transmission: 0.5,
            carcass_transmission: 0.3,
            sick_speed_penalty: 0.5,
            sick_hunger_factor: 1.3,
            sick_rest_energy: 0.45,
            sick_blocks_mating: true,
            kill_sick_bonus: 0.25,
            epidemic_share: 0.15,
            epidemic_min_cases: 20,
            reservoir_days: 120,
            emergence_per_day: 0.004,
            emergence_host_ref: 500,
            emergence_host_min: 60,
            recovery_notable_min_severity: 0.7,
            spillover_chance: 0.003,
            spillover_jitter: 0.25,
            spillover_cross_immunity: 0.5,
            max_pathogens: 8,
            resistance_founder_sd: 0.20,
            parasite_uptake: 0.04,
            parasite_shed: 0.002,
            parasite_cell_decay: 0.95,
            parasite_clearance: 0.03,
            parasite_carcass_transfer: 0.5,
            parasite_birth_transfer: 0.3,
            parasite_hunger_w: 0.4,
            parasite_fertility_w: 0.5,
            parasite_hp_threshold: 0.7,
            parasite_hp_loss: 0.005,
            parasite_water_bonus: 2.0,
            parasite_carcass_seed: 0.03,
            parasite_baseline: 0.05,
            parasite_ground_rate: 0.02,
            pathogens: vec![
                PathogenParams {
                    name: "Greyfever".into(),
                    hosts: hosts(&[(SpeciesId::Vole, 1.0), (SpeciesId::Hare, 1.0), (SpeciesId::Deer, 0.6)]),
                    transmissibility: 0.006,
                    incubation_days: 3,
                    infectious_days: 10,
                    lethality_per_day: 0.06,
                    immunity_days: 360,
                    severity: 0.8,
                    den_bonus: 1.0,
                    moisture_bonus: 0.0,
                },
                PathogenParams {
                    name: "Redmange".into(),
                    hosts: hosts(&[(SpeciesId::Fox, 1.0), (SpeciesId::Wolf, 0.8), (SpeciesId::Lynx, 0.6)]),
                    transmissibility: 0.008,
                    incubation_days: 7,
                    infectious_days: 40,
                    lethality_per_day: 0.01,
                    immunity_days: 0,
                    severity: 0.5,
                    den_bonus: 3.0,
                    moisture_bonus: 0.0,
                },
                PathogenParams {
                    name: "Hoofrot".into(),
                    hosts: hosts(&[(SpeciesId::Deer, 1.0), (SpeciesId::Hare, 0.3)]),
                    transmissibility: 0.002,
                    incubation_days: 5,
                    infectious_days: 20,
                    lethality_per_day: 0.02,
                    immunity_days: 180,
                    severity: 1.0,
                    den_bonus: 1.0,
                    moisture_bonus: 0.03,
                },
            ],
        }
    }
}

impl DiseaseParams {
    /// Roster size capped at `max_pathogens` (≤ 8, the width of `immune_until`).
    pub fn max_pathogens(&self) -> usize {
        self.max_pathogens.clamp(1, 8)
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
    pub predation: PredationParams,
    pub disease: DiseaseParams,
    pub social: SocialParams,
}

/// `(path, doc)` for every leaf parameter (C6 FR6).
const FIELD_DOCS: &[(&str, &str)] = &[
            // ---- world
            ("world.width", "World width in cells."),
            ("world.height", "World height in cells."),
            ("world.water_pct", "Target percentage of water cells (lakes + rivers)."),
            ("world.forest_pct", "Target percentage of forest cells."),
            ("world.rock_pct", "Target percentage of impassable rock cells."),
            ("world.rainfall", "Climate: dry | normal | wet."),
            // ---- time
            ("time.season_days", "Days per season (a year is four seasons)."),
            ("time.ticks_per_day", "Ticks per day (one tick is one simulated hour)."),
            ("time.start_hour", "Hour of day at tick 0."),
            ("time.sunrise_hour", "First daylight hour."),
            ("time.sunset_hour", "First night hour."),
            // ---- ui
            ("ui.speeds", "Speed multipliers offered by the UI."),
            ("ui.base_ticks_per_second", "Ticks per real second at x1 speed."),
            ("ui.auto_pause_on_extinction", "Pause when a species goes extinct."),
            ("ui.log_births", "Show births in the map ticker."),
            ("ui.pause_on_follow_death", "Pause when the followed creature dies."),
            ("ui.autosave_days", "Autosave every N days (0 = off)."),
            ("ui.day_night_tint", "Apply the blue night tint to the map."),
            ("ui.auto_pause_on_epidemic", "Pause when a pathogen becomes epidemic."),
            ("ui.scarcity_thresholds.scarce", "Vegetation below this fraction is Scarce."),
            ("ui.scarcity_thresholds.strained", "Vegetation below this fraction is Strained."),
            ("ui.scarcity_thresholds.plenty", "Vegetation above this fraction (with prey) is Plenty."),
            ("ui.scarcity_thresholds.plenty_min_prey", "Prey count a region needs to reach Plenty."),
            ("ui.scarcity_thresholds.crowded_prey_per_veg", "Prey-per-vegetation ratio that reads as Crowded."),
            // ---- events
            ("events.capacity", "Event ring buffer size."),
            // ---- stats
            ("stats.series_days", "Days of daily samples retained."),
            // ---- creatures
            ("creatures.initial_counts", "Founding population per species."),
            ("creatures.adult_age_days", "Days to adulthood per species."),
            ("creatures.hunger_base", "Base hourly hunger accumulation."),
            ("creatures.hunger_per_size", "Extra hunger per unit of body size."),
            ("creatures.hunger_metabolism_k", "Metabolism multiplier on hunger."),
            ("creatures.thirst_per_hour", "Hourly thirst accumulation."),
            ("creatures.energy_awake_per_hour", "Energy spent per waking hour."),
            ("creatures.energy_rest_per_hour", "Energy recovered per resting hour."),
            ("creatures.den_rest_bonus", "Rest multiplier inside a den."),
            ("creatures.graze_per_hour", "Vegetation eaten per grazing hour."),
            ("creatures.graze_nutrition", "Hunger relief per vegetation eaten."),
            ("creatures.graze_min_vegetation", "Minimum vegetation to graze."),
            ("creatures.drink_per_hour", "Thirst relief per drinking hour."),
            ("creatures.hp_loss_per_hour", "HP lost per hour while starving or thirsting."),
            ("creatures.hp_regen_per_hour", "HP regained per hour while comfortable."),
            ("creatures.max_age_base", "Base maximum lifespan in days."),
            ("creatures.max_age_per_longevity", "Extra lifespan days per longevity trait."),
            ("creatures.carcass_decay_days", "Days for a carcass to fully decay."),
            ("creatures.trail_len", "Remembered trail length."),
            ("creatures.move_speed_base", "Base movement speed."),
            ("creatures.move_speed_per_trait", "Extra speed per speed trait."),
            ("creatures.move_cost_energy", "Energy cost per move."),
            ("creatures.replan_ticks", "Ticks between goal replans."),
            ("creatures.pressure_per_creature_tick", "Prey pressure added per passing creature."),
            ("creatures.pressure_decay_per_day", "Daily prey-pressure decay."),
            ("creatures.den_create_chance_per_rest_hour", "Chance to dig a den per rest hour."),
            ("creatures.max_dens_per_region", "Maximum dens per region."),
            // ---- genetics
            ("genetics.mutation_rate", "Per-trait mutation chance per birth."),
            ("genetics.mutation_strength", "Standard deviation of a mutation."),
            ("genetics.mutation_notable", "|mutation| that emits a Mutation event."),
            ("genetics.gestation_days", "Pregnancy length per species."),
            ("genetics.litter_max", "Maximum litter per species."),
            ("genetics.mate_cooldown_days", "Days between pregnancies per species."),
            ("genetics.mate_hunger_max", "Hunger threshold to mate."),
            ("genetics.mate_thirst_max", "Thirst threshold to mate."),
            ("genetics.mate_energy_min", "Energy threshold to mate."),
            ("genetics.mate_cell_vegetation_min", "Vegetation required on the mating cell."),
            ("genetics.breeding_seasons", "Seasons in which mating is allowed."),
            ("genetics.follow_mother_days", "Days a juvenile follows its mother."),
            ("genetics.newborn_hp", "HP of a newborn."),
            ("genetics.pregnancy_hunger_factor", "Hunger multiplier while pregnant."),
            ("genetics.max_population_soft_cap", "Soft cap on total population."),
            ("genetics.maturity_age_span", "Adult age x (1 + (maturity - 0.5) x 2 x this)."),
            ("genetics.maturity_litter_span", "Litter size x (1 + (maturity - 0.5) x 2 x this)."),
            ("genetics.maturity_lifespan_span", "Max lifespan x (1 + (maturity - 0.5) x 2 x this)."),
            ("genetics.drift_every_generations", "Generations between drift samples."),
            ("genetics.lineage_keep_generations", "Generations kept in the lineage store."),
            ("genetics.lineage_up", "Generations up the S08 tree root."),
            ("genetics.lineage_rows_max", "Maximum S08 tree rows."),
            // ---- ecology
            ("ecology.regrowth_rate", "Vegetation regrowth multiplier."),
            ("ecology.growth_k", "Vegetation growth rate."),
            ("ecology.dieback_k", "Vegetation die-back rate."),
            ("ecology.evap_k", "Evaporation rate."),
            ("ecology.rain_amount", "Moisture added per rain event."),
            ("ecology.seed_sprout_chance_per_day", "Chance a seed sprouts per day."),
            ("ecology.drought_moisture", "Moisture below which a drought flags."),
            ("ecology.drought_days", "Days before a drought is declared."),
            ("ecology.drought_recover_margin", "Recovery margin above the drought line."),
            ("ecology.water_dry_region_moisture", "Moisture at which a water cell dries."),
            ("ecology.water_refill_region_moisture", "Moisture at which a dry cell refills."),
            ("ecology.water_changes_per_region_per_day", "Water cells changed per region per day."),
            ("ecology.max_vegetation", "Maximum vegetation per terrain."),
            ("ecology.season_cap", "Seasonal vegetation cap."),
            ("ecology.season_regrowth", "Seasonal regrowth multiplier."),
            ("ecology.season_evaporation", "Seasonal evaporation multiplier."),
            ("ecology.season_metabolism", "Seasonal metabolism multiplier."),
            ("ecology.rain_chance_per_day", "Rain chance per climate."),
            // ---- predation
            ("predation.detect_threshold", "Prey hidden when camouflage x cover >= sense x this."),
            ("predation.cover_by_terrain", "Cover bonus per terrain."),
            ("predation.den_protects", "Resting prey on a den cell cannot be targeted."),
            ("predation.chase_trigger_cheb", "Chebyshev distance that starts the chase clock."),
            ("predation.chase_max_ticks", "Maximum chase length."),
            ("predation.chase_speed_bonus", "Extra move budget while chasing."),
            ("predation.catch_distance_cheb", "Contact distance for a kill roll."),
            ("predation.kill_base", "Base kill chance."),
            ("predation.kill_speed_w", "Speed advantage weight in the kill chance."),
            ("predation.kill_aggression_w", "Aggression weight in the kill chance."),
            ("predation.kill_size_w", "Prey size penalty weight in the kill chance."),
            ("predation.kill_min", "Minimum kill chance."),
            ("predation.kill_max", "Maximum kill chance."),
            ("predation.eat_hours_base", "Base hours spent eating a kill."),
            ("predation.eat_hours_per_size", "Extra eating hours per prey size."),
            ("predation.hunger_per_kill_base", "Base hunger relief per kill."),
            ("predation.hunger_per_kill_per_size", "Extra hunger relief per prey size."),
            ("predation.kill_consumes_decay", "Decay added to a carcass per kill."),
            ("predation.hunt_cooldown_hours", "Hours between hunts."),
            ("predation.hunt_hunger_min", "Hunger threshold to start hunting."),
            ("predation.scavenge_hunger_min", "Hunger threshold to scavenge."),
            ("predation.carcass_nutrition", "Hunger relief per scavenge visit."),
            ("predation.scavenge_hours", "Hours spent scavenging."),
            ("predation.scavenge_consumes_decay", "Decay added per scavenge visit."),
            ("predation.difficulty", "Predation difficulty: easy | normal | hard."),
            ("predation.prey_preference", "Per-predator prey preference shares."),
            ("predation.nocturnal", "Species active at night."),
            ("predation.flee_distance", "Distance a prey flees."),
            ("predation.flee_ticks", "Ticks a prey flees."),
            ("predation.flee_energy_factor", "Energy cost multiplier while fleeing."),
            ("predation.rest_detect_factor", "Sense multiplier while resting."),
            ("predation.migrate_veg", "Vegetation-to-cap shortfall that triggers migration."),
            ("predation.migrate_days", "Days a migration trigger must persist."),
            ("predation.migrate_pressure", "Pressure that triggers migration."),
            ("predation.migrate_prey_min", "Prey per region below which prey migrate."),
            ("predation.migrate_cooldown_days", "Days before a pair can migrate again."),
            ("predation.local_extinction_min", "Population that defines a local line."),
            // ---- disease (C7)
            ("disease.enabled", "Master switch for pathogens and parasites."),
            ("disease.resist_hunger_cost", "Hunger rate x (1 + cost x resistance)."),
            ("disease.susceptibility_w", "Infection chance x (1 - w x resistance)."),
            ("disease.lethality_resist_w", "Daily death hazard x (1 - w x resistance)."),
            ("disease.duration_resist_w", "Infectious days x (1 - w x resistance), min 2."),
            ("disease.contact_cheb", "Chebyshev distance within which creatures are contacts."),
            ("disease.vertical_transmission", "Chance a newborn of an infectious mother is infected."),
            ("disease.carcass_transmission", "Chance of infection from eating a carcass that died infectious."),
            ("disease.sick_speed_penalty", "Move budget x (1 - penalty x severity) while infectious."),
            ("disease.sick_hunger_factor", "Hunger multiplier while infectious (fever)."),
            ("disease.sick_rest_energy", "Infectious creatures rest below this energy."),
            ("disease.sick_blocks_mating", "Infectious creatures do not mate."),
            ("disease.kill_sick_bonus", "Kill chance bonus x severity against infectious prey."),
            ("disease.epidemic_share", "Active cases / living hosts that counts as an epidemic."),
            ("disease.epidemic_min_cases", "Minimum active cases for an epidemic."),
            ("disease.reservoir_days", "Days after the last case before a pathogen can re-emerge."),
            ("disease.emergence_per_day", "Base daily emergence hazard at emergence_host_ref hosts."),
            ("disease.emergence_host_ref", "Host count at which the base emergence hazard applies."),
            ("disease.emergence_host_min", "No emergence below this many living hosts."),
            ("disease.recovery_notable_min_severity", "Recovery events only for cases at least this severe."),
            ("disease.spillover_chance", "Chance per infected meal that a pathogen jumps into the eater's species."),
            ("disease.spillover_jitter", "Strain parameters x N(1, jitter), clamped 0.25..2."),
            ("disease.spillover_cross_immunity", "Susceptibility to a strain x (1 - this) when immune to its parent."),
            ("disease.max_pathogens", "Roster plus live strains (at most 8)."),
            ("disease.resistance_founder_sd", "Founder spread of the Resistance trait (other traits: 0.12)."),
            ("disease.parasite_uptake", "Load gained per graze/drink tick x cell load x (1 - resistance)."),
            ("disease.parasite_shed", "Cell load gained per tick x creature load."),
            ("disease.parasite_cell_decay", "Daily multiplier on cell parasite load."),
            ("disease.parasite_clearance", "Daily load cleared x (0.5 + resistance)."),
            ("disease.parasite_carcass_transfer", "Load gained from eating a carcass x its load."),
            ("disease.parasite_birth_transfer", "Newborn load as a share of the mother's."),
            ("disease.parasite_hunger_w", "Hunger rate x (1 + w x load)."),
            ("disease.parasite_fertility_w", "Effective fertility x (1 - w x load)."),
            ("disease.parasite_hp_threshold", "Above this load hp drains every hour."),
            ("disease.parasite_hp_loss", "Hourly hp loss above the parasite threshold."),
            ("disease.parasite_water_bonus", "Shedding/uptake multiplier on shallow water."),
            ("disease.parasite_carcass_seed", "Cell parasite load added per carcass per day."),
            ("disease.parasite_baseline", "Minimum parasite load of founders and newborns."),
            ("disease.parasite_ground_rate", "Daily cell parasite growth x cell traffic (prey + predator pressure)."),
            ("disease.pathogens", "The pathogen roster: name, hosts, transmissibility, timings, lethality, immunity, severity, bonuses."),
            // ---- social (C8)
            ("social.group_size_max", "Preferred group size = sociality x this."),
            ("social.cohesion_min", "Below this sociality a creature never herds."),
            ("social.graze_cohesion_w", "Graze score divisor weight from distance to the kin centroid."),
            ("social.alarm_cells", "A fleeing prey alerts same-species kin within sociality x this cells."),
            ("social.pack_join_bonus", "Hunt score boost per packmate already on the target (max 3)."),
            ("social.pack_kill_bonus", "Kill chance added per extra participant (max 3)."),
            ("social.pack_share", "Hunger relief a non-killer participant gets, as a share of a kill."),
            ("social.pack_share_cheb", "Chebyshev distance within which a hunter counts as a participant."),
        ];

impl Params {
    /// Parse a (possibly partial) TOML table, deep-merged over defaults.
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    /// Deep-merge a partial TOML overlay onto `self` (C6 FR6). Unknown keys are
    /// still rejected by `deny_unknown_fields`; missing keys keep their value.
    pub fn apply_overlay(&mut self, s: &str) -> Result<(), String> {
        let base = toml::Value::try_from(self.clone()).map_err(|e| e.to_string())?;
        let overlay: toml::Value = toml::from_str(s).map_err(|e| e.to_string())?;
        let merged = deep_merge(base, overlay);
        *self = merged.try_into().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Dump the defaults as TOML with one `#` comment per leaf (C6 FR6).
    pub fn dump_toml() -> String {
        let default = Self::default();
        let text = toml::to_string_pretty(&default).unwrap_or_default();
        let mut doc: toml_edit::DocumentMut = match text.parse() {
            Ok(d) => d,
            Err(_) => return text,
        };
        for (path, comment) in Self::field_docs() {
            attach_comment(&mut doc, path, comment);
        }
        doc.to_string()
    }

    /// `(path, doc)` for every leaf parameter (C6 FR6). One entry per struct
    /// field that is not itself a nested `*Params` struct.
pub const fn field_docs() -> &'static [(&'static str, &'static str)] {
        FIELD_DOCS
    }
}

/// A named parameter preset (C6 FR7). `overlay` is a partial TOML table merged
/// over the current parameters; empty for Balanced (the defaults).
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub name: &'static str,
    pub description: &'static str,
    pub overlay: &'static str,
}

pub const PRESETS: [Preset; 6] = [
    Preset { name: "Balanced", description: "default values, gentle seasons", overlay: "" },
    Preset {
        name: "Harsh winter",
        description: "180-day seasons, regrowth 0.6",
        overlay: "time.season_days = 180\necology.regrowth_rate = 0.6\n",
    },
    Preset {
        name: "Lush",
        description: "forest 30%, regrowth 1.4, predation hard",
        overlay: "world.forest_pct = 30\necology.regrowth_rate = 1.4\npredation.difficulty = \"hard\"\n",
    },
    Preset { name: "Archipelago", description: "water 55%, islands isolate lineages", overlay: "world.water_pct = 55\n" },
    Preset {
        name: "Fast evolution",
        description: "mutation rate 0.10, strength 0.12",
        overlay: "genetics.mutation_rate = 0.10\ngenetics.mutation_strength = 0.12\n",
    },
    Preset {
        name: "Plague years",
        description: "outbreaks 3x as often, short reservoir, mutation 0.06",
        overlay: "disease.emergence_per_day = 0.012\ndisease.reservoir_days = 45\ngenetics.mutation_rate = 0.06\n",
    },
];

/// Recursively merge `overlay` onto `base`: tables deep-merge, anything else is
/// replaced by the overlay value.
fn deep_merge(base: toml::Value, overlay: toml::Value) -> toml::Value {
    match (base, overlay) {
        (toml::Value::Table(mut b), toml::Value::Table(o)) => {
            for (k, v) in o {
                match b.remove(&k) {
                    Some(existing) => b.insert(k, deep_merge(existing, v)),
                    None => b.insert(k, v),
                };
            }
            toml::Value::Table(b)
        }
        (_, overlay) => overlay,
    }
}

/// Attach a `# comment` before the leaf key at `path` in a `toml_edit` document.
fn attach_comment(doc: &mut toml_edit::DocumentMut, path: &str, comment: &str) {
    let parts: Vec<&str> = path.split('.').collect();
    let (tables, leaf) = parts.split_at(parts.len().saturating_sub(1));
    let Some(leaf) = leaf.first() else { return };
    let mut table = doc.as_table_mut();
    for part in tables {
        match table.get_mut(part).and_then(|i| i.as_table_mut()) {
            Some(t) => table = t,
            None => return,
        }
    }
    let decor = format!("# {comment}\n");
    if let Some(item) = table.get_mut(leaf) {
        if let Some(t) = item.as_table_mut() {
            t.decor_mut().set_prefix(decor);
            return;
        }
        // An array of tables (`[[disease.pathogens]]`): comment the first table,
        // never the key, or the comment ends up inside the header brackets.
        if let Some(arr) = item.as_array_of_tables_mut() {
            if let Some(first) = arr.iter_mut().next() {
                first.decor_mut().set_prefix(decor);
            }
            return;
        }
    }
    if let Some(mut key) = table.key_mut(leaf) {
        key.leaf_decor_mut().set_prefix(decor);
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
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

    #[test]
    fn unknown_key_errors() {
        let err = Params::from_toml("[world]\nrainfall = \"dry\"\nbogus = 1\n").unwrap_err();
        assert!(err.to_string().contains("bogus"), "error should name the key: {err}");
        let mut p = Params::default();
        let e = p.apply_overlay("[predation]\nkill_bogus = 1\n").unwrap_err();
        assert!(e.contains("kill_bogus"), "overlay error should name the key: {e}");
    }

    #[test]
    fn dump_params_round_trip() {
        let dump = Params::dump_toml();
        assert!(dump.contains("# "), "dump should carry comments");
        let parsed = Params::from_toml(&dump).unwrap();
        assert_eq!(parsed, Params::default());
    }

    #[test]
    fn preset_overlay_merge() {
        let mut p = Params::default();
        p.apply_overlay(PRESETS[1].overlay).unwrap(); // Harsh winter
        assert_eq!(p.time.season_days, 180);
        assert_eq!(p.ecology.regrowth_rate, 0.6);

        let mut p = Params::default();
        p.apply_overlay(PRESETS[2].overlay).unwrap(); // Lush
        assert_eq!(p.world.forest_pct, 30);
        assert_eq!(p.ecology.regrowth_rate, 1.4);
        assert_eq!(p.predation.difficulty, Difficulty::Hard);

        let mut p = Params::default();
        p.apply_overlay(PRESETS[3].overlay).unwrap(); // Archipelago
        assert_eq!(p.world.water_pct, 55);

        let mut p = Params::default();
        p.apply_overlay(PRESETS[4].overlay).unwrap(); // Fast evolution
        assert_eq!(p.genetics.mutation_rate, 0.10);
        assert_eq!(p.genetics.mutation_strength, 0.12);

        assert_eq!(PRESETS[0].overlay, "", "Balanced is the defaults");
    }

    #[test]
    fn maturity_factor_is_neutral_at_half() {
        let p = GeneticsParams::default();
        assert_eq!(GeneticsParams::maturity_factor(0.5, p.maturity_age_span), 1.0);
        assert_eq!(GeneticsParams::maturity_factor(0.5, p.maturity_lifespan_span), 1.0);
        // Slow (high maturity) means later, larger, longer; fast means the reverse.
        assert!(GeneticsParams::maturity_factor(0.98, p.maturity_age_span) > 1.0);
        assert!(GeneticsParams::maturity_factor(0.02, p.maturity_age_span) < 1.0);
        assert!(GeneticsParams::maturity_factor(0.98, p.maturity_litter_span) > 1.0);
        assert!(GeneticsParams::maturity_factor(0.98, p.maturity_lifespan_span) > 1.0);
        // Litter never drops below one, however fast the life history.
        assert_eq!(p.litter_size(SpeciesId::Deer, 0.0, 0.02), 1);
    }

    #[test]
    fn social_defaults_documented() {
        let s = SocialParams::default();
        assert!(s.group_size_max > 0.0 && s.cohesion_min > 0.0);
        assert!(s.pack_share > 0.0 && s.pack_share <= 1.0);
        assert!((1..=3).contains(&(s.pack_share_cheb.min(3))));
    }

    #[test]
    fn field_docs_complete() {
        let value = toml::Value::try_from(Params::default()).unwrap();
        let docs: BTreeMap<&str, &str> = Params::field_docs().iter().copied().collect();
        let map_fields: &[&str] = &[
            "creatures.initial_counts",
            "creatures.adult_age_days",
            "genetics.gestation_days",
            "genetics.litter_max",
            "genetics.mate_cooldown_days",
            "predation.cover_by_terrain",
            "predation.prey_preference",
            "ecology.max_vegetation",
            "ecology.season_cap",
            "ecology.season_regrowth",
            "ecology.season_evaporation",
            "ecology.season_metabolism",
            "ecology.rain_chance_per_day",
        ];
        let mut leaves: Vec<String> = Vec::new();
        collect_leaves(&value, "", map_fields, &mut leaves);
        for leaf in &leaves {
            assert!(docs.contains_key(leaf.as_str()), "missing doc for {leaf}");
        }
        for path in docs.keys() {
            assert!(leaves.iter().any(|l| l == path), "stale doc for {path}");
        }
    }

    fn collect_leaves(v: &toml::Value, prefix: &str, map_fields: &[&str], out: &mut Vec<String>) {
        let join = |p: &str, k: &str| if p.is_empty() { k.to_string() } else { format!("{p}.{k}") };
        match v {
            toml::Value::Table(t) => {
                if !prefix.is_empty() && map_fields.contains(&prefix) {
                    out.push(prefix.to_string());
                    return;
                }
                for (k, val) in t {
                    collect_leaves(val, &join(prefix, k), map_fields, out);
                }
            }
            _ => out.push(prefix.to_string()),
        }
    }
}
