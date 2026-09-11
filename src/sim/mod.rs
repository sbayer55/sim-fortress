//! Pure, deterministic simulation core. No `ratatui` types may appear anywhere
//! under `src/sim` (enforced by a test); this module only reads/writes plain data.

pub mod behavior;
pub mod creatures;
pub mod ecology;
pub mod events;
pub mod genetics;
pub mod geom;
pub mod lineage;
pub mod params;
pub mod rng;
pub mod spatial;
pub mod species;
pub mod stats;
pub mod time;
pub mod world;

pub use creatures::{Cause, Creature, CreatureId, Death, DeathTallies, Goal, Mutation, RestReason, Sex};
pub use events::{Event, EventKind};
pub use geom::{cheb, dist};
pub use params::{Params, Rainfall};
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

/// A notification the UI should surface (e.g. an extinction modal). Always empty
/// in C1; populated from C5.
#[derive(Clone, Debug, PartialEq)]
pub struct Alert {
    pub kind: EventKind,
    pub text: String,
}

pub struct StepReport {
    pub alerts: Vec<Alert>,
}

pub struct Sim {
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

        // Place founders (FR3), then build the spatial index over them.
        let mut creatures = CreatureStore::new();
        for c in creatures::place_founders(&world, &params.creatures, &mut creature_rng) {
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
        }
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
    /// at midnight. Order is fixed for determinism (FR9).
    pub fn step(&mut self) -> StepReport {
        // The initial Spring is announced at tick 0 (Year 1, Day 1, 06:00).
        if self.time.tick == 0 {
            self.push_season_event(Season::Spring);
        }
        if let Some(season) = self.time.advance() {
            self.push_season_event(season);
        }

        // Creature behaviour and movement (perception uses the previous tick's
        // spatial snapshot; it is rebuilt below for the next tick and the UI).
        behavior::tick_creatures(
            &mut self.creatures,
            &self.spatial,
            &mut self.world,
            &mut self.events,
            &self.time,
            &self.params.creatures,
            &self.params.ecology,
            &self.params.genetics,
            &mut self.creature_rng,
            &mut self.deaths,
            &mut self.lineage,
            &mut self.soft_cap_noted,
        );
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
            behavior::day_boundary(
                &mut self.creatures,
                &mut self.world,
                &mut self.events,
                &self.time,
                &self.params.creatures,
                &mut self.deaths,
                &mut self.lineage,
            );
            let c = census(&self.creatures);
            let day = self.time.day_index() as u32;
            stats::update_species_daily(&mut self.species, &c, &self.deaths, day, self.params.genetics.drift_every_generations);
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
            );
            self.deaths = DeathTallies::default();
            if day.is_multiple_of(7) {
                self.lineage.prune(&c.max_generation, self.params.genetics.lineage_keep_generations, &self.creatures);
            }
        }

        self.spatial.rebuild(&self.creatures, &self.world);

        StepReport { alerts: Vec::new() }
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
        // Every living creature's id, x, y, hp, hunger and goal (FR9).
        for c in self.creatures.living() {
            feed(&mut h, &c.id.0.to_le_bytes());
            feed(&mut h, &(c.x as u64).to_le_bytes());
            feed(&mut h, &(c.y as u64).to_le_bytes());
            feed(&mut h, &c.hp.to_bits().to_le_bytes());
            feed(&mut h, &c.hunger.to_bits().to_le_bytes());
            feed(&mut h, &[c.goal as u8]);
        }
        feed(&mut h, &self.time.tick.to_le_bytes());
        feed(&mut h, &self.rng.state().to_le_bytes());
        feed(&mut h, &self.creature_rng.state().to_le_bytes());
        feed(&mut h, &(self.events.len() as u64).to_le_bytes());
        for &flagged in &self.drought {
            feed(&mut h, &[flagged as u8]);
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
        assert_eq!(a.checksum(), 0x46aa_875f_107c_0e5b);
    }

    #[test]
    fn checksum_includes_creatures() {
        let a = Sim::new(42, Params::default());
        let mut b = Sim::new(42, Params::default());
        let id = b.creatures.living_ids()[0];
        b.creatures.get_mut(id).unwrap().x += 1;
        assert_ne!(a.checksum(), b.checksum(), "checksum must reflect creature state");
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
