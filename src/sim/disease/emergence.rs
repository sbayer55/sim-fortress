//! New-pathogen emergence from reservoir regions.

use crate::sim::creatures::CreatureStore;
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::params::DiseaseParams;
use crate::sim::rng::Rng;
use crate::sim::time::Time;
use crate::sim::world::World;
use super::types::{DiseaseState, Outbreak, PathogenId, Stage};
use super::effects::new_infection;

/// Section 3: roll for reservoir emergence of roster pathogens.
#[allow(clippy::too_many_arguments)]
pub(super) fn seed_emergence(
    store: &mut CreatureStore,
    world: &World,
    events: &mut EventRing,
    time: &Time,
    dp: &DiseaseParams,
    state: &mut DiseaseState,
    n: usize,
    day: u32,
    population: &[u32; 6],
    resist: [f32; 6],
    rng: &mut Rng,
) {
    for slot in 0..n {
        let pid = PathogenId(crate::cast!(slot => u8));
        let (is_strain, active) = (state.pathogens[slot].is_strain(), state.stats[slot].active);
        if is_strain || active > 0 {
            continue;
        }
        let last = state.last_case_day[slot];
        if last != u32::MAX && day_guard(last, day, dp) {
            continue;
        }
        let hosts = state.hosts_living(pid, population);
        if hosts < dp.emergence_host_min {
            continue;
        }
        let hazard = dp.emergence_per_day * crate::cast!(hosts => f32) / crate::cast!(dp.emergence_host_ref.max(1) => f32);
        if !rng.chance(hazard.clamp(0.0, 1.0)) {
            continue;
        }
        emerge(store, world, events, time, dp, state, pid, resist);
    }
}

/// True while a pathogen is still inside its post-case reservoir window.
const fn day_guard(last: u32, day: u32, dp: &DiseaseParams) -> bool {
    day.saturating_sub(last) < dp.reservoir_days
}

/// Regions holding at least one active case of `outbreak`.
pub(super) fn regions_with_cases(store: &CreatureStore, world: &World, outbreak: u16, _first: u16) -> usize {
    let mut seen = [false; 8];
    for c in store.living() {
        if c.infection.is_some_and(|i| i.outbreak == outbreak) {
            seen[world.region_index(c.x, c.y).min(7)] = true;
        }
    }
    seen.iter().filter(|&&s| s).count()
}

/// FR8: pick the index case (densest region, lowest Resistance, ties by id),
/// push the outbreak record and emit the `Outbreak` event.
#[allow(clippy::too_many_arguments)]
fn emerge(store: &mut CreatureStore, world: &World, events: &mut EventRing, time: &Time, dp: &DiseaseParams, state: &mut DiseaseState, pid: PathogenId, resist: [f32; 6]) {
    let day = crate::cast!(time.day_index() => u32);
    let Some(path) = state.pathogen(pid).cloned() else { return };
    let mut region_hosts = [0u32; 8];
    for c in store.living() {
        if path.host(c.species) > 0.0 && c.infection.is_none() {
            region_hosts[world.region_index(c.x, c.y).min(7)] += 1;
        }
    }
    let Some(region) = (0..8).max_by_key(|&i| (region_hosts[i], std::cmp::Reverse(i))) else { return };
    if region_hosts[region] == 0 {
        return;
    }
    let index_case = store
        .living()
        .filter(|c| path.host(c.species) > 0.0 && c.infection.is_none() && world.region_index(c.x, c.y).min(7) == region)
        .min_by(|a, b| a.genome.resistance().partial_cmp(&b.genome.resistance()).unwrap_or(std::cmp::Ordering::Equal).then(a.id.cmp(&b.id)))
        .map(|c| (c.id, c.species, c.x, c.y));
    let Some((id, species, x, y)) = index_case else { return };
    let mut cases = [0u32; 6];
    cases[species.index()] = 1;
    let index = state.push_outbreak(Outbreak {
        pathogen: pid,
        started_day: day,
        ended_day: None,
        origin_region: crate::cast!(region => u8),
        index_case: id,
        cases: 1,
        deaths: 0,
        recovered: 0,
        peak_active: 1,
        peak_day: day,
        species_cases: cases,
        species_deaths: [0; 6],
        epidemic: false,
        resist_at_start: resist,
        resist_at_end: resist,
        active: 1,
        cases_today: 1,
    });
    let slot = crate::cast!(pid.0 => usize);
    state.stats[slot].outbreaks += 1;
    state.stats[slot].total_cases += 1;
    state.last_case_day[slot] = day;
    let inf = match store.get(id) {
        Some(c) => new_infection(c, pid, Stage::Infectious, day, None, index, dp, state),
        None => return,
    };
    if let Some(c) = store.get_mut(id) {
        c.infection = Some(inf);
    }
    let region_name = world.regions.get(region).map(|r| r.0.clone()).unwrap_or_default();
    events.push(Event {
        year: time.year(),
        day: time.day_of_year(),
        hour: time.hour(),
        kind: EventKind::Outbreak,
        species: Some(species),
        subject: Some(id),
        text: format!("{} breaks out among the {} of {}", path.name(), species.plural().to_lowercase(), region_name),
        pos: Some((x, y)),
        detail: String::new(),
    });
}
