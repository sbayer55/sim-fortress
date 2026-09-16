//! Outbreak tracking, epidemic announcements and disease tallies.

use crate::sim::creatures::{Creature, CreatureStore};
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::params::DiseaseParams;
use crate::sim::time::Time;
use crate::sim::world::World;
use crate::sim::Alert;
use super::types::{DiseaseState, Pathogen};
use super::emergence::regions_with_cases;

/// Section 1: per-pathogen active/immune counts from the living set.
pub(super) fn tally_stats(store: &CreatureStore, state: &mut DiseaseState, n: usize, day: u32) {
    for s in &mut state.stats {
        s.active = 0;
        s.active_by_species.fill(0);
        s.immune = 0;
    }
    for c in store.living() {
        count_one(state, c, n, day);
    }
    for slot in 0..n {
        state.stats[slot].peak_active = state.stats[slot].peak_active.max(state.stats[slot].active);
    }
}

/// Add one living creature to the per-pathogen active and immune tallies.
fn count_one(state: &mut DiseaseState, c: &Creature, n: usize, day: u32) {
    if let Some(inf) = c.infection {
        let slot = crate::cast!(inf.pathogen.0 => usize);
        if slot < n {
            state.stats[slot].active += 1;
            state.stats[slot].active_by_species[c.species.index()] += 1;
        }
    }
    for slot in 0..n {
        if day < c.immune_until[slot] {
            state.stats[slot].immune += 1;
        }
    }
}

/// Section 2: advance outbreak bookkeeping and flag ended/epidemic outbreaks.
pub(super) fn track_outbreaks(
    store: &CreatureStore,
    state: &mut DiseaseState,
    day: u32,
    dp: &DiseaseParams,
    population: &[u32],
    resist: &[f32],
) -> (Vec<u16>, Vec<u16>) {
    let mut active_by_outbreak: Vec<u32> = vec![0; state.outbreaks.len()];
    for c in store.living() {
        if let Some(inf) = c.infection {
            if let Some(i) = inf.outbreak.checked_sub(state.first_index) {
                if let Some(a) = active_by_outbreak.get_mut(crate::cast!(i => usize)) {
                    *a += 1;
                }
            }
        }
    }
    let mut ended: Vec<u16> = Vec::new();
    let mut epidemics: Vec<u16> = Vec::new();
    let hosts_by_outbreak: Vec<u32> = state.outbreaks.iter().map(|o| state.hosts_living(o.pathogen, population)).collect();
    for (i, o) in state.outbreaks.iter_mut().enumerate() {
        if o.ended_day.is_some() {
            continue;
        }
        let active = active_by_outbreak[i];
        o.active = active;
        if active > o.peak_active {
            o.peak_active = active;
            o.peak_day = day;
        }
        let index = state.first_index + crate::cast!(i => u16);
        if active == 0 {
            o.ended_day = Some(day);
            o.resist_at_end = resist.to_vec();
            ended.push(index);
        }
        // Epidemic threshold, once per outbreak.
        let hosts: u32 = hosts_by_outbreak[i];
        if !o.epidemic && active >= dp.epidemic_min_cases && crate::cast!(active => f32) >= dp.epidemic_share * crate::cast!(hosts => f32) {
            o.epidemic = true;
            epidemics.push(index);
        }
    }
    (ended, epidemics)
}

/// Section 2b: emit an event and alert for each newly epidemic outbreak.
pub(super) fn announce_epidemics(
    store: &CreatureStore,
    world: &World,
    events: &mut EventRing,
    time: &Time,
    state: &DiseaseState,
    epidemics: Vec<u16>,
    alerts: &mut Vec<Alert>,
) {
    for index in epidemics {
        let Some(o) = state.outbreak(index) else { continue };
        let pid = o.pathogen;
        let (active, regions) = (o.active, regions_with_cases(store, world, index, state.first_index));
        let name = state.name(pid).to_string();
        let strain = state.pathogen(pid).is_some_and(Pathogen::is_strain);
        let subject = o.index_case;
        events.push(Event {
            year: time.year(),
            day: time.day_of_year(),
            hour: time.hour(),
            kind: EventKind::Epidemic,
            species: None,
            subject: Some(subject),
            text: if strain {
                format!("A new strain is epidemic: {name} — {active} sick across {regions} regions")
            } else {
                format!("{name} is epidemic: {active} sick across {regions} regions")
            },
            pos: None,
            detail: String::new(),
        });
        alerts.push(Alert::Epidemic { event_index: events.total(), pathogen: pid, outbreak: index });
    }
}

/// Section 2c: emit a burn-out event for each outbreak that ended today.
pub(super) fn announce_ended(events: &mut EventRing, time: &Time, state: &mut DiseaseState, day: u32, ended: Vec<u16>) {
    for index in ended {
        let Some(o) = state.outbreak(index) else { continue };
        let pid = o.pathogen;
        let (days, dead, recovered, epidemic) = (o.duration_days(day), o.deaths, o.recovered, o.epidemic);
        let name = state.name(pid).to_string();
        let slot = crate::cast!(pid.0 => usize);
        let strain = state.pathogens.get(slot).is_some_and(Pathogen::is_strain);
        if strain && state.stats[slot].active == 0 {
            state.pathogens[slot].extinct = true;
        }
        events.push(Event {
            year: time.year(),
            day: time.day_of_year(),
            hour: time.hour(),
            kind: if epidemic { EventKind::EpidemicOver } else { EventKind::Note },
            species: None,
            subject: None,
            text: format!(
                "The {} outbreak has burned out after {} days: {} dead, {} recovered{}",
                name,
                days,
                dead,
                recovered,
                if strain { "; the strain is gone" } else { "" }
            ),
            pos: None,
            detail: String::new(),
        });
    }
}
