//! Per-creature disease effects, immunity and susceptibility.

use crate::sim::creatures::{Creature, CreatureId};
use crate::sim::params::{DiseaseParams, PathogenParams};
use super::types::{DiseaseState, Infection, PathogenId, Stage};

/// The per-tick effects of infection, parasites and resistance (FR6): one
/// place computes them and the C3/C4/C5 systems read the fields.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effects {
    pub hunger_factor: f32,
    pub speed_factor: f32,
    pub rest_energy: f32,
    pub can_mate: bool,
    pub kill_bonus: f32,
    pub fertility_factor: f32,
}

/// The C3 rest threshold, kept here so the sick override has one baseline.
pub const REST_ENERGY: f32 = 0.25;

pub fn effects(c: &Creature, dp: &DiseaseParams) -> Effects {
    if !dp.enabled {
        return Effects { hunger_factor: 1.0, speed_factor: 1.0, rest_energy: REST_ENERGY, can_mate: true, kill_bonus: 0.0, fertility_factor: 1.0 };
    }
    let resistance = c.genome.resistance();
    let infectious = c.infection.filter(|i| i.stage == Stage::Infectious);
    let severity = infectious.map_or(0.0, |i| i.severity);
    let load = c.parasite_load;
    Effects {
        hunger_factor: (1.0 + dp.resist_hunger_cost * resistance)
            * if infectious.is_some() { dp.sick_hunger_factor } else { 1.0 }
            * (1.0 + dp.parasite_hunger_w * load),
        speed_factor: 1.0 - dp.sick_speed_penalty * severity,
        rest_energy: if infectious.is_some() { dp.sick_rest_energy } else { REST_ENERGY },
        can_mate: !(dp.sick_blocks_mating && infectious.is_some()),
        kill_bonus: dp.kill_sick_bonus * severity,
        fertility_factor: 1.0 - dp.parasite_fertility_w * load,
    }
}

/// `true` while the creature is in the infectious stage.
pub fn is_infectious(c: &Creature) -> bool {
    c.infection.is_some_and(|i| i.stage == Stage::Infectious)
}

/// `true` while the creature is immune to `p` (day-indexed table, FR3).
pub fn is_immune(c: &Creature, p: PathogenId, day: u32) -> bool {
    c.immune_until.get(crate::cast!(p.0 => usize)).is_some_and(|&until| day < until)
}

/// Susceptibility of `c` to pathogen `p`: host multiplier, resistance, and
/// cross-immunity from the parent of a strain. `0.0` = cannot be infected.
pub(super) fn susceptibility(c: &Creature, p: PathogenId, day: u32, dp: &DiseaseParams, state: &DiseaseState) -> f32 {
    if c.infection.is_some() || is_immune(c, p, day) {
        return 0.0;
    }
    let Some(path) = state.pathogen(p) else { return 0.0 };
    let host = path.host(c.species);
    if host <= 0.0 {
        return 0.0;
    }
    let mut s = host * (1.0 - dp.susceptibility_w * c.genome.resistance());
    if path.is_strain() {
        let root = state.root(p);
        let immune_to_kin = (0..crate::cast!(state.pathogens.len() => u8))
            .map(PathogenId)
            .filter(|&q| q != p && state.root(q) == root)
            .any(|q| is_immune(c, q, day));
        if immune_to_kin {
            s *= 1.0 - dp.spillover_cross_immunity;
        }
    }
    // Quirks (Hardy, Sickly, Plague-proof …); exactly 1 without them.
    (s * c.qm.susceptibility).max(0.0)
}

/// Build a fresh infection of `p` for `c` in the given stage.
pub(super) fn new_infection(c: &Creature, p: PathogenId, stage: Stage, day: u32, source: Option<CreatureId>, outbreak: u16, dp: &DiseaseParams, state: &DiseaseState) -> Infection {
    let Some(path) = state.pathogen(p) else {
        return Infection { pathogen: p, stage, since_day: day, ends_day: day, severity: 0.0, source, outbreak };
    };
    let r = c.genome.resistance();
    let ends_day = match stage {
        Stage::Incubating => day + path.params.incubation_days,
        Stage::Infectious => day + infectious_days(&path.params, r, dp),
    };
    Infection { pathogen: p, stage, since_day: day, ends_day, severity: path.params.severity * (1.0 - 0.5 * r), source, outbreak }
}

pub(super) fn infectious_days(p: &PathogenParams, resistance: f32, dp: &DiseaseParams) -> u32 {
    (crate::cast!((crate::cast!(p.infectious_days => f32) * (1.0 - dp.duration_resist_w * resistance)).round() => u32)).max(2)
}
