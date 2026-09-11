//! Pure, deterministic simulation core. No `ratatui` types may appear anywhere
//! under `src/sim` (enforced by a test); this module only reads/writes plain data.

pub mod ecology;
pub mod events;
pub mod params;
pub mod rng;
pub mod species;
pub mod stats;
pub mod time;
pub mod world;

pub use events::{Event, EventKind};
pub use params::{Params, Rainfall};
pub use rng::Rng;
pub use species::{Genome, Kind, SpeciesId, TRAIT_NAMES};
pub use stats::{Sample, Series};
pub use time::{Season, Time};
pub use world::{Cell, RegionRect, Terrain, World};

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
    pub rng: Rng,
    pub time: Time,
    pub world: World,
    pub events: EventRing,
    pub series: Series,
    pub drought: [bool; 8],
    pub drought_days_below: [u32; 8],
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
        Sim { params, rng, time, world, events, series, drought: [false; 8], drought_days_below: [0; 8] }
    }

    /// Advance one tick and append any season-boundary event.
    pub fn step(&mut self) -> StepReport {
        // The initial Spring is announced at tick 0 (Year 1, Day 1, 06:00).
        if self.time.tick == 0 {
            self.push_season_event(Season::Spring);
        }
        if let Some(season) = self.time.advance() {
            self.push_season_event(season);
        }
        // Daily ecology update runs at midnight (hour 0).
        if self.time.hour() == 0 {
            crate::sim::ecology::daily_update(
                &mut self.world,
                &mut self.rng,
                &self.time,
                &mut self.events,
                &mut self.series,
                &mut self.drought,
                &mut self.drought_days_below,
                &self.params.ecology,
                self.params.world.rainfall,
            );
        }
        StepReport { alerts: Vec::new() }
    }

    fn push_season_event(&mut self, season: Season) {
        self.events.push(Event {
            year: self.time.year(),
            day: self.time.day_of_year(),
            hour: self.time.hour(),
            kind: EventKind::Season,
            species: None,
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
        feed(&mut h, &self.time.tick.to_le_bytes());
        feed(&mut h, &self.rng.state().to_le_bytes());
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
        assert_eq!(a.checksum(), 0x1e07_5d3a_f213_a13c);
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
