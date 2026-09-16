//! Pure, deterministic simulation core. No `ratatui` types may appear anywhere
//! under `src/sim` (enforced by a test); this module only reads/writes plain data.

pub mod behavior;
pub mod creatures;
pub mod disease;
pub mod ecology;
pub mod events;
pub mod genetics;
pub mod geom;
pub mod lineage;
pub mod params;
pub mod predation;
pub mod rng;
pub mod save;
pub mod spatial;
pub mod species;
pub mod stats;
pub mod time;
pub mod world;

pub use creatures::{Cause, Creature, CreatureId, Death, DeathTallies, Goal, Mutation, RestReason, Sex};
pub use disease::{DiseaseState, Infection, Outbreak, Pathogen, PathogenId, PathogenStats, Stage};
pub use events::{Event, EventKind};
pub use geom::{cheb, dist};
pub use params::{Params, PredationParams, Rainfall, Roster, SpeciesParams};
pub use params::{Difficulty, Preset, PRESETS};
pub use rng::Rng;
pub use spatial::SpatialIndex;
pub use species::{Genome, Kind, SpeciesId, TRAIT_NAMES};
pub use lineage::{Lineage, LineageNode, Tree, TreeItem};
pub use params::GeneticsParams;
pub use stats::{census, group_census, Census, GroupCensus, Sample, Series, SpeciesStats};
pub use time::{Season, Time};
pub use world::{Cell, RegionRect, Terrain, World};


use creatures::CreatureStore;
use events::EventRing;
use serde::{Deserialize, Serialize};

/// A notification the UI should surface (e.g. an extinction modal). C5 FR8.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Alert {
    Extinction {
        /// Absolute (1-based) event sequence number of the `Extinction` event.
        event_index: u64,
        species: SpeciesId,
        last: CreatureId,
    },
    /// C7 FR9: an outbreak crossed the epidemic threshold.
    Epidemic {
        event_index: u64,
        pathogen: PathogenId,
        /// Absolute outbreak index (`Sim.disease.outbreak(index)`).
        outbreak: u16,
    },
}

pub use creatures::ExtinctionRecord;

#[derive(Debug)]
pub struct StepReport {
    pub alerts: Vec<Alert>,
}

/// Per-system wall-clock timing (C6 FR8/FR9), accumulated over a run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(clippy::struct_field_names)]
pub struct Profile {
    pub behavior_ns: u64,
    pub day_boundary_ns: u64,
    pub ecology_ns: u64,
    pub migration_ns: u64,
    pub spatial_ns: u64,
    /// Contagion pass + daily disease update (C7).
    pub disease_ns: u64,
    /// Whole step (everything, including the systems above).
    pub step_ns: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sim {
    /// The world seed this sim was generated from (C6 FR1 header; informational
    /// after generation since the rng state is serialised).
    pub seed: u64,
    pub params: Params,
    /// Ecology RNG (rain, regrowth, droughts) — unchanged from C2.
    pub rng: Rng,
    /// Creature RNG (placement, goals, dens), decoupled so creature behaviour
    /// does not perturb the ecology stream.
    pub creature_rng: Rng,
    pub time: Time,
    pub world: World,
    pub events: EventRing,
    pub series: Series,
    pub creatures: CreatureStore,
    /// Rebuilt on load; never serialised (C6 FR1).
    #[serde(skip)]
    pub spatial: SpatialIndex,
    pub deaths: DeathTallies,
    pub drought: [bool; 8],
    pub drought_days_below: [u32; 8],
    /// Per-species live statistics (C4 FR5), roster order.
    pub species: Vec<SpeciesStats>,
    /// Per-species group-size census (C8 follow-up): the herds and packs the
    /// cohesion rule currently holds together. Recomputed from positions at the
    /// day boundary and after a load, never serialised, so the file format and
    /// the checksum are untouched.
    #[serde(skip)]
    pub group_stats: GroupCensus,
    /// Ancestry of every creature born (C4 FR6), pruned weekly.
    pub lineage: Lineage,
    /// The soft-cap Note has been logged for the current crossing.
    pub soft_cap_noted: bool,
    /// How many times the soft-cap Note has been logged (acceptance: 0).
    pub soft_cap_crossings: u32,
    soft_cap_counted: bool,
    // ---- C5 predation / extinction / migration state ----
    /// A species has been marked globally extinct (once each).
    pub extinct: Vec<bool>,
    /// Retained death record of the last individual, per species.
    pub last_extinct: Vec<Option<ExtinctionRecord>>,
    /// Tick until which a (species × 8 + region) pair cannot migrate again.
    pub migration_cooldown_until: Vec<u64>,
    /// Consecutive days the migration trigger has held, per (species, region).
    pub migration_days_below: Vec<u32>,
    /// Last day a (species, region) count was ≥ `local_extinction_min`.
    pub local_min_day: Vec<Option<u32>>,
    /// A local-extinction Note has been emitted for this pair (until repopulated).
    pub local_noted: Vec<bool>,
    /// Per-system timings (C6 FR8); never serialised.
    #[serde(skip)]
    pub profile: Profile,
    /// Whether `--profile` timing is being accumulated.
    #[serde(skip)]
    pub profile_enabled: bool,
    // ---- C7 disease ----
    /// Disease RNG, a third stream so enabling disease leaves the ecology and
    /// creature streams untouched.
    pub disease_rng: Rng,
    pub disease: DiseaseState,
}

impl Sim {
    pub fn new(seed: u64, params: Params) -> Self {
        let world = World::generate(seed, &params.world);
        let time = Time::new(
            params.time.start_hour,
            params.time.season_days,
            params.time.ticks_per_day,
            params.time.sunrise_hour,
            params.time.sunset_hour,
        );
        let events = EventRing::new(params.events.capacity);
        let series = Series::new(params.stats.series_days);
        let rng = Rng::new(seed);
        let mut creature_rng = Rng::new(seed ^ 0x9E37_79B9_7F4A_7C15);
        let disease_rng = Rng::new(seed ^ 0x7F4A_7C15_9E37_79B9);
        let disease = DiseaseState::new(&params.disease, &params.species);
        let n = params.species.len();

        // Place founders (FR3), then build the spatial index over them.
        let mut creatures = CreatureStore::new();
        for mut c in creatures::place_founders(&world, &params.species, &params.creatures, &params.genetics, params.disease.resistance_founder_sd, &mut creature_rng) {
            if params.disease.enabled {
                c.parasite_load = params.disease.parasite_baseline;
            }
            creatures.insert(c);
        }
        let mut spatial = SpatialIndex::new(&world);
        spatial.rebuild(&creatures, &world);
        let mut lineage = Lineage::new();
        for c in creatures.living() {
            lineage.record(c, &params.species, params.genetics.mutation_notable);
        }
        let species = SpeciesStats::all(&census(&creatures, n), &params.species, 0, params.genetics.drift_every_generations);
        let group_stats = group_census(&creatures, &params.social, n);

        Self {
            seed,
            params,
            rng,
            creature_rng,
            time,
            world,
            events,
            series,
            creatures,
            spatial,
            deaths: DeathTallies::new(n),
            drought: [false; 8],
            drought_days_below: [0; 8],
            species,
            group_stats,
            lineage,
            soft_cap_noted: false,
            soft_cap_crossings: 0,
            soft_cap_counted: false,
            extinct: vec![false; n],
            last_extinct: vec![None; n],
            migration_cooldown_until: vec![0; n * 8],
            migration_days_below: vec![0; n * 8],
            local_min_day: vec![None; n * 8],
            local_noted: vec![false; n * 8],
            profile: Profile::default(),
            profile_enabled: false,
            disease_rng,
            disease,
        }
    }

    /// Rebuild the spatial index from current creature positions (C6 FR1: the
    /// index is never serialised and must be rebuilt after a load).
    pub fn rebuild_spatial(&mut self) {
        self.spatial.rebuild(&self.creatures, &self.world);
    }

    /// Recompute the group-size census from the living set's current positions
    /// (C8 follow-up). Draws no RNG, so it never perturbs a run.
    pub fn refresh_group_stats(&mut self) {
        self.group_stats = group_census(&self.creatures, &self.params.social, self.params.species.len());
    }

    /// The species roster (`[[species]]`).
    pub const fn roster(&self) -> &Roster {
        &self.params.species
    }

    /// The roster record of a species.
    pub fn species_params(&self, id: SpeciesId) -> &SpeciesParams {
        self.params.species.get(id)
    }

    /// Births so far today for species index `i` (the live counter).
    pub fn births_today(&self, i: usize) -> u32 {
        self.deaths.births[i]
    }

    /// Deaths so far today for species index `i` (the live counter).
    pub fn deaths_today(&self, i: usize) -> u32 {
        self.deaths.deaths[i]
    }

    /// The oldest living creature (lowest `born_day`, ties by id), if any.
    pub fn oldest_living(&self) -> Option<CreatureId> {
        self.creatures.living().min_by_key(|c| (c.born_day, c.id)).map(|c| c.id)
    }

    /// Advance one tick: season event, creature behaviour, then the daily update
    /// at midnight. Order is fixed for determinism (FR9). Returns the alerts
    /// raised this tick (extinction, C5 FR8).
    pub fn step(&mut self) -> StepReport {
        let step_start = std::time::Instant::now();
        let mut alerts: Vec<Alert> = Vec::new();
        // The initial Spring is announced at tick 0 (Year 1, Day 1, 06:00).
        if self.time.tick == 0 {
            self.push_season_event(Season::Spring);
        }
        if let Some(season) = self.time.advance() {
            self.push_season_event(season);
        }
        self.run_behavior();
        if self.time.hour() == 0 {
            self.run_midnight(&mut alerts);
        }
        let t0 = std::time::Instant::now();
        self.spatial.rebuild(&self.creatures, &self.world);
        if self.profile_enabled {
            self.profile.spatial_ns += crate::cast!(t0.elapsed().as_nanos() => u64);
            self.profile.step_ns += crate::cast!(step_start.elapsed().as_nanos() => u64);
        }
        StepReport { alerts }
    }

    /// Per-tick creature behaviour, plus soft-cap crossing bookkeeping.
    fn run_behavior(&mut self) {
        // spatial snapshot; it is rebuilt below for the next tick and the UI).
        let t0 = std::time::Instant::now();
        behavior::tick_creatures(
            &mut self.creatures,
            &self.spatial,
            &mut self.world,
            &mut self.events,
            &self.time,
            &self.params.species,
            &self.params.creatures,
            &self.params.ecology,
            &self.params.genetics,
            &self.params.predation,
            &self.params.disease,
            &self.params.social,
            &mut self.creature_rng,
            &mut self.deaths,
            &mut self.lineage,
            &mut self.soft_cap_noted,
            &mut self.disease,
            &mut self.disease_rng,
        );
        if self.profile_enabled {
            self.profile.behavior_ns += crate::cast!(t0.elapsed().as_nanos() => u64);
        }
        if self.soft_cap_noted {
            if !self.soft_cap_counted {
                self.soft_cap_crossings += 1;
                self.soft_cap_counted = true;
            }
            if (crate::cast!(self.creatures.len_living() => u32)) < self.params.genetics.max_population_soft_cap {
                self.soft_cap_noted = false; // re-arm for the next crossing
                self.soft_cap_counted = false;
            }
        }
    }

    /// Midnight: the day boundary, then the daily subsystems.
    fn run_midnight(&mut self, alerts: &mut Vec<Alert>) {
        let c = self.day_boundary_update();
        self.midnight_systems(&c, alerts);
        // Refresh last: the midnight disease pass can still kill after the day
        // boundary, and the census must describe the set the tick ends with.
        self.refresh_group_stats();
    }

    /// Midnight: age/behaviour day boundary, census and species statistics.
    fn day_boundary_update(&mut self) -> Census {
        let t0 = std::time::Instant::now();
        behavior::day_boundary(
            &mut self.creatures,
            &mut self.world,
            &mut self.events,
            &self.time,
            &self.params.species,
            &self.params.creatures,
            &self.params.genetics,
            &self.params.disease,
            &mut self.deaths,
            &mut self.lineage,
            &mut self.disease,
            &mut self.disease_rng,
        );
        let c = census(&self.creatures, self.params.species.len());
        let day = crate::cast!(self.time.day_index() => u32);
        stats::update_species_daily(&mut self.species, &c, &self.deaths, day, self.params.genetics.drift_every_generations);
        if self.profile_enabled {
            self.profile.day_boundary_ns += crate::cast!(t0.elapsed().as_nanos() => u64);
        }
        c
    }

    /// Midnight: disease, ecology, migration and extinction detection.
    fn midnight_systems(&mut self, c: &Census, alerts: &mut Vec<Alert>) {
        self.disease_step(c, alerts);
        self.ecology_step(c);
        self.deaths = self.deaths.next_day();
        let day = crate::cast!(self.time.day_index() => u32);
        if day % 7 == 0 {
            self.lineage.prune(&c.max_generation, self.params.genetics.lineage_keep_generations, &self.creatures);
        }
        // C5 FR7: migration evaluation, then FR8 extinction detection.
        let t0 = std::time::Instant::now();
        behavior::migration_daily(
            &mut self.creatures,
            &self.world,
            &mut self.events,
            &self.time,
            &self.params.species,
            &self.params.ecology,
            &self.params.predation,
            &mut self.migration_cooldown_until,
            &mut self.migration_days_below,
        );
        self.detect_extinctions(alerts);
        if self.profile_enabled {
            self.profile.migration_ns += crate::cast!(t0.elapsed().as_nanos() => u64);
        }
    }

    /// C7 FR8: outbreak tracking, epidemic alerts and emergence, after the census.
    fn disease_step(&mut self, c: &Census, alerts: &mut Vec<Alert>) {
        let t0 = std::time::Instant::now();
        alerts.extend(disease::daily_update(
            &mut self.creatures,
            &self.world,
            &mut self.events,
            &self.time,
            &self.params.species,
            &self.params.disease,
            &mut self.disease,
            &c.population,
            &mut self.disease_rng,
        ));
        if self.profile_enabled {
            self.profile.disease_ns += crate::cast!(t0.elapsed().as_nanos() => u64);
        }
    }

    /// The midnight ecology update and its series sample.
    fn ecology_step(&mut self, c: &Census) {
        let t0 = std::time::Instant::now();
        ecology::daily_update(
            &mut self.world,
            &mut self.rng,
            &self.time,
            &mut self.events,
            &mut self.series,
            &mut self.drought,
            &mut self.drought_days_below,
            &self.params.ecology,
            self.params.world.rainfall,
            c,
            &self.deaths,
            &self.disease,
        );
        if self.profile_enabled {
            self.profile.ecology_ns += crate::cast!(t0.elapsed().as_nanos() => u64);
        }
    }

    /// C5 FR8: mark species `id` globally extinct and queue its alert. A no-op
    /// unless the species was introduced and its population just reached zero.
    fn record_global_extinction(&mut self, i: usize, id: SpeciesId, alerts: &mut Vec<Alert>) {
        let initial = self.params.species.get(id).initial_count;
        let peak = self.species[i].peak;
        if initial == 0 || peak == 0 || self.species[i].count != 0 {
            return;
        }
        self.extinct[i] = true;
        // The last individual is the species' most recent death, recorded by
        // `behavior::kill` (survives carcass freeing).
        let record = self.deaths.last_death[i].clone();
        let last_id = record.as_ref().map(|r| r.last);
        let text = match &record {
            Some(r) => format!(
                "The {} are extinct; the last individual was {} {} ({} in {})",
                self.params.species.plural(id),
                r.name,
                r.tag,
                r.cause.label(),
                r.region
            ),
            None => format!("The {} are extinct", self.params.species.plural(id)),
        };
        let pos = record.as_ref().map(|r| r.pos);
        self.events.push(Event {
            year: self.time.year(),
            day: self.time.day_of_year(),
            hour: self.time.hour(),
            kind: EventKind::Extinction,
            species: Some(id),
            subject: last_id,
            text,
            pos,
            detail: String::new(),
        });
        let event_index = self.events.total();
        if let Some(r) = record {
            self.last_extinct[i] = Some(r);
        }
        if let Some(last) = last_id {
            alerts.push(Alert::Extinction { event_index, species: id, last });
        }
    }

    /// C5 FR8: mark globally extinct species and queue one `Extinction` alert each.
    fn detect_extinctions(&mut self, alerts: &mut Vec<Alert>) {
        let mut region_counts = vec![[0u32; 8]; self.params.species.len()];
        for c in self.creatures.living() {
            let ri = self.world.region_index(c.x, c.y).min(7);
            region_counts[c.species.index()][ri] += 1;
        }

        for i in 0..self.params.species.len() {
            let id = SpeciesId::from_index(i);
            // Global extinction (once per species).
            if !self.extinct[i] {
                self.record_global_extinction(i, id, alerts);
            }

            // Local extinction (FR8): a region whose count drops to 0 after being
            // ≥ `local_extinction_min` within the last season (90 days) emits a
            // Note once per (species, region) until repopulated.
            let today = crate::cast!(self.time.day_index() => u32);
            for ri in 0..8 {
                let key = i * 8 + ri;
                let count = region_counts[i][ri];
                if count >= self.params.predation.local_extinction_min {
                    self.local_min_day[key] = Some(today);
                }
                let recent_min = self.local_min_day[key].is_some_and(|d| today.saturating_sub(d) <= 90);
                if count > 0 {
                    self.local_noted[key] = false; // re-arm once repopulated
                } else if recent_min && !self.local_noted[key] {
                    self.local_noted[key] = true;
                    let r = &self.world.regions[ri];
                    self.events.push(Event {
                        year: self.time.year(),
                        day: self.time.day_of_year(),
                        hour: self.time.hour(),
                        kind: EventKind::Note,
                        species: Some(id),
                        subject: None,
                        text: format!("The {} line of {} is extinct", self.params.species.plural(id), r.0),
                        pos: Some(((r.1 + r.3).div_euclid(2), (r.2 + r.4).div_euclid(2))),
                        detail: String::new(),
                    });
                }
            }
        }
    }

    /// C5 FR8: the S12 alert for `species` was dismissed — release the retained
    /// death record of its last individual.
    pub fn dismiss_extinction(&mut self, species: SpeciesId) {
        self.last_extinct[species.index()] = None;
    }

    fn push_season_event(&mut self, season: Season) {
        self.events.push(Event {
            year: self.time.year(),
            day: self.time.day_of_year(),
            hour: self.time.hour(),
            kind: EventKind::Season,
            species: None,
            subject: None,
            text: season.event_text().to_string(),
            pos: None,
            detail: String::new(),
        });
    }

    /// FNV-1a 64 over, per cell in row-major order: terrain as u8, elevation,
    /// moisture, vegetation and `dried_from`; then tick, rng state, `events.len()`
    /// and the per-region drought flags.
    pub fn checksum(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let prime: u64 = 0x100_0000_01b3;
        let feed = |h: &mut u64, bytes: &[u8]| {
            for &b in bytes {
                *h ^= u64::from(b);
                *h = h.wrapping_mul(prime);
            }
        };
        for cell in &self.world.cells {
            feed(&mut h, &[crate::cast!(cell.terrain => u8)]);
            feed(&mut h, &cell.elevation.to_bits().to_le_bytes());
            feed(&mut h, &cell.moisture.to_bits().to_le_bytes());
            feed(&mut h, &cell.vegetation.to_bits().to_le_bytes());
            let dried = cell.dried_from.map_or(0, |t| crate::cast!(t => u8) + 1);
            feed(&mut h, &[dried]);
        }
        // Every living creature's id, x, y, hp, hunger, goal and C5 hunt/flee/wary
        // state (FR9).
        for c in self.creatures.living() {
            feed(&mut h, &c.id.0.to_le_bytes());
            feed(&mut h, &(crate::cast!(c.x => u64)).to_le_bytes());
            feed(&mut h, &(crate::cast!(c.y => u64)).to_le_bytes());
            feed(&mut h, &c.hp.to_bits().to_le_bytes());
            feed(&mut h, &c.hunger.to_bits().to_le_bytes());
            feed(&mut h, &[crate::cast!(c.goal => u8)]);
            feed(&mut h, &[crate::cast!(c.hunt_phase => u8)]);
            feed(&mut h, &c.hunt_target.map_or(u64::MAX, |t| u64::from(t.0)).to_le_bytes());
            feed(&mut h, &c.chase_start_tick.map_or([0xff; 8], u64::to_le_bytes));
            feed(&mut h, &c.hunt_cooldown_until.to_le_bytes());
            feed(&mut h, &c.eat_until.map_or([0xff; 8], u64::to_le_bytes));
            feed(&mut h, &c.scavenge_target.map_or(u64::MAX, |t| u64::from(t.0)).to_le_bytes());
            feed(&mut h, &c.flee_until.to_le_bytes());
            feed(&mut h, &c.threatened_by.map_or((u64::MAX, u64::MAX, u64::MAX), |(x, y, s)| (crate::cast!(x => u64), crate::cast!(y => u64), crate::cast!(s.index() => u64))).0.to_le_bytes());
            feed(&mut h, &c.threatened_by.map_or((u64::MAX, u64::MAX, u64::MAX), |(x, y, s)| (crate::cast!(x => u64), crate::cast!(y => u64), crate::cast!(s.index() => u64))).1.to_le_bytes());
            feed(&mut h, &c.threatened_by.map_or((u64::MAX, u64::MAX, u64::MAX), |(x, y, s)| (crate::cast!(x => u64), crate::cast!(y => u64), crate::cast!(s.index() => u64))).2.to_le_bytes());
            // C5 FR5b: the wary tier's own timer and away-vector.
            feed(&mut h, &c.wary_until.to_le_bytes());
            let wary = c.wary_by.map_or((u64::MAX, u64::MAX, u64::MAX), |(x, y, s)| (crate::cast!(x => u64), crate::cast!(y => u64), crate::cast!(s.index() => u64)));
            feed(&mut h, &wary.0.to_le_bytes());
            feed(&mut h, &wary.1.to_le_bytes());
            feed(&mut h, &wary.2.to_le_bytes());
            feed(&mut h, &c.migrate_until.to_le_bytes());
            feed(&mut h, &c.migrate_target.map_or((u64::MAX, u64::MAX), |(x, y)| (crate::cast!(x => u64), crate::cast!(y => u64))).0.to_le_bytes());
            feed(&mut h, &c.migrate_target.map_or((u64::MAX, u64::MAX), |(x, y)| (crate::cast!(x => u64), crate::cast!(y => u64))).1.to_le_bytes());
            // C7: infection state and parasite load.
            match c.infection {
                Some(i) => {
                    feed(&mut h, &[i.pathogen.0, crate::cast!(i.stage => u8)]);
                    feed(&mut h, &i.ends_day.to_le_bytes());
                }
                None => feed(&mut h, &[0xff, 0xff]),
            }
            feed(&mut h, &c.parasite_load.to_bits().to_le_bytes());
        }
        for &d in &self.disease.last_case_day {
            feed(&mut h, &d.to_le_bytes());
        }
        feed(&mut h, &(crate::cast!(self.disease.outbreaks.len() => u64)).to_le_bytes());
        feed(&mut h, &self.disease_rng.state().to_le_bytes());
        feed(&mut h, &self.time.tick.to_le_bytes());
        feed(&mut h, &self.rng.state().to_le_bytes());
        feed(&mut h, &self.creature_rng.state().to_le_bytes());
        feed(&mut h, &self.events.total().to_le_bytes());
        for &flagged in &self.drought {
            feed(&mut h, &[u8::from(flagged)]);
        }
        for &e in &self.extinct {
            feed(&mut h, &[u8::from(e)]);
        }
        h
    }
}

/// FNV-1a 64-bit hash over an arbitrary byte slice (test-only helper).
#[cfg(test)]
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::species::testing::{FOX, HARE, VOLE};

    #[test]
    fn fnv1a64_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn determinism_10k_ticks() {
        let mut a = Sim::new(42, Params::default());
        let mut b = Sim::new(42, Params::default());
        for _ in 0..10_000 {
            a.step();
            b.step();
        }
        assert_eq!(a.checksum(), b.checksum());
    }

    #[test]
    fn checksum_is_fnv_stable() {
        let mut a = Sim::new(7, Params::default());
        for _ in 0..8640 {
            a.step();
        }
        let mut b = Sim::new(7, Params::default());
        for _ in 0..8640 {
            b.step();
        }
        assert_eq!(a.checksum(), b.checksum());
        // Lock the exact value so accidental algorithm changes fail loudly.
        // Re-baselined for C8: the genome grew to eleven traits (Sociality and
        // Maturity), which shifts the founder jitter and every downstream draw.
        // Re-baselined for C5 FR5b: prey now spend wary ticks steering away from
        // predators that are not hunting them, so every trajectory downstream of
        // the first such encounter moves. Deliberate, not an accident.
        // Re-baselined for the erosion-based world generator: every cell's
        // terrain, elevation and moisture changed, so founders land elsewhere.
        // Re-baselined for climate physics: the wind and pole draws precede
        // the tectonic noise and orographic rain replaces the free rain
        // field, so every cell's moisture and terrain moved again.
        assert_eq!(a.checksum(), 0x4a21_08fc_4589_0590);
    }

    #[test]
    fn checksum_includes_creatures() {
        let a = Sim::new(42, Params::default());
        let mut b = Sim::new(42, Params::default());
        let id = b.creatures.living_ids()[0];
        b.creatures.get_mut(id).unwrap().x += 1;
        assert_ne!(a.checksum(), b.checksum(), "checksum must reflect creature state");
    }

    /// A sim with only the given species (and `n` founders), all killed on the
    /// first tick by zeroing hp, then stepped to the next midnight.
    fn extinction_sim(species: SpeciesId, n: u32) -> Sim {
        let mut p = Params::default();
        p.species.clear_initial_counts();
        p.species.get_mut(species).initial_count = n;
        let mut sim = Sim::new(7, p);
        for id in sim.creatures.living_ids() {
            let c = sim.creatures.get_mut(id).unwrap();
            c.hp = 0.0;
            c.hunger = 1.0; // so `needs` drives hp below zero instead of regenerating
            c.thirst = 0.0;
        }
        sim
    }

    /// Step until the first midnight (hour 0) and collect every alert.
    fn step_to_midnight(sim: &mut Sim) -> Vec<Alert> {
        let mut alerts = Vec::new();
        loop {
            let r = sim.step();
            alerts.extend(r.alerts);
            if sim.time.hour() == 0 {
                return alerts;
            }
        }
    }

    #[test]
    fn extinction_once_and_not_for_absent_species() {
        let mut sim = extinction_sim(HARE, 3);
        let alerts = step_to_midnight(&mut sim);
        assert!(alerts.iter().any(|a| matches!(a, Alert::Extinction { species: HARE, .. })), "hare extinction alert: {alerts:?}");
        // Fox has initial_count == 0 and must never emit.
        assert!(!alerts.iter().any(|a| matches!(a, Alert::Extinction { species: FOX, .. })));
        // A species emits at most once.
        let mut more = Vec::new();
        for _ in 0..200 {
            more.extend(sim.step().alerts);
        }
        let hare = alerts.into_iter().chain(more).filter(|a| matches!(a, Alert::Extinction { species: HARE, .. })).count();
        assert_eq!(hare, 1, "a species must emit at most once");
    }

    #[test]
    fn alert_queue_two_species_same_day() {
        let mut p = Params::default();
        p.species.clear_initial_counts();
        p.species.set_initial_count("vole", 2);
        p.species.set_initial_count("hare", 2);
        let mut sim = Sim::new(7, p);
        for id in sim.creatures.living_ids() {
            let c = sim.creatures.get_mut(id).unwrap();
            c.hp = 0.0;
            c.hunger = 1.0;
            c.thirst = 0.0;
        }
        let alerts = step_to_midnight(&mut sim);
        let species: Vec<SpeciesId> = alerts
            .iter()
            .filter_map(|a| match a {
                Alert::Extinction { species, .. } => Some(*species),
                Alert::Epidemic { .. } => None,
            })
            .collect();
        assert_eq!(species, vec![VOLE, HARE], "species-table order on the same day");
    }

    #[test]
    fn no_ratatui_in_sim() {
        for path in sim_sources() {
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(!text.contains("ratatui"), "{} must not reference ratatui", path.display());
        }
    }

    #[test]
    fn no_hashmap_in_sim() {
        for path in sim_sources() {
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(!text.contains("HashMap"), "{} uses HashMap", path.display());
            assert!(!text.contains("HashSet"), "{} uses HashSet", path.display());
        }
    }

    fn sim_sources() -> Vec<std::path::PathBuf> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sim");
        let mut out = Vec::new();
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            for entry in std::fs::read_dir(&d).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                // Skip this file: the test harness itself names "ratatui"/"HashMap".
                if path.extension().and_then(|e| e.to_str()) == Some("rs")
                    && path.file_name().and_then(|f| f.to_str()) != Some("mod.rs")
                {
                    out.push(path);
                }
            }
        }
        out.sort();
        out
    }
}
