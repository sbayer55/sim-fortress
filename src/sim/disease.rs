//! Disease and parasites (C7). Contagious pathogens spread by proximity and
//! through carcasses; a continuous parasite load builds up from fouled ground
//! and water; the Resistance gene gates both. Outbreaks are recorded as named
//! events, and a pathogen can mutate into a predator's species when the predator
//! eats infected prey (spillover, FR8b).
//!
//! Determinism: infectious creatures are visited in id order, queued infections
//! are applied in `(target, source)` order, and every roll comes from the
//! dedicated `disease_rng` stream.

use serde::{Deserialize, Serialize};

use crate::sim::creatures::{Cause, Creature, CreatureId, CreatureStore, DeathTallies};
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::geom;
use crate::sim::lineage::Lineage;
use crate::sim::params::{DiseaseParams, PathogenParams};
use crate::sim::rng::Rng;
use crate::sim::spatial::SpatialIndex;
use crate::sim::species::SpeciesId;
use crate::sim::time::Time;
use crate::sim::world::{Terrain, World};
use crate::sim::Alert;

/// Width of the per-creature immunity table: roster plus strains, at most 8.
pub const MAX_PATHOGENS: usize = 8;

/// Index into `DiseaseState::pathogens` (never into the params roster).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PathogenId(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Stage {
    Incubating,
    Infectious,
}

/// One contagious infection (a creature carries at most one at a time, FR3).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Infection {
    pub pathogen: PathogenId,
    pub stage: Stage,
    pub since_day: u32,
    /// Day of the next stage transition (resistance-adjusted at onset).
    pub ends_day: u32,
    /// `pathogen.severity × (1 − 0.5 × resistance)`; drives the effects.
    pub severity: f32,
    pub source: Option<CreatureId>,
    /// Absolute outbreak index (see `DiseaseState::outbreak`).
    pub outbreak: u16,
}

/// A live pathogen: the roster record plus strain bookkeeping (FR8b).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pathogen {
    pub params: PathogenParams,
    /// The pathogen this strain mutated from (`None` for the roster).
    pub parent: Option<PathogenId>,
    pub born_day: Option<u32>,
    /// A strain whose last case is gone; its slot may be reused.
    pub extinct: bool,
}

impl Pathogen {
    pub fn name(&self) -> &str {
        &self.params.name
    }

    pub fn host(&self, s: SpeciesId) -> f32 {
        self.params.hosts.get(&s).copied().unwrap_or(0.0)
    }

    pub fn is_strain(&self) -> bool {
        self.parent.is_some()
    }
}

/// One outbreak of one pathogen, from index case to burn-out (FR8).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Outbreak {
    pub pathogen: PathogenId,
    pub started_day: u32,
    pub ended_day: Option<u32>,
    pub origin_region: u8,
    pub index_case: CreatureId,
    /// Creatures that reached the infectious stage.
    pub cases: u32,
    pub deaths: u32,
    pub recovered: u32,
    pub peak_active: u32,
    pub peak_day: u32,
    pub species_cases: [u32; 6],
    pub species_deaths: [u32; 6],
    pub epidemic: bool,
    /// Per-species mean Resistance on the start day and at burn-out.
    pub resist_at_start: [f32; 6],
    pub resist_at_end: [f32; 6],
    /// Daily active count, refreshed by `daily_update`.
    pub active: u32,
    /// New infectious cases today (for the sidebar's `+N` line).
    pub cases_today: u32,
}

impl Outbreak {
    pub fn duration_days(&self, today: u32) -> u32 {
        self.ended_day.unwrap_or(today).saturating_sub(self.started_day)
    }
}

/// Per-pathogen running statistics, refreshed daily.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PathogenStats {
    pub outbreaks: u32,
    pub total_cases: u32,
    pub total_deaths: u32,
    pub active: u32,
    pub active_by_species: [u32; 6],
    pub peak_active: u32,
    pub immune: u32,
}

/// Everything the disease system owns on the `Sim`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiseaseState {
    pub pathogens: Vec<Pathogen>,
    /// Day of the last case per pathogen slot (`u32::MAX` = never).
    pub last_case_day: [u32; MAX_PATHOGENS],
    pub stats: [PathogenStats; MAX_PATHOGENS],
    /// Outbreak history, bounded to `OUTBREAKS_MAX`; `first_index` keeps the
    /// absolute indices stored on infections stable when the oldest is dropped.
    pub outbreaks: Vec<Outbreak>,
    pub first_index: u16,
    /// Spillovers that found no free slot (diagnostics only).
    pub failed_spillovers: u32,
}

pub const OUTBREAKS_MAX: usize = 64;

impl DiseaseState {
    pub fn new(dp: &DiseaseParams) -> Self {
        let pathogens = dp
            .pathogens
            .iter()
            .take(dp.max_pathogens())
            .map(|p| Pathogen { params: p.clone(), parent: None, born_day: None, extinct: false })
            .collect();
        DiseaseState {
            pathogens,
            last_case_day: [u32::MAX; MAX_PATHOGENS],
            stats: [PathogenStats::default(); MAX_PATHOGENS],
            outbreaks: Vec::new(),
            first_index: 0,
            failed_spillovers: 0,
        }
    }

    pub fn pathogen(&self, id: PathogenId) -> Option<&Pathogen> {
        self.pathogens.get(id.0 as usize)
    }

    /// The name of a pathogen slot, or `?` when the slot is empty.
    pub fn name(&self, id: PathogenId) -> &str {
        self.pathogen(id).map(|p| p.name()).unwrap_or("?")
    }

    /// The roster ancestor of a strain (itself for roster pathogens).
    pub fn root(&self, id: PathogenId) -> PathogenId {
        let mut cur = id;
        for _ in 0..MAX_PATHOGENS {
            match self.pathogen(cur).and_then(|p| p.parent) {
                Some(p) => cur = p,
                None => return cur,
            }
        }
        cur
    }

    pub fn outbreak(&self, index: u16) -> Option<&Outbreak> {
        let i = index.checked_sub(self.first_index)? as usize;
        self.outbreaks.get(i)
    }

    pub fn outbreak_mut(&mut self, index: u16) -> Option<&mut Outbreak> {
        let i = index.checked_sub(self.first_index)? as usize;
        self.outbreaks.get_mut(i)
    }

    /// The most recent still-open outbreak of `id`, if any.
    pub fn open_outbreak(&self, id: PathogenId) -> Option<(u16, &Outbreak)> {
        self.outbreaks
            .iter()
            .enumerate()
            .rev()
            .find(|(_, o)| o.pathogen == id && o.ended_day.is_none())
            .map(|(i, o)| (self.first_index + i as u16, o))
    }

    fn push_outbreak(&mut self, o: Outbreak) -> u16 {
        if self.outbreaks.len() >= OUTBREAKS_MAX {
            self.outbreaks.remove(0);
            self.first_index += 1;
        }
        self.outbreaks.push(o);
        self.first_index + (self.outbreaks.len() - 1) as u16
    }

    /// Living hosts of a pathogen (host multiplier > 0), all species.
    pub fn hosts_living(&self, id: PathogenId, population: &[u32; 6]) -> u32 {
        let Some(p) = self.pathogen(id) else { return 0 };
        SpeciesId::ALL.iter().filter(|s| p.host(**s) > 0.0).map(|s| population[s.index()]).sum()
    }
}

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
    let severity = infectious.map(|i| i.severity).unwrap_or(0.0);
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
    c.immune_until.get(p.0 as usize).is_some_and(|&until| day < until)
}

/// Susceptibility of `c` to pathogen `p`: host multiplier, resistance, and
/// cross-immunity from the parent of a strain. `0.0` = cannot be infected.
fn susceptibility(c: &Creature, p: PathogenId, day: u32, dp: &DiseaseParams, state: &DiseaseState) -> f32 {
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
        let immune_to_kin = (0..state.pathogens.len() as u8)
            .map(PathogenId)
            .filter(|&q| q != p && state.root(q) == root)
            .any(|q| is_immune(c, q, day));
        if immune_to_kin {
            s *= 1.0 - dp.spillover_cross_immunity;
        }
    }
    s.max(0.0)
}

/// Build a fresh infection of `p` for `c` in the given stage.
fn new_infection(c: &Creature, p: PathogenId, stage: Stage, day: u32, source: Option<CreatureId>, outbreak: u16, dp: &DiseaseParams, state: &DiseaseState) -> Infection {
    let path = state.pathogen(p).expect("pathogen slot");
    let r = c.genome.resistance();
    let ends_day = match stage {
        Stage::Incubating => day + path.params.incubation_days,
        Stage::Infectious => day + infectious_days(&path.params, r, dp),
    };
    Infection { pathogen: p, stage, since_day: day, ends_day, severity: path.params.severity * (1.0 - 0.5 * r), source, outbreak }
}

fn infectious_days(p: &PathogenParams, resistance: f32, dp: &DiseaseParams) -> u32 {
    ((p.infectious_days as f32 * (1.0 - dp.duration_resist_w * resistance)).round() as u32).max(2)
}

// ------------------------------------------------------------------ per tick

/// FR4: infectious-first contagion pass over the previous tick's spatial index.
pub fn contagion_pass(store: &mut CreatureStore, spatial: &SpatialIndex, world: &World, time: &Time, dp: &DiseaseParams, state: &DiseaseState, rng: &mut Rng) {
    if !dp.enabled {
        return;
    }
    let day = time.day_index() as u32;
    let infectious: Vec<CreatureId> = {
        let mut v: Vec<CreatureId> = store.living().filter(|c| is_infectious(c)).map(|c| c.id).collect();
        v.sort_unstable();
        v
    };
    if infectious.is_empty() {
        return;
    }
    let r = (dp.contact_cheb + 1) as u16;
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
/// transmission when the carcass died infectious, and (FR8b) the rare spillover
/// into a non-host eater. Returns the events to log.
#[allow(clippy::too_many_arguments)]
pub fn on_eat(
    store: &mut CreatureStore,
    eater_id: CreatureId,
    carcass_id: CreatureId,
    world: &World,
    events: &mut EventRing,
    time: &Time,
    dp: &DiseaseParams,
    state: &mut DiseaseState,
    rng: &mut Rng,
) {
    if !dp.enabled {
        return;
    }
    let day = time.day_index() as u32;
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
            let outbreak = state.open_outbreak(p).map(|(i, _)| i).unwrap_or(state.first_index);
            let inf = new_infection(eater, p, Stage::Incubating, day, Some(carcass_id), outbreak, dp, state);
            if let Some(e) = store.get_mut(eater_id) {
                e.infection = Some(inf);
            }
        }
    } else if eater.infection.is_none() && rng.chance(dp.spillover_chance) {
        spillover(store, eater_id, p, (cx, cy), carcass_species, world, events, time, dp, state, rng);
    }
}

/// FR8b: create a strain of `parent` in the eater's species and seed it.
#[allow(clippy::too_many_arguments)]
fn spillover(
    store: &mut CreatureStore,
    eater_id: CreatureId,
    parent: PathogenId,
    carcass_pos: (usize, usize),
    prey_species: SpeciesId,
    world: &World,
    events: &mut EventRing,
    time: &Time,
    dp: &DiseaseParams,
    state: &mut DiseaseState,
    rng: &mut Rng,
) {
    let day = time.day_index() as u32;
    let Some(eater) = store.get(eater_id) else { return };
    let species = eater.species;
    let Some(parent_p) = state.pathogen(parent).cloned() else { return };
    let root = state.root(parent);
    let root_name = state.name(root).to_string();

    // Jittered copy with a single host.
    let jitter = |rng: &mut Rng| (1.0 + rng.gauss(0.0, dp.spillover_jitter)).clamp(0.25, 2.0);
    let mut params = parent_p.params.clone();
    params.name = format!("{} ({} strain)", root_name, species.plural().to_lowercase());
    params.hosts = [(species, 1.0)].into_iter().collect();
    params.transmissibility *= jitter(rng);
    params.lethality_per_day *= jitter(rng);
    params.infectious_days = ((params.infectious_days as f32 * jitter(rng)).round() as u32).max(2);
    let strain = Pathogen { params, parent: Some(parent), born_day: Some(day), extinct: false };

    // Slot rule: append, else reuse the oldest extinct strain past its reservoir.
    let slot = if state.pathogens.len() < dp.max_pathogens() {
        state.pathogens.push(strain);
        Some(state.pathogens.len() - 1)
    } else {
        let reusable = state
            .pathogens
            .iter()
            .enumerate()
            .filter(|(i, p)| p.is_strain() && p.extinct && day.saturating_sub(state.last_case_day[*i]) >= dp.reservoir_days)
            .min_by_key(|(i, p)| (p.born_day.unwrap_or(0), *i))
            .map(|(i, _)| i);
        match reusable {
            Some(i) => {
                state.pathogens[i] = strain;
                state.stats[i] = PathogenStats::default();
                state.last_case_day[i] = u32::MAX;
                for c in store.living_mut() {
                    c.immune_until[i] = 0;
                }
                Some(i)
            }
            None => None,
        }
    };
    let Some(slot) = slot else {
        state.failed_spillovers += 1;
        return;
    };
    let pid = PathogenId(slot as u8);
    let region = world.region_index(carcass_pos.0, carcass_pos.1).min(7) as u8;
    let resist = mean_resistance(store);
    let mut cases = [0u32; 6];
    cases[species.index()] = 1;
    let index = state.push_outbreak(Outbreak {
        pathogen: pid,
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
        species_deaths: [0; 6],
        epidemic: false,
        resist_at_start: resist,
        resist_at_end: resist,
        active: 1,
        cases_today: 1,
    });
    state.stats[slot].outbreaks += 1;
    state.stats[slot].total_cases += 1;
    state.last_case_day[slot] = day;
    let inf = match store.get(eater_id) {
        Some(e) => new_infection(e, pid, Stage::Infectious, day, None, index, dp, state),
        None => return,
    };
    let (name, tag, ex, ey) = match store.get_mut(eater_id) {
        Some(e) => {
            e.infection = Some(inf);
            (e.name_str().to_string(), e.tag(), e.x, e.y)
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
        text: format!("{} has jumped to the {}: {} {} ate a sick {}", root_name, species.plural().to_lowercase(), name, tag, prey_species.name().to_lowercase()),
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
            let day = time.day_index() as u32;
            child.infection = Some(new_infection(child, inf.pathogen, Stage::Incubating, day, Some(mother.id), inf.outbreak, dp, state));
        }
    }
}

// ------------------------------------------------------------------ daily

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
    let day = time.day_index() as u32;
    for o in state.outbreaks.iter_mut() {
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
            Stage::Incubating => {
                if day >= inf.ends_day {
                    let ends = day + infectious_days(&path.params, r, dp);
                    let species = c.species;
                    if let Some(c) = store.get_mut(id) {
                        if let Some(i) = c.infection.as_mut() {
                            i.stage = Stage::Infectious;
                            i.ends_day = ends;
                        }
                    }
                    let slot = inf.pathogen.0 as usize;
                    state.stats[slot].total_cases += 1;
                    state.last_case_day[slot] = day;
                    if let Some(o) = state.outbreak_mut(inf.outbreak) {
                        o.cases += 1;
                        o.cases_today += 1;
                        o.species_cases[species.index()] += 1;
                    }
                }
            }
            Stage::Infectious => {
                let hazard = (path.params.lethality_per_day * host * (1.0 - dp.lethality_resist_w * r)).clamp(0.0, 1.0);
                if rng.chance(hazard) {
                    let species = c.species;
                    if let Some(c) = store.get_mut(id) {
                        c.died_infected = Some(inf.pathogen);
                        crate::sim::behavior::kill(c, Cause::Disease, world, events, time, tallies, lineage, None, 0, Some(path.name()));
                    }
                    let slot = inf.pathogen.0 as usize;
                    state.stats[slot].total_deaths += 1;
                    state.last_case_day[slot] = day;
                    if let Some(o) = state.outbreak_mut(inf.outbreak) {
                        o.deaths += 1;
                        o.species_deaths[species.index()] += 1;
                    }
                } else if day >= inf.ends_day {
                    let until = if path.params.immunity_days == 0 { u32::MAX } else { day + path.params.immunity_days };
                    let slot = inf.pathogen.0 as usize;
                    let (name, tag, x, y, species) = {
                        let c = store.get_mut(id).expect("living");
                        c.infection = None;
                        c.immune_until[slot] = until;
                        c.infections_survived = c.infections_survived.saturating_add(1);
                        (c.name_str().to_string(), c.tag(), c.x, c.y, c.species)
                    };
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
            }
        }
    }
    // Parasite clearance.
    for c in store.living_mut() {
        if c.parasite_load > 0.0 {
            c.parasite_load = (c.parasite_load - dp.parasite_clearance * (0.5 + c.genome.resistance())).max(0.0);
        }
    }
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
    std::array::from_fn(|i| if n[i] > 0 { sum[i] / n[i] as f32 } else { 0.0 })
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
    let day = time.day_index() as u32;
    let n = state.pathogens.len();

    // 1. Stats from the living set.
    for s in state.stats.iter_mut() {
        s.active = 0;
        s.active_by_species = [0; 6];
        s.immune = 0;
    }
    let mut active_by_outbreak: Vec<u32> = vec![0; state.outbreaks.len()];
    for c in store.living() {
        if let Some(inf) = c.infection {
            let slot = inf.pathogen.0 as usize;
            if slot < n {
                state.stats[slot].active += 1;
                state.stats[slot].active_by_species[c.species.index()] += 1;
            }
            if let Some(i) = inf.outbreak.checked_sub(state.first_index) {
                if let Some(a) = active_by_outbreak.get_mut(i as usize) {
                    *a += 1;
                }
            }
        }
        for slot in 0..n {
            if day < c.immune_until[slot] {
                state.stats[slot].immune += 1;
            }
        }
    }
    for slot in 0..n {
        state.stats[slot].peak_active = state.stats[slot].peak_active.max(state.stats[slot].active);
    }

    // 2. Outbreak tracking.
    let resist = mean_resistance(store);
    let mut ended: Vec<u16> = Vec::new();
    let mut epidemics: Vec<u16> = Vec::new();
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
        let index = state.first_index + i as u16;
        if active == 0 {
            o.ended_day = Some(day);
            o.resist_at_end = resist;
            ended.push(index);
        }
        // Epidemic threshold, once per outbreak.
        let pid = o.pathogen;
        let hosts: u32 = SpeciesId::ALL
            .iter()
            .filter(|s| state.pathogens.get(pid.0 as usize).is_some_and(|p| p.host(**s) > 0.0))
            .map(|s| population[s.index()])
            .sum();
        if !o.epidemic && active >= dp.epidemic_min_cases && active as f32 >= dp.epidemic_share * hosts as f32 {
            o.epidemic = true;
            epidemics.push(index);
        }
    }
    for index in epidemics {
        let Some(o) = state.outbreak(index) else { continue };
        let pid = o.pathogen;
        let (active, regions) = (o.active, regions_with_cases(store, world, index, state.first_index));
        let name = state.name(pid).to_string();
        let strain = state.pathogen(pid).is_some_and(|p| p.is_strain());
        let subject = o.index_case;
        events.push(Event {
            year: time.year(),
            day: time.day_of_year(),
            hour: time.hour(),
            kind: EventKind::Epidemic,
            species: None,
            subject: Some(subject),
            text: if strain {
                format!("A new strain is epidemic: {} — {} sick across {} regions", name, active, regions)
            } else {
                format!("{} is epidemic: {} sick across {} regions", name, active, regions)
            },
            pos: None,
            detail: String::new(),
        });
        alerts.push(Alert::Epidemic { event_index: events.total(), pathogen: pid, outbreak: index });
    }
    for index in ended {
        let Some(o) = state.outbreak(index) else { continue };
        let pid = o.pathogen;
        let (days, dead, recovered, epidemic) = (o.duration_days(day), o.deaths, o.recovered, o.epidemic);
        let name = state.name(pid).to_string();
        let slot = pid.0 as usize;
        let strain = state.pathogens.get(slot).is_some_and(|p| p.is_strain());
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

    // 3. Emergence (roster pathogens only).
    for slot in 0..n {
        let pid = PathogenId(slot as u8);
        let (is_strain, active) = (state.pathogens[slot].is_strain(), state.stats[slot].active);
        if is_strain || active > 0 {
            continue;
        }
        let last = state.last_case_day[slot];
        if last != u32::MAX && day.saturating_sub(last) < dp.reservoir_days {
            continue;
        }
        let hosts = state.hosts_living(pid, population);
        if hosts < dp.emergence_host_min {
            continue;
        }
        let hazard = dp.emergence_per_day * hosts as f32 / dp.emergence_host_ref.max(1) as f32;
        if !rng.chance(hazard.clamp(0.0, 1.0)) {
            continue;
        }
        emerge(store, world, events, time, dp, state, pid, resist);
    }
    alerts
}

/// Regions holding at least one active case of `outbreak`.
fn regions_with_cases(store: &CreatureStore, world: &World, outbreak: u16, _first: u16) -> usize {
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
    let day = time.day_index() as u32;
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
        origin_region: region as u8,
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
    let slot = pid.0 as usize;
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

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::sim::creatures::place_founders;
    use crate::sim::params::{CreaturesParams, GeneticsParams, Params, WorldParams};
    use crate::sim::stats::census;
    use crate::sim::Sim;

    fn world() -> World {
        World::generate(7, &WorldParams::default())
    }

    fn store() -> CreatureStore {
        let mut s = CreatureStore::new();
        for c in place_founders(&world(), &CreaturesParams::default(), &GeneticsParams::default(), 0.20, &mut Rng::new(1)) {
            s.insert(c);
        }
        s
    }

    fn day(d: u32) -> Time {
        let mut t = Time::new(6, 90, 24, 6, 20);
        for _ in 0..d * 24 {
            t.advance();
        }
        t
    }

    fn first_of(store: &CreatureStore, s: SpeciesId) -> CreatureId {
        store.living().find(|c| c.species == s).map(|c| c.id).unwrap()
    }

    #[test]
    fn effects_table() {
        // Fixed weights so the expectations do not follow the balance table.
        let mut dp = DiseaseParams::default();
        dp.resist_hunger_cost = 0.25;
        let mut st = store();
        let id = first_of(&st, SpeciesId::Vole);
        let c = st.get_mut(id).unwrap();
        c.genome.0[8] = 0.5;
        c.parasite_load = 0.0;
        let e = effects(c, &dp);
        assert!((e.hunger_factor - 1.125).abs() < 1e-4, "resistance cost only: {}", e.hunger_factor);
        assert_eq!(e.speed_factor, 1.0);
        assert!(e.can_mate);
        c.infection = Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 0, ends_day: 9, severity: 0.8, source: None, outbreak: 0 });
        let e = effects(c, &dp);
        assert!((e.speed_factor - 0.6).abs() < 1e-5);
        assert!(!e.can_mate);
        assert!((e.kill_bonus - 0.2).abs() < 1e-5);
        assert!((e.hunger_factor - 1.125 * 1.3).abs() < 1e-4);
        c.parasite_load = 0.5;
        let e = effects(c, &dp);
        assert!((e.fertility_factor - 0.75).abs() < 1e-5);
    }

    #[test]
    fn contact_probability_formula() {
        // Two voles adjacent; the infectious one rolls on the other every tick.
        let mut dp = DiseaseParams::default();
        dp.pathogens[0].transmissibility = 0.02;
        dp.susceptibility_w = 0.8;
        let w = world();
        let state = DiseaseState::new(&dp);
        let mut hits = 0u32;
        let n = 10_000;
        let mut rng = Rng::new(3);
        let mut st = store();
        let a = first_of(&st, SpeciesId::Vole);
        let b = st.living().filter(|c| c.species == SpeciesId::Vole && c.id != a).map(|c| c.id).next().unwrap();
        let (ax, ay) = {
            let c = st.get(a).unwrap();
            (c.x, c.y)
        };
        // Move b next to a, on a non-den cell, and fix b's resistance.
        let (bx, by) = crate::sim::behavior::find_walkable_near(ax, ay, &w).unwrap();
        {
            let cb = st.get_mut(b).unwrap();
            cb.x = bx;
            cb.y = by;
            cb.genome.0[8] = 0.30;
        }
        let expect = 0.02 * (1.0 - 0.8 * 0.30);
        for _ in 0..n {
            for c in st.living_mut() {
                c.infection = None;
            }
            st.get_mut(a).unwrap().infection = Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 0, ends_day: 9, severity: 0.8, source: None, outbreak: 0 });
            let mut idx = SpatialIndex::new(&w);
            idx.rebuild(&st, &w);
            contagion_pass(&mut st, &idx, &w, &day(1), &dp, &state, &mut rng);
            if st.get(b).unwrap().infection.is_some() {
                hits += 1;
            }
        }
        let got = hits as f32 / n as f32;
        // Other voles nearby may also be infected by `a`, but b's roll is independent.
        assert!((got - expect).abs() < expect * 0.15, "got {got} want ≈ {expect}");
        let mut idx = SpatialIndex::new(&w);
        idx.rebuild(&st, &w);
        let _ = geom::cheb(ax, ay, bx, by);
    }

    #[test]
    fn immune_and_non_host_never_infected() {
        let dp = DiseaseParams::default();
        let w = world();
        let state = DiseaseState::new(&dp);
        let mut st = store();
        let a = first_of(&st, SpeciesId::Vole);
        let (ax, ay) = {
            let c = st.get(a).unwrap();
            (c.x, c.y)
        };
        let fox = first_of(&st, SpeciesId::Fox);
        let b = st.living().filter(|c| c.species == SpeciesId::Vole && c.id != a).map(|c| c.id).next().unwrap();
        let (bx, by) = crate::sim::behavior::find_walkable_near(ax, ay, &w).unwrap();
        for id in [fox, b] {
            let c = st.get_mut(id).unwrap();
            c.x = bx;
            c.y = by;
        }
        st.get_mut(b).unwrap().immune_until[0] = u32::MAX;
        let mut rng = Rng::new(5);
        for _ in 0..2000 {
            st.get_mut(a).unwrap().infection = Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 0, ends_day: 9, severity: 0.8, source: None, outbreak: 0 });
            let mut idx = SpatialIndex::new(&w);
            idx.rebuild(&st, &w);
            contagion_pass(&mut st, &idx, &w, &day(1), &dp, &state, &mut rng);
            assert!(st.get(fox).unwrap().infection.is_none(), "fox is not a Greyfever host");
            assert!(st.get(b).unwrap().infection.is_none(), "immune vole");
            for c in st.living_mut() {
                if c.id != a {
                    c.infection = None;
                }
            }
        }
    }

    #[test]
    fn incubation_lethality_recovery() {
        let mut dp = DiseaseParams::default();
        dp.pathogens[0].lethality_per_day = 0.0;
        let mut w = world();
        let mut state = DiseaseState::new(&dp);
        let mut st = store();
        let mut events = EventRing::new(100);
        let mut tallies = DeathTallies::default();
        let mut lineage = Lineage::new();
        let mut rng = Rng::new(1);
        let a = first_of(&st, SpeciesId::Vole);
        st.get_mut(a).unwrap().genome.0[8] = 0.5;
        st.get_mut(a).unwrap().infection = Some(Infection { pathogen: PathogenId(0), stage: Stage::Incubating, since_day: 0, ends_day: 3, severity: 0.8, source: None, outbreak: 0 });
        progress_daily(&mut st, &mut w, &mut events, &day(2), &dp, &mut state, &mut tallies, &mut lineage, &mut rng);
        assert_eq!(st.get(a).unwrap().infection.unwrap().stage, Stage::Incubating);
        progress_daily(&mut st, &mut w, &mut events, &day(3), &dp, &mut state, &mut tallies, &mut lineage, &mut rng);
        let inf = st.get(a).unwrap().infection.unwrap();
        assert_eq!(inf.stage, Stage::Infectious);
        // 10 × (1 − 0.4 × 0.5) = 8 days.
        assert_eq!(inf.ends_day, 3 + 8);
        assert_eq!(state.stats[0].total_cases, 1);
        for d in 4..11 {
            progress_daily(&mut st, &mut w, &mut events, &day(d), &dp, &mut state, &mut tallies, &mut lineage, &mut rng);
            assert!(st.get(a).unwrap().alive);
        }
        progress_daily(&mut st, &mut w, &mut events, &day(11), &dp, &mut state, &mut tallies, &mut lineage, &mut rng);
        let c = st.get(a).unwrap();
        assert!(c.infection.is_none(), "recovered");
        assert_eq!(c.immune_until[0], 11 + 360);
        assert_eq!(c.infections_survived, 1);
        assert!(events.iter().any(|e| e.kind == EventKind::Recovery), "severity 0.8 ≥ 0.7 emits Recovery");

        // Lethality: hazard 1.0 kills on the first infectious day (the runtime
        // pathogen list is what `progress_daily` reads, not the params roster).
        state.pathogens[0].params.lethality_per_day = 1.0;
        let b = first_of(&st, SpeciesId::Hare);
        st.get_mut(b).unwrap().genome.0[8] = 0.0;
        st.get_mut(b).unwrap().infection = Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 11, ends_day: 21, severity: 0.8, source: None, outbreak: 0 });
        progress_daily(&mut st, &mut w, &mut events, &day(12), &dp, &mut state, &mut tallies, &mut lineage, &mut rng);
        let c = st.get(b).unwrap();
        assert!(!c.alive);
        assert_eq!(c.death.unwrap().cause, Cause::Disease);
        assert_eq!(c.died_infected, Some(PathogenId(0)));
        assert_eq!(tallies.disease, 1);
        assert!(events.iter().any(|e| e.kind == EventKind::DeathDisease && e.text.contains("Greyfever")));
    }

    #[test]
    fn lethality_by_resistance() {
        let dp = DiseaseParams::default();
        // hazard = 0.06 × host × (1 − 1.4 r), clamped at 0: r = 0.3 → 0.0348/day,
        // r = 0.9 → 0 (fully resistant).
        let p = &dp.pathogens[0];
        let h0 = (p.lethality_per_day * 1.0 * (1.0 - dp.lethality_resist_w * 0.0)).clamp(0.0, 1.0);
        let h3 = (p.lethality_per_day * 1.0 * (1.0 - dp.lethality_resist_w * 0.3)).clamp(0.0, 1.0);
        let h9 = (p.lethality_per_day * 1.0 * (1.0 - dp.lethality_resist_w * 0.9)).clamp(0.0, 1.0);
        assert!((h0 - 0.06).abs() < 1e-6);
        assert!((h3 - 0.0348).abs() < 1e-5);
        assert_eq!(h9, 0.0);
        assert_eq!(infectious_days(p, 0.0, &dp), 10);
        assert_eq!(infectious_days(p, 0.98, &dp), 6);
    }

    #[test]
    fn vertical_transmission_and_birth_load() {
        let mut dp = DiseaseParams::default();
        dp.vertical_transmission = 1.0;
        let state = DiseaseState::new(&dp);
        let st = store();
        let a = first_of(&st, SpeciesId::Vole);
        let mut mother = st.get(a).unwrap().clone();
        mother.parasite_load = 0.6;
        mother.infection = Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 0, ends_day: 9, severity: 0.8, source: None, outbreak: 3 });
        let mut child = mother.clone();
        child.infection = None;
        child.parasite_load = 0.0;
        at_birth(&mut child, &mother, &day(1), &dp, &state, &mut Rng::new(1));
        assert!((child.parasite_load - 0.18f32.max(dp.parasite_baseline)).abs() < 1e-5);
        let inf = child.infection.unwrap();
        assert_eq!(inf.stage, Stage::Incubating);
        assert_eq!(inf.outbreak, 3);
        assert_eq!(inf.source, Some(mother.id));
    }

    #[test]
    fn carcass_transmission_and_parasite_transfer() {
        let mut dp = DiseaseParams::default();
        dp.carcass_transmission = 1.0;
        let w = world();
        let mut state = DiseaseState::new(&dp);
        let mut st = store();
        let mut events = EventRing::new(10);
        let mut rng = Rng::new(2);
        let fox = first_of(&st, SpeciesId::Fox);
        let vole = first_of(&st, SpeciesId::Vole);
        let hare = first_of(&st, SpeciesId::Hare);
        {
            let k = st.get_mut(vole).unwrap();
            k.alive = false;
            k.parasite_load = 0.8;
            k.died_infected = Some(PathogenId(0)); // Greyfever: fox is not a host
        }
        dp.spillover_chance = 0.0;
        on_eat(&mut st, fox, vole, &w, &mut events, &day(1), &dp, &mut state, &mut rng);
        let f = st.get(fox).unwrap();
        assert!((f.parasite_load - 0.4).abs() < 1e-5, "trophic transfer ≥ 0.35: {}", f.parasite_load);
        assert!(f.infection.is_none(), "non-host, spillover disabled");
        // A host eater (hare scavenging is not a thing, but the rule is general).
        st.get_mut(hare).unwrap().genome.0[8] = 0.0;
        on_eat(&mut st, hare, vole, &w, &mut events, &day(1), &dp, &mut state, &mut rng);
        assert!(st.get(hare).unwrap().infection.is_some(), "host eater is infected at transmission 1.0");
    }

    #[test]
    fn spillover_creates_strain_and_index_case() {
        let mut dp = DiseaseParams::default();
        dp.spillover_chance = 1.0;
        let w = world();
        let mut state = DiseaseState::new(&dp);
        let mut st = store();
        let mut events = EventRing::new(10);
        let mut rng = Rng::new(2);
        let fox = first_of(&st, SpeciesId::Fox);
        let vole = first_of(&st, SpeciesId::Vole);
        {
            let k = st.get_mut(vole).unwrap();
            k.alive = false;
            k.died_infected = Some(PathogenId(0));
        }
        on_eat(&mut st, fox, vole, &w, &mut events, &day(5), &dp, &mut state, &mut rng);
        assert_eq!(state.pathogens.len(), 4);
        let strain = &state.pathogens[3];
        assert_eq!(strain.name(), "Greyfever (foxes strain)");
        assert_eq!(strain.parent, Some(PathogenId(0)));
        assert_eq!(strain.host(SpeciesId::Fox), 1.0);
        assert_eq!(strain.host(SpeciesId::Vole), 0.0);
        let f = st.get(fox).unwrap();
        let inf = f.infection.unwrap();
        assert_eq!(inf.pathogen, PathogenId(3));
        assert_eq!(inf.stage, Stage::Infectious);
        assert_eq!(state.outbreaks.len(), 1);
        assert_eq!(state.outbreaks[0].index_case, fox);
        assert!(events.iter().any(|e| e.kind == EventKind::Spillover && e.text.contains("jumped to the foxes")));
        assert_eq!(state.root(PathogenId(3)), PathogenId(0));
    }

    #[test]
    fn spillover_slot_reuse_zeroes_immunity() {
        let mut dp = DiseaseParams::default();
        dp.spillover_chance = 1.0;
        dp.max_pathogens = 4;
        dp.reservoir_days = 10;
        let w = world();
        let mut state = DiseaseState::new(&dp);
        let mut st = store();
        let mut events = EventRing::new(10);
        let mut rng = Rng::new(2);
        let vole = first_of(&st, SpeciesId::Vole);
        st.get_mut(vole).unwrap().alive = false;
        st.get_mut(vole).unwrap().died_infected = Some(PathogenId(0));
        let foxes: Vec<CreatureId> = st.living().filter(|c| c.species == SpeciesId::Fox).map(|c| c.id).take(2).collect();
        on_eat(&mut st, foxes[0], vole, &w, &mut events, &day(5), &dp, &mut state, &mut rng);
        assert_eq!(state.pathogens.len(), 4);
        // Second spillover: no free slot, strain not extinct → fails.
        on_eat(&mut st, foxes[1], vole, &w, &mut events, &day(6), &dp, &mut state, &mut rng);
        assert_eq!(state.failed_spillovers, 1);
        // Mark the strain extinct and past its reservoir; give a fox immunity to it.
        state.pathogens[3].extinct = true;
        state.last_case_day[3] = 0;
        st.get_mut(foxes[1]).unwrap().immune_until[3] = u32::MAX;
        st.get_mut(foxes[1]).unwrap().infection = None;
        on_eat(&mut st, foxes[1], vole, &w, &mut events, &day(40), &dp, &mut state, &mut rng);
        assert_eq!(state.pathogens.len(), 4);
        assert_eq!(state.pathogens[3].born_day, Some(40));
        assert!(st.get(foxes[1]).unwrap().infection.is_some(), "immunity to the old strain was cleared");
    }

    #[test]
    fn emergence_needs_min_hosts_and_reservoir() {
        let mut dp = DiseaseParams::default();
        dp.emergence_per_day = 1.0;
        dp.emergence_host_ref = 1;
        let w = world();
        let mut state = DiseaseState::new(&dp);
        let mut st = store();
        let mut events = EventRing::new(10);
        let mut rng = Rng::new(9);
        let pop = census(&st).population;
        // Enough hosts: Greyfever emerges on day 1.
        let alerts = daily_update(&mut st, &w, &mut events, &day(1), &dp, &mut state, &pop, &mut rng);
        assert!(alerts.is_empty());
        assert_eq!(state.stats[0].outbreaks, 1);
        assert!(events.iter().any(|e| e.kind == EventKind::Outbreak && e.text.starts_with("Greyfever breaks out")));
        let o = &state.outbreaks[0];
        let idx = st.get(o.index_case).unwrap();
        assert_eq!(idx.infection.unwrap().stage, Stage::Infectious);
        // The index case is the lowest-resistance host of the densest region.
        let region = o.origin_region as usize;
        let min_r = st
            .living()
            .filter(|c| c.species.kind() == crate::sim::species::Kind::Prey && w.region_index(c.x, c.y).min(7) == region)
            .map(|c| c.genome.resistance())
            .fold(f32::INFINITY, f32::min);
        assert!((idx.genome.resistance() - min_r).abs() < 1e-6);
        // Reservoir: clear the case, no re-emergence within reservoir_days.
        st.get_mut(o.index_case).unwrap().infection = None;
        let _ = daily_update(&mut st, &w, &mut events, &day(2), &dp, &mut state, &pop, &mut rng);
        assert_eq!(state.stats[0].outbreaks, 1, "reservoir cooldown holds");
        assert!(state.outbreaks[0].ended_day.is_some());
        let _ = daily_update(&mut st, &w, &mut events, &day(200), &dp, &mut state, &pop, &mut rng);
        assert_eq!(state.stats[0].outbreaks, 2, "re-emerges after the reservoir");
        // Too few hosts: no emergence.
        let few = [10u32, 10, 10, 0, 0, 0];
        for c in st.living_mut() {
            c.infection = None;
        }
        state.last_case_day = [u32::MAX; MAX_PATHOGENS];
        let mut state2 = DiseaseState::new(&dp);
        let _ = daily_update(&mut st, &w, &mut events, &day(300), &dp, &mut state2, &few, &mut rng);
        assert_eq!(state2.stats[0].outbreaks, 0);
    }

    #[test]
    fn epidemic_threshold_once_and_outbreak_ends() {
        let mut dp = DiseaseParams::default();
        dp.emergence_per_day = 0.0;
        dp.epidemic_min_cases = 3;
        dp.epidemic_share = 0.001;
        let w = world();
        let mut state = DiseaseState::new(&dp);
        let mut st = store();
        let mut events = EventRing::new(10);
        let mut rng = Rng::new(9);
        let pop = census(&st).population;
        let voles: Vec<CreatureId> = st.living().filter(|c| c.species == SpeciesId::Vole).map(|c| c.id).take(3).collect();
        let index = state.push_outbreak(Outbreak {
            pathogen: PathogenId(0),
            started_day: 0,
            ended_day: None,
            origin_region: 0,
            index_case: voles[0],
            cases: 3,
            deaths: 0,
            recovered: 0,
            peak_active: 1,
            peak_day: 0,
            species_cases: [3, 0, 0, 0, 0, 0],
            species_deaths: [0; 6],
            epidemic: false,
            resist_at_start: [0.0; 6],
            resist_at_end: [0.0; 6],
            active: 0,
            cases_today: 0,
        });
        for id in &voles {
            st.get_mut(*id).unwrap().infection = Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 0, ends_day: 9, severity: 0.8, source: None, outbreak: index });
        }
        let alerts = daily_update(&mut st, &w, &mut events, &day(1), &dp, &mut state, &pop, &mut rng);
        assert_eq!(alerts.len(), 1);
        assert!(matches!(alerts[0], Alert::Epidemic { pathogen: PathogenId(0), .. }));
        let alerts = daily_update(&mut st, &w, &mut events, &day(2), &dp, &mut state, &pop, &mut rng);
        assert!(alerts.is_empty(), "epidemic fires once per outbreak");
        for id in &voles {
            st.get_mut(*id).unwrap().infection = None;
        }
        let _ = daily_update(&mut st, &w, &mut events, &day(3), &dp, &mut state, &pop, &mut rng);
        assert_eq!(state.outbreaks[0].ended_day, Some(3));
        assert!(events.iter().any(|e| e.kind == EventKind::EpidemicOver));
    }

    #[test]
    fn parasite_uptake_shed_clear() {
        let dp = DiseaseParams::default();
        let mut w = world();
        let mut st = store();
        let a = first_of(&st, SpeciesId::Deer);
        let (x, y) = {
            let c = st.get(a).unwrap();
            (c.x, c.y)
        };
        w.cell_mut(x, y).parasite_load = 0.5;
        {
            let c = st.get_mut(a).unwrap();
            c.genome.0[8] = 0.5;
            parasite_uptake(c, &w, &dp);
            assert!((c.parasite_load - dp.parasite_uptake * 0.5 * 0.5).abs() < 1e-6);
            c.parasite_load = 0.5;
            parasite_shed(c, &mut w, &dp);
        }
        let after_shed = 0.5 + dp.parasite_shed * 0.5;
        assert!((w.cell(x, y).parasite_load - after_shed).abs() < 1e-6);
        w.cell_mut(x, y).prey_pressure = 0.0;
        w.cell_mut(x, y).pred_pressure = 0.0;
        decay_cells(&mut w, &dp);
        assert!((w.cell(x, y).parasite_load - after_shed * dp.parasite_cell_decay).abs() < 1e-5);
    }

    #[test]
    fn disabled_is_inert() {
        let mut p = Params::default();
        p.disease.enabled = false;
        let mut sim = Sim::new(3, p);
        for _ in 0..24 * 60 {
            sim.step();
        }
        assert!(sim.creatures.living().all(|c| c.infection.is_none() && c.parasite_load == 0.0));
        assert!(sim.disease.outbreaks.is_empty());
    }
}
