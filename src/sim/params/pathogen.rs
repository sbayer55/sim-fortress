//! Pathogen and disease tunables.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use crate::sim::species::SpeciesId;

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
