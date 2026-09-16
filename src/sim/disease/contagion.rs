//! Per-tick contagion plus parasite uptake and shedding.

use crate::sim::creatures::{Creature, CreatureId, CreatureStore};
use crate::sim::events::EventRing;
use crate::sim::geom;
use crate::sim::params::{DiseaseParams, Roster};
use crate::sim::rng::Rng;
use crate::sim::spatial::SpatialIndex;
use crate::sim::time::Time;
use crate::sim::world::{Terrain, World};
use super::types::{DiseaseState, PathogenId, Stage};
use super::effects::{is_infectious, new_infection, susceptibility};
use super::spillover::spillover;

/// FR4: infectious-first contagion pass over the previous tick's spatial index.
pub fn contagion_pass(store: &mut CreatureStore, spatial: &SpatialIndex, world: &World, time: &Time, dp: &DiseaseParams, state: &DiseaseState, rng: &mut Rng) {
    if !dp.enabled {
        return;
    }
    let day = crate::cast!(time.day_index() => u32);
    let infectious: Vec<CreatureId> = {
        let mut v: Vec<CreatureId> = store.living().filter(|c| is_infectious(c)).map(|c| c.id).collect();
        v.sort_unstable();
        v
    };
    if infectious.is_empty() {
        return;
    }
    let r = crate::cast!((dp.contact_cheb + 1) => u16);
    let mut queued: Vec<(CreatureId, CreatureId, PathogenId, u16)> = Vec::new();
    for src_id in infectious {
        let Some(src) = store.get(src_id) else { continue };
        let Some(inf) = src.infection else { continue };
        let Some(path) = state.pathogen(inf.pathogen) else { continue };
        let (sx, sy) = (src.x, src.y);
        let src_on_den = world.dens.iter().any(|&(x, y)| x == sx && y == sy);
        let mut contacts: Vec<CreatureId> = Vec::new();
        spatial.for_each_within(sx, sy, r, |id, px, py| {
            if id != src_id && geom::cheb(sx, sy, px, py) <= dp.contact_cheb {
                contacts.push(id);
            }
        });
        for t_id in contacts {
            let Some(t) = store.get(t_id) else { continue };
            if !t.alive {
                continue;
            }
            let s = susceptibility(t, inf.pathogen, day, dp, state);
            if s <= 0.0 {
                continue;
            }
            let den = src_on_den && world.dens.iter().any(|&(x, y)| x == t.x && y == t.y);
            let p = path.params.transmissibility * s * if den { path.params.den_bonus } else { 1.0 }
                + path.params.moisture_bonus * world.cell(t.x, t.y).moisture * s.min(1.0);
            if rng.chance(p.clamp(0.0, 1.0)) {
                queued.push((t_id, src_id, inf.pathogen, inf.outbreak));
            }
        }
    }
    queued.sort_unstable();
    let mut last: Option<CreatureId> = None;
    for (t_id, src_id, p, outbreak) in queued {
        if last == Some(t_id) {
            continue;
        }
        last = Some(t_id);
        let inf = match store.get(t_id) {
            Some(t) if t.alive && t.infection.is_none() => new_infection(t, p, Stage::Incubating, day, Some(src_id), outbreak, dp, state),
            _ => continue,
        };
        if let Some(t) = store.get_mut(t_id) {
            t.infection = Some(inf);
        }
    }
}

/// FR7: parasite uptake from the cell while grazing or drinking.
pub fn parasite_uptake(c: &mut Creature, world: &World, dp: &DiseaseParams) {
    if !dp.enabled {
        return;
    }
    let cell = world.cell(c.x, c.y);
    if cell.parasite_load <= 0.0 {
        return;
    }
    let water = if cell.terrain == Terrain::ShallowWater { dp.parasite_water_bonus } else { 1.0 };
    // Saturating uptake: the `(1 − load)` term gives the shed/uptake loop a
    // stable equilibrium below 1 instead of a runaway.
    let l = c.parasite_load;
    c.parasite_load = (l + dp.parasite_uptake * cell.parasite_load * (1.0 - c.genome.resistance()) * (1.0 - l) * water).min(1.0);
}

/// FR7: shed parasites onto the current cell (gated on a small load).
pub fn parasite_shed(c: &Creature, world: &mut World, dp: &DiseaseParams) {
    if !dp.enabled || c.parasite_load < 0.02 {
        return;
    }
    let cell = world.cell_mut(c.x, c.y);
    let water = if cell.terrain == Terrain::ShallowWater { dp.parasite_water_bonus } else { 1.0 };
    cell.parasite_load = (cell.parasite_load + dp.parasite_shed * c.parasite_load * water).min(1.0);
}

/// FR7: an eater took a meal from `carcass`: parasite transfer, carcass
/// transmission when the carcass died infectious, and (`FR8b`) the rare spillover
/// into a non-host eater. Returns the events to log.
#[allow(clippy::too_many_arguments)]
pub fn on_eat(
    store: &mut CreatureStore,
    eater_id: CreatureId,
    carcass_id: CreatureId,
    world: &World,
    events: &mut EventRing,
    time: &Time,
    roster: &Roster,
    dp: &DiseaseParams,
    state: &mut DiseaseState,
    rng: &mut Rng,
) {
    if !dp.enabled {
        return;
    }
    let day = crate::cast!(time.day_index() => u32);
    let (carcass_load, died_infected, cx, cy, carcass_species) = match store.get(carcass_id) {
        Some(k) => (k.parasite_load, k.died_infected, k.x, k.y, k.species),
        None => return,
    };
    if let Some(e) = store.get_mut(eater_id) {
        e.parasite_load = (e.parasite_load + dp.parasite_carcass_transfer * carcass_load).min(1.0);
    }
    let Some(p) = died_infected else { return };
    let Some(eater) = store.get(eater_id) else { return };
    if !eater.alive {
        return;
    }
    let is_host = state.pathogen(p).is_some_and(|path| path.host(eater.species) > 0.0);
    if is_host {
        let s = susceptibility(eater, p, day, dp, state);
        if s > 0.0 && rng.chance((dp.carcass_transmission * s).clamp(0.0, 1.0)) {
            let outbreak = state.open_outbreak(p).map_or(state.first_index, |(i, _)| i);
            let inf = new_infection(eater, p, Stage::Incubating, day, Some(carcass_id), outbreak, dp, state);
            if let Some(e) = store.get_mut(eater_id) {
                e.infection = Some(inf);
            }
        }
    } else if eater.infection.is_none() && rng.chance(dp.spillover_chance) {
        spillover(store, eater_id, p, (cx, cy), carcass_species, world, events, time, roster, dp, state, rng);
    }
}
