//! Infection lifecycle: incubation, death, recovery and parasite clearing.

use crate::sim::creatures::{Cause, CreatureId, CreatureStore, DeathTallies};
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::lineage::Lineage;
use crate::sim::params::DiseaseParams;
use crate::sim::time::Time;
use crate::sim::world::World;
use super::types::{DiseaseState, Infection, Pathogen, Stage};
use super::effects::infectious_days;

/// Move an incubating infection into its infectious stage.
pub(super) fn become_infectious(
    store: &mut CreatureStore,
    state: &mut DiseaseState,
    id: CreatureId,
    day: u32,
    inf: Infection,
    path: &Pathogen,
    dp: &DiseaseParams,
) {
    let Some((r, species)) = store.get(id).map(|c| (c.genome.resistance(), c.species)) else { return };
    let ends = day + infectious_days(&path.params, r, dp);
    if let Some(i) = store.get_mut(id).and_then(|c| c.infection.as_mut()) {
        i.stage = Stage::Infectious;
        i.ends_day = ends;
    }
    let slot = crate::cast!(inf.pathogen.0 => usize);
    state.stats[slot].total_cases += 1;
    state.last_case_day[slot] = day;
    if let Some(o) = state.outbreak_mut(inf.outbreak) {
        o.cases += 1;
        o.cases_today += 1;
        o.species_cases[species.index()] += 1;
    }
}

/// Kill a creature that failed its lethality roll.
#[allow(clippy::too_many_arguments)]
pub(super) fn disease_death(
    store: &mut CreatureStore,
    world: &mut World,
    events: &mut EventRing,
    time: &Time,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
    state: &mut DiseaseState,
    id: CreatureId,
    day: u32,
    inf: Infection,
    path: &Pathogen,
) {
    let species = match store.get(id) {
        Some(c) => c.species,
        None => return,
    };
    if let Some(c) = store.get_mut(id) {
        c.died_infected = Some(inf.pathogen);
        crate::sim::behavior::kill(c, Cause::Disease, world, events, time, tallies, lineage, None, 0, Some(path.name()));
    }
    let slot = crate::cast!(inf.pathogen.0 => usize);
    state.stats[slot].total_deaths += 1;
    state.last_case_day[slot] = day;
    if let Some(o) = state.outbreak_mut(inf.outbreak) {
        o.deaths += 1;
        o.species_deaths[species.index()] += 1;
    }
}

/// Clear an infection that ran its course and grant immunity.
#[allow(clippy::too_many_arguments)]
pub(super) fn recover(
    store: &mut CreatureStore,
    events: &mut EventRing,
    time: &Time,
    dp: &DiseaseParams,
    state: &mut DiseaseState,
    id: CreatureId,
    day: u32,
    inf: Infection,
    path: &Pathogen,
) {
    let slot = crate::cast!(inf.pathogen.0 => usize);
    let until = if path.params.immunity_days == 0 { u32::MAX } else { day + path.params.immunity_days };
    let Some(c) = store.get_mut(id) else { return };
    c.infection = None;
    c.immune_until[slot] = until;
    c.infections_survived = c.infections_survived.saturating_add(1);
    let (name, tag, x, y, species) = (c.name_str().to_string(), c.tag(), c.x, c.y, c.species);
    state.last_case_day[slot] = day;
    if let Some(o) = state.outbreak_mut(inf.outbreak) {
        o.recovered += 1;
    }
    if inf.severity >= dp.recovery_notable_min_severity {
        events.push(Event {
            year: time.year(),
            day: time.day_of_year(),
            hour: time.hour(),
            kind: EventKind::Recovery,
            species: Some(species),
            subject: Some(id),
            text: format!("{} {} recovered from {}", name, tag, path.name()),
            pos: Some((x, y)),
            detail: String::new(),
        });
    }
}

/// Bleed off parasite load towards zero across the living population.
pub(super) fn clear_parasites(store: &mut CreatureStore, dp: &DiseaseParams) {
    for c in store.living_mut() {
        if c.parasite_load > 0.0 {
            c.parasite_load = (c.parasite_load - dp.parasite_clearance * (0.5 + c.genome.resistance())).max(0.0);
        }
    }
}
