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
pub use params::{Params, PredationParams, Rainfall};
pub use params::{Difficulty, Preset, PRESETS};
pub use rng::Rng;
pub use spatial::SpatialIndex;
pub use species::{Genome, Kind, SpeciesId, TRAIT_NAMES};
pub use lineage::{Lineage, LineageNode, Tree, TreeItem};
pub use params::GeneticsParams;
pub use stats::{census, Census, Sample, Series, SpeciesStats};
pub use time::{Season, Time};
pub use world::{Cell, RegionRect, Terrain, World};


use creatures::CreatureStore;
use events::EventRing;
use serde::{Deserialize, Serialize};

/// A notification the UI should surface (e.g. an extinction modal). C5 FR8.
#[derive(Clone, Debug, PartialEq)]
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

pub struct StepReport {
    pub alerts: Vec<Alert>,
}

/// Per-system wall-clock timing (C6 FR8/FR9), accumulated over a run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
    /// Per-species live statistics (C4 FR5), `SpeciesId::ALL` order.
    pub species: [SpeciesStats; 6],
    /// Ancestry of every creature born (C4 FR6), pruned weekly.
    pub lineage: Lineage,
    /// The soft-cap Note has been logged for the current crossing.
    pub soft_cap_noted: bool,
    /// How many times the soft-cap Note has been logged (acceptance: 0).
    pub soft_cap_crossings: u32,
    soft_cap_counted: bool,
    // ---- C5 predation / extinction / migration state ----
    /// A species has been marked globally extinct (once each).
    pub extinct: [bool; 6],
    /// Retained death record of the last individual, per species.
    pub last_extinct: [Option<ExtinctionRecord>; 6],
    /// Tick until which a (species × 8 + region) pair cannot migrate again.
    #[serde(with = "serde_big_array::BigArray")]
    pub migration_cooldown_until: [u64; 48],
    /// Consecutive days the migration trigger has held, per (species, region).
    #[serde(with = "serde_big_array::BigArray")]
    pub migration_days_below: [u32; 48],
    /// Last day a (species, region) count was ≥ `local_extinction_min`.
    #[serde(with = "serde_big_array::BigArray")]
    pub local_min_day: [Option<u32>; 48],
    /// A local-extinction Note has been emitted for this pair (until repopulated).
    #[serde(with = "serde_big_array::BigArray")]
    pub local_noted: [bool; 48],
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
    pub fn new(seed: u64, params: Params) -> Sim {
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
        let disease = DiseaseState::new(&params.disease);

        // Place founders (FR3), then build the spatial index over them.
        let mut creatures = CreatureStore::new();
        for mut c in creatures::place_founders(&world, &params.creatures, &params.genetics, params.disease.resistance_founder_sd, &mut creature_rng) {
            if params.disease.enabled {
                c.parasite_load = params.disease.parasite_baseline;
            }
            creatures.insert(c);
        }
        let mut spatial = SpatialIndex::new(&world);
        spatial.rebuild(&creatures, &world);
        let mut lineage = Lineage::new();
        for c in creatures.living() {
            lineage.record(c, params.genetics.mutation_notable);
        }
        let species = SpeciesStats::all(&census(&creatures), 0, params.genetics.drift_every_generations);

        Sim {
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
            deaths: DeathTallies::default(),
            drought: [false; 8],
            drought_days_below: [0; 8],
            species,
            lineage,
            soft_cap_noted: false,
            soft_cap_crossings: 0,
            soft_cap_counted: false,
            extinct: [false; 6],
            last_extinct: [None, None, None, None, None, None],
            migration_cooldown_until: [0; 48],
            migration_days_below: [0; 48],
            local_min_day: [None; 48],
            local_noted: [false; 48],
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

    /// Births so far today for species index `i` (the live counter).
    pub fn births_today(&self, i: usize) -> u32 {
        self.deaths.births[i]
    }

    /// Deaths so far today for species index `i` (the live counter).
    pub fn deaths_today(&self, i: usize) -> u32 {
        self.deaths.deaths[i]
    }

    /// The oldest living creature (lowest born_day, ties by id), if any.
    pub fn oldest_living(&self) -> Option<CreatureId> {
        self.creatures.living().min_by_key(|c| (c.born_day, c.id)).map(|c| c.id)
    }

    /// Advance one tick: season event, creature behaviour, then the daily update
    /// at midnight. Order is fixed for determinism (FR9). Returns the alerts
    /// raised this tick (extinction, C5 FR8).
    pub fn step(&mut self) -> StepReport {
        let profiling = self.profile_enabled;
        let step_start = std::time::Instant::now();
        let mut alerts: Vec<Alert> = Vec::new();
        // The initial Spring is announced at tick 0 (Year 1, Day 1, 06:00).
        if self.time.tick == 0 {
            self.push_season_event(Season::Spring);
        }
        if let Some(season) = self.time.advance() {
            self.push_season_event(season);
        }

        // Creature behaviour and movement (perception uses the previous tick's
        // spatial snapshot; it is rebuilt below for the next tick and the UI).
        let t0 = std::time::Instant::now();
        behavior::tick_creatures(
            &mut self.creatures,
            &self.spatial,
            &mut self.world,
            &mut self.events,
            &self.time,
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
        if profiling {
            self.profile.behavior_ns += t0.elapsed().as_nanos() as u64;
        }
        if self.soft_cap_noted {
            if !self.soft_cap_counted {
                self.soft_cap_crossings += 1;
                self.soft_cap_counted = true;
            }
            if (self.creatures.len_living() as u32) < self.params.genetics.max_population_soft_cap {
                self.soft_cap_noted = false; // re-arm for the next crossing
                self.soft_cap_counted = false;
            }
        }

        // Daily ecology + census at midnight (hour 0).
        if self.time.hour() == 0 {
            let t0 = std::time::Instant::now();
            behavior::day_boundary(
                &mut self.creatures,
                &mut self.world,
                &mut self.events,
                &self.time,
                &self.params.creatures,
                &self.params.genetics,
                &self.params.disease,
                &mut self.deaths,
                &mut self.lineage,
                &mut self.disease,
                &mut self.disease_rng,
            );
            let c = census(&self.creatures);
            let day = self.time.day_index() as u32;
            stats::update_species_daily(&mut self.species, &c, &self.deaths, day, self.params.genetics.drift_every_generations);
            if profiling {
                self.profile.day_boundary_ns += t0.elapsed().as_nanos() as u64;
            }
            // C7 FR8: outbreak tracking, epidemic alerts and emergence, after the census.
            let t0 = std::time::Instant::now();
            alerts.extend(disease::daily_update(
                &mut self.creatures,
                &self.world,
                &mut self.events,
                &self.time,
                &self.params.disease,
                &mut self.disease,
                &c.population,
                &mut self.disease_rng,
            ));
            if profiling {
                self.profile.disease_ns += t0.elapsed().as_nanos() as u64;
            }
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
                &c,
                &self.deaths,
                &self.disease,
            );
            if profiling {
                self.profile.ecology_ns += t0.elapsed().as_nanos() as u64;
            }
            self.deaths = self.deaths.next_day();
            if day.is_multiple_of(7) {
                self.lineage.prune(&c.max_generation, self.params.genetics.lineage_keep_generations, &self.creatures);
            }

            // C5 FR7: migration evaluation, then FR8 extinction detection.
            let t0 = std::time::Instant::now();
            behavior::migration_daily(
                &mut self.creatures,
                &self.world,
                &mut self.events,
                &self.time,
                &self.params.ecology,
                &self.params.predation,
                &mut self.migration_cooldown_until,
                &mut self.migration_days_below,
            );
            self.detect_extinctions(&mut alerts);
            if profiling {
                self.profile.migration_ns += t0.elapsed().as_nanos() as u64;
            }
        }

        let t0 = std::time::Instant::now();
        self.spatial.rebuild(&self.creatures, &self.world);
        if profiling {
            self.profile.spatial_ns += t0.elapsed().as_nanos() as u64;
        }

        if profiling {
            self.profile.step_ns += step_start.elapsed().as_nanos() as u64;
        }

        StepReport { alerts }
    }

    /// C5 FR8: mark globally extinct species and queue one `Extinction` alert each.
    fn detect_extinctions(&mut self, alerts: &mut Vec<Alert>) {
        let mut region_counts = [[0u32; 8]; 6];
        for c in self.creatures.living() {
            let ri = self.world.region_index(c.x, c.y).min(7);
            region_counts[c.species.index()][ri] += 1;
        }

        for (i, id) in SpeciesId::ALL.iter().enumerate() {
            // Global extinction (once per species).
            if !self.extinct[i] {
                let initial = self.params.creatures.initial_counts.get(id).copied().unwrap_or(0);
                let peak = self.species[i].peak;
                if initial > 0 && peak > 0 && self.species[i].count == 0 {
                    self.extinct[i] = true;
                    // The last individual is the species' most recent death,
                    // recorded by `behavior::kill` (survives carcass freeing).
                    let record = self.deaths.last_death[i].clone();
                    let last_id = record.as_ref().map(|r| r.last);
                    let text = match &record {
                        Some(r) => format!(
                            "The {} are extinct; the last individual was {} {} ({} in {})",
                            id.plural(),
                            r.name,
                            r.tag,
                            r.cause.label(),
                            r.region
                        ),
                        None => format!("The {} are extinct", id.plural()),
                    };
                    let pos = record.as_ref().map(|r| r.pos);
                    self.events.push(Event {
                        year: self.time.year(),
                        day: self.time.day_of_year(),
                        hour: self.time.hour(),
                        kind: EventKind::Extinction,
                        species: Some(*id),
                        subject: last_id,
                        text,
                        pos,
                        detail: String::new(),
                    });
                    let event_index = self.events.total();
                    if let Some(r) = record {
                        self.last_extinct[i] = Some(r);
                        if let Some(last) = last_id {
                            alerts.push(Alert::Extinction { event_index, species: *id, last });
                        }
                    }
                }
            }

            // Local extinction (FR8): a region whose count drops to 0 after being
            // ≥ `local_extinction_min` within the last season (90 days) emits a
            // Note once per (species, region) until repopulated.
            let today = self.time.day_index() as u32;
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
                        species: Some(*id),
                        subject: None,
                        text: format!("The {} line of {} is extinct", id.plural(), r.0),
                        pos: Some(((r.1 + r.3) / 2, (r.2 + r.4) / 2)),
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
    /// moisture, vegetation and `dried_from`; then tick, rng state, events.len()
    /// and the per-region drought flags.
    pub fn checksum(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let prime: u64 = 0x100_0000_01b3;
        let feed = |h: &mut u64, bytes: &[u8]| {
            for &b in bytes {
                *h ^= b as u64;
                *h = h.wrapping_mul(prime);
            }
        };
        for cell in &self.world.cells {
            feed(&mut h, &[cell.terrain as u8]);
            feed(&mut h, &cell.elevation.to_bits().to_le_bytes());
            feed(&mut h, &cell.moisture.to_bits().to_le_bytes());
            feed(&mut h, &cell.vegetation.to_bits().to_le_bytes());
            let dried = cell.dried_from.map(|t| t as u8 + 1).unwrap_or(0);
            feed(&mut h, &[dried]);
        }
        // Every living creature's id, x, y, hp, hunger, goal and C5 hunt/flee state (FR9).
        for c in self.creatures.living() {
            feed(&mut h, &c.id.0.to_le_bytes());
            feed(&mut h, &(c.x as u64).to_le_bytes());
            feed(&mut h, &(c.y as u64).to_le_bytes());
            feed(&mut h, &c.hp.to_bits().to_le_bytes());
            feed(&mut h, &c.hunger.to_bits().to_le_bytes());
            feed(&mut h, &[c.goal as u8]);
            feed(&mut h, &[c.hunt_phase as u8]);
            feed(&mut h, &c.hunt_target.map(|t| t.0 as u64).unwrap_or(u64::MAX).to_le_bytes());
            feed(&mut h, &c.chase_start_tick.map(|t| t.to_le_bytes()).unwrap_or([0xff; 8]));
            feed(&mut h, &c.hunt_cooldown_until.to_le_bytes());
            feed(&mut h, &c.eat_until.map(|t| t.to_le_bytes()).unwrap_or([0xff; 8]));
            feed(&mut h, &c.scavenge_target.map(|t| t.0 as u64).unwrap_or(u64::MAX).to_le_bytes());
            feed(&mut h, &c.flee_until.to_le_bytes());
            feed(&mut h, &c.threatened_by.map(|(x, y, s)| (x as u64, y as u64, s.index() as u64)).unwrap_or((u64::MAX, u64::MAX, u64::MAX)).0.to_le_bytes());
            feed(&mut h, &c.threatened_by.map(|(x, y, s)| (x as u64, y as u64, s.index() as u64)).unwrap_or((u64::MAX, u64::MAX, u64::MAX)).1.to_le_bytes());
            feed(&mut h, &c.threatened_by.map(|(x, y, s)| (x as u64, y as u64, s.index() as u64)).unwrap_or((u64::MAX, u64::MAX, u64::MAX)).2.to_le_bytes());
            feed(&mut h, &c.migrate_until.to_le_bytes());
            feed(&mut h, &c.migrate_target.map(|(x, y)| (x as u64, y as u64)).unwrap_or((u64::MAX, u64::MAX)).0.to_le_bytes());
            feed(&mut h, &c.migrate_target.map(|(x, y)| (x as u64, y as u64)).unwrap_or((u64::MAX, u64::MAX)).1.to_le_bytes());
            // C7: infection state and parasite load.
            match c.infection {
                Some(i) => {
                    feed(&mut h, &[i.pathogen.0, i.stage as u8]);
                    feed(&mut h, &i.ends_day.to_le_bytes());
                }
                None => feed(&mut h, &[0xff, 0xff]),
            }
            feed(&mut h, &c.parasite_load.to_bits().to_le_bytes());
        }
        for &d in &self.disease.last_case_day {
            feed(&mut h, &d.to_le_bytes());
        }
        feed(&mut h, &(self.disease.outbreaks.len() as u64).to_le_bytes());
        feed(&mut h, &self.disease_rng.state().to_le_bytes());
        feed(&mut h, &self.time.tick.to_le_bytes());
        feed(&mut h, &self.rng.state().to_le_bytes());
        feed(&mut h, &self.creature_rng.state().to_le_bytes());
        feed(&mut h, &self.events.total().to_le_bytes());
        for &flagged in &self.drought {
            feed(&mut h, &[flagged as u8]);
        }
        for &e in &self.extinct {
            feed(&mut h, &[e as u8]);
        }
        h
    }
}

/// FNV-1a 64-bit hash over an arbitrary byte slice (test-only helper).
#[cfg(test)]
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(a.checksum(), 0x348e3c6eeec2e6d6);
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
        p.creatures.initial_counts.clear();
        p.creatures.initial_counts.insert(species, n);
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
        let mut sim = extinction_sim(SpeciesId::Hare, 3);
        let alerts = step_to_midnight(&mut sim);
        assert!(alerts.iter().any(|a| matches!(a, Alert::Extinction { species: SpeciesId::Hare, .. })), "hare extinction alert: {alerts:?}");
        // Fox has initial_count == 0 and must never emit.
        assert!(!alerts.iter().any(|a| matches!(a, Alert::Extinction { species: SpeciesId::Fox, .. })));
        // A species emits at most once.
        let mut more = Vec::new();
        for _ in 0..200 {
            more.extend(sim.step().alerts);
        }
        let hare: Vec<_> = alerts.into_iter().chain(more).filter(|a| matches!(a, Alert::Extinction { species: SpeciesId::Hare, .. })).collect();
        assert_eq!(hare.len(), 1, "a species must emit at most once");
    }

    #[test]
    fn alert_queue_two_species_same_day() {
        let mut p = Params::default();
        p.creatures.initial_counts.clear();
        p.creatures.initial_counts.insert(SpeciesId::Vole, 2);
        p.creatures.initial_counts.insert(SpeciesId::Hare, 2);
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
        assert_eq!(species, vec![SpeciesId::Vole, SpeciesId::Hare], "species-table order on the same day");
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
                } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    // Skip this file: the test harness itself names "ratatui"/"HashMap".
                    if path.file_name().and_then(|f| f.to_str()) != Some("mod.rs") {
                        out.push(path);
                    }
                }
            }
        }
        out.sort();
        out
    }
}
