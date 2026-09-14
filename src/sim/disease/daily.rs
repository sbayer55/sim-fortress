//! Daily disease progression, carcass decay and resistance means.

use crate::sim::creatures::{CreatureStore, DeathTallies};
use crate::sim::events::EventRing;
use crate::sim::lineage::Lineage;
use crate::sim::params::DiseaseParams;
use crate::sim::rng::Rng;
use crate::sim::time::Time;
use crate::sim::world::World;
use crate::sim::Alert;
use super::types::{DiseaseState, Stage};
use super::lifecycle::{become_infectious, clear_parasites, disease_death, recover};
use super::outbreaks::{announce_ended, announce_epidemics, tally_stats, track_outbreaks};
use super::emergence::seed_emergence;

/// FR5: stage transitions, lethality rolls, recovery, parasite clearance.
/// Runs in `day_boundary` before age death.
#[allow(clippy::too_many_arguments)]
pub fn progress_daily(
    store: &mut CreatureStore,
    world: &mut World,
    events: &mut EventRing,
    time: &Time,
    dp: &DiseaseParams,
    state: &mut DiseaseState,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
    rng: &mut Rng,
) {
    if !dp.enabled {
        return;
    }
    let day = crate::cast!(time.day_index() => u32);
    for o in &mut state.outbreaks {
        o.cases_today = 0;
    }
    let ids = store.living_ids();
    for id in ids {
        let Some(c) = store.get(id) else { continue };
        let Some(inf) = c.infection else {
            continue;
        };
        let Some(path) = state.pathogen(inf.pathogen).cloned() else {
            if let Some(c) = store.get_mut(id) {
                c.infection = None;
            }
            continue;
        };
        let host = path.host(c.species).max(0.0);
        let r = c.genome.resistance();
        match inf.stage {
            Stage::Incubating if day >= inf.ends_day => {
                become_infectious(store, state, id, day, inf, &path, dp);
            }
            Stage::Incubating => {}
            Stage::Infectious => {
                let hazard = (path.params.lethality_per_day * host * (1.0 - dp.lethality_resist_w * r)).clamp(0.0, 1.0);
                if rng.chance(hazard) {
                    disease_death(store, world, events, time, tallies, lineage, state, id, day, inf, &path);
                } else if day >= inf.ends_day {
                    recover(store, events, time, dp, state, id, day, inf, &path);
                }
            }
        }
    }
    clear_parasites(store, dp);
}

/// FR7: daily decay of the cell parasite field (called with the pressure decay).
pub fn decay_cells(world: &mut World, dp: &DiseaseParams) {
    if !dp.enabled {
        return;
    }
    for cell in &mut world.cells {
        if cell.parasite_load > 0.0 {
            cell.parasite_load *= dp.parasite_cell_decay;
            if cell.parasite_load < 0.001 {
                cell.parasite_load = 0.0;
            }
        }
        // Traffic fouls the ground: the C3 pressure maps already measure crowding.
        let traffic = cell.prey_pressure + cell.pred_pressure;
        if traffic > 0.0 {
            cell.parasite_load = (cell.parasite_load + dp.parasite_ground_rate * traffic).min(1.0);
        }
    }
    // Carcasses seed the field: without a source the load would stay at zero forever.
    let w = world.width;
    let carcasses = world.carcasses.clone();
    for (x, y) in carcasses {
        if let Some(cell) = world.cells.get_mut(y * w + x) {
            cell.parasite_load = (cell.parasite_load + dp.parasite_carcass_seed).min(1.0);
        }
    }
}

/// Per-species mean Resistance over the living population.
pub fn mean_resistance(store: &CreatureStore) -> [f32; 6] {
    let mut sum = [0.0f32; 6];
    let mut n = [0u32; 6];
    for c in store.living() {
        sum[c.species.index()] += c.genome.resistance();
        n[c.species.index()] += 1;
    }
    std::array::from_fn(|i| if n[i] > 0 { sum[i] / crate::cast!(n[i] => f32) } else { 0.0 })
}

/// FR8: after the census — refresh per-pathogen stats, track outbreaks
/// (peaks, epidemic threshold, burn-out) and roll emergence. Returns alerts.
#[allow(clippy::too_many_arguments)]
pub fn daily_update(
    store: &mut CreatureStore,
    world: &World,
    events: &mut EventRing,
    time: &Time,
    dp: &DiseaseParams,
    state: &mut DiseaseState,
    population: &[u32; 6],
    rng: &mut Rng,
) -> Vec<Alert> {
    let mut alerts = Vec::new();
    if !dp.enabled {
        return alerts;
    }
    let day = crate::cast!(time.day_index() => u32);
    let n = state.pathogens.len();

    tally_stats(store, state, n, day);
    let resist = mean_resistance(store);
    let (ended, epidemics) = track_outbreaks(store, state, day, dp, population, resist);
    announce_epidemics(store, world, events, time, state, epidemics, &mut alerts);
    announce_ended(events, time, state, day, ended);
    seed_emergence(store, world, events, time, dp, state, n, day, population, resist, rng);
    alerts
}
