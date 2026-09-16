//! Strain mutation, cross-species spillover and vertical transmission at birth.

use crate::sim::creatures::{Creature, CreatureId, CreatureStore};
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::params::{DiseaseParams, Roster};
use crate::sim::rng::Rng;
use crate::sim::species::SpeciesId;
use crate::sim::time::Time;
use crate::sim::world::World;
use super::types::{DiseaseState, Outbreak, Pathogen, PathogenId, PathogenStats, Stage};
use super::effects::new_infection;
use super::daily::mean_resistance;

/// `FR8b`: create a strain of `parent` in the eater's species and seed it.
#[allow(clippy::too_many_arguments)]
/// A jittered, single-host copy of `parent` adapted to the eater's species.
fn spillover_strain(
    store: &CreatureStore,
    eater_id: CreatureId,
    parent: PathogenId,
    day: u32,
    roster: &Roster,
    dp: &DiseaseParams,
    state: &DiseaseState,
    rng: &mut Rng,
) -> Option<(Pathogen, SpeciesId)> {
    let species = store.get(eater_id)?.species;
    let parent_p = state.pathogen(parent)?.clone();
    let root_name = state.name(state.root(parent)).to_string();
    let jitter = |rng: &mut Rng| (1.0 + rng.gauss(0.0, dp.spillover_jitter)).clamp(0.25, 2.0);
    let mut params = parent_p.params;
    params.name = format!("{} ({} strain)", root_name, roster.plural(species).to_lowercase());
    params.hosts = std::iter::once((roster.name(species).to_string(), 1.0)).collect();
    let mut host_by_species = vec![0.0; roster.len()];
    host_by_species[species.index()] = 1.0;
    params.transmissibility *= jitter(rng);
    params.lethality_per_day *= jitter(rng);
    params.infectious_days = (crate::cast!((crate::cast!(params.infectious_days => f32) * jitter(rng)).round() => u32)).max(2);
    let strain = Pathogen { params, host_by_species, parent: Some(parent), born_day: Some(day), extinct: false };
    Some((strain, species))
}

/// Append the strain, or reuse the oldest extinct strain past its reservoir.
fn strain_slot(
    state: &mut DiseaseState,
    store: &mut CreatureStore,
    day: u32,
    dp: &DiseaseParams,
    strain: Pathogen,
) -> Option<usize> {
    if state.pathogens.len() < dp.max_pathogens() {
        state.pathogens.push(strain);
        return Some(state.pathogens.len() - 1);
    }
    let reusable = state
        .pathogens
        .iter()
        .enumerate()
        .filter(|(i, p)| p.is_strain() && p.extinct && day.saturating_sub(state.last_case_day[*i]) >= dp.reservoir_days)
        .min_by_key(|(i, p)| (p.born_day.unwrap_or(0), *i))
        .map(|(i, _)| i)?;
    state.pathogens[reusable] = strain;
    state.stats[reusable] = PathogenStats::default();
    state.last_case_day[reusable] = u32::MAX;
    for c in store.living_mut() {
        c.immune_until[reusable] = 0;
    }
    Some(reusable)
}

/// Open the index outbreak for a freshly seeded strain.
#[allow(clippy::too_many_arguments)]
fn open_strain_outbreak(
    state: &mut DiseaseState,
    store: &CreatureStore,
    eater_id: CreatureId,
    slot: usize,
    species: SpeciesId,
    n_species: usize,
    region: u8,
    day: u32,
) -> u16 {
    let resist = mean_resistance(store, n_species);
    let mut cases = vec![0u32; n_species];
    cases[species.index()] = 1;
    let index = state.push_outbreak(Outbreak {
        pathogen: PathogenId(crate::cast!(slot => u8)),
        started_day: day,
        ended_day: None,
        origin_region: region,
        index_case: eater_id,
        cases: 1,
        deaths: 0,
        recovered: 0,
        peak_active: 1,
        peak_day: day,
        species_cases: cases,
        species_deaths: vec![0; n_species],
        epidemic: false,
        resist_at_start: resist.clone(),
        resist_at_end: resist,
        active: 1,
        cases_today: 1,
    });
    state.stats[slot].outbreaks += 1;
    state.stats[slot].total_cases += 1;
    state.last_case_day[slot] = day;
    index
}

/// `FR8b`: create a strain of `parent` in the eater's species and seed it.
#[allow(clippy::too_many_arguments)]
pub(super) fn spillover(
    store: &mut CreatureStore,
    eater_id: CreatureId,
    parent: PathogenId,
    carcass_pos: (usize, usize),
    prey_species: SpeciesId,
    world: &World,
    events: &mut EventRing,
    time: &Time,
    roster: &Roster,
    dp: &DiseaseParams,
    state: &mut DiseaseState,
    rng: &mut Rng,
) {
    let day = crate::cast!(time.day_index() => u32);
    let Some((strain, species)) = spillover_strain(store, eater_id, parent, day, roster, dp, state, rng) else { return };
    let root_name = state.name(state.root(parent)).to_string();
    let Some(slot) = strain_slot(state, store, day, dp, strain) else {
        state.failed_spillovers += 1;
        return;
    };
    let pid = PathogenId(crate::cast!(slot => u8));
    let region = crate::cast!(world.region_index(carcass_pos.0, carcass_pos.1).min(7) => u8);
    let index = open_strain_outbreak(state, store, eater_id, slot, species, roster.len(), region, day);
    let inf = match store.get(eater_id) {
        Some(e) => new_infection(e, pid, Stage::Infectious, day, None, index, dp, state),
        None => return,
    };
    let (name, tag, ex, ey) = match store.get_mut(eater_id) {
        Some(e) => {
            e.infection = Some(inf);
            (e.name_str(roster).to_string(), e.tag(roster), e.x, e.y)
        }
        None => return,
    };
    events.push(Event {
        year: time.year(),
        day: time.day_of_year(),
        hour: time.hour(),
        kind: EventKind::Spillover,
        species: Some(species),
        subject: Some(eater_id),
        text: format!("{} has jumped to the {}: {} {} ate a sick {}", root_name, roster.plural(species).to_lowercase(), name, tag, roster.name(prey_species)),
        pos: Some((ex, ey)),
        detail: format!("new strain {} in slot {}", state.name(pid), slot),
    });
}

/// FR7: a newborn of an infectious mother may start incubating; parasites
/// pass down as a share of the mother's load.
pub fn at_birth(child: &mut Creature, mother: &Creature, time: &Time, dp: &DiseaseParams, state: &DiseaseState, rng: &mut Rng) {
    if !dp.enabled {
        return;
    }
    child.parasite_load = (dp.parasite_birth_transfer * mother.parasite_load).max(dp.parasite_baseline).min(1.0);
    if let Some(inf) = mother.infection.filter(|i| i.stage == Stage::Infectious) {
        if rng.chance(dp.vertical_transmission) {
            let day = crate::cast!(time.day_index() => u32);
            child.infection = Some(new_infection(child, inf.pathogen, Stage::Incubating, day, Some(mother.id), inf.outbreak, dp, state));
        }
    }
}
