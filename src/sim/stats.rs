//! Daily time series: a ring buffer of per-day snapshots.

use serde::{Deserialize, Serialize};

use crate::sim::creatures::CreatureStore;
use crate::sim::species::{Genome, SpeciesId};

/// Per-species population and genome statistics, computed from the living set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Census {
    pub population: [u32; 6],
    pub adults: [u32; 6],
    pub juveniles: [u32; 6],
    pub genome_mean: [Genome; 6],
    pub genome_min: [Genome; 6],
    pub genome_max: [Genome; 6],
}

/// Count living creatures per species and reduce their genomes to mean/min/max.
pub fn census(store: &CreatureStore) -> Census {
    let mut population = [0u32; 6];
    let mut adults = [0u32; 6];
    let mut juveniles = [0u32; 6];
    let mut sum = [[0.0f32; 8]; 6];
    let mut min = [[1.0f32; 8]; 6];
    let mut max = [[0.0f32; 8]; 6];

    for c in store.living() {
        let i = SpeciesId::ALL.iter().position(|&s| s == c.species).unwrap();
        population[i] += 1;
        if c.adult {
            adults[i] += 1;
        } else {
            juveniles[i] += 1;
        }
        for t in 0..8 {
            let v = c.genome.0[t];
            sum[i][t] += v;
            min[i][t] = min[i][t].min(v);
            max[i][t] = max[i][t].max(v);
        }
    }

    let mut genome_mean = [Genome([0.0; 8]); 6];
    let mut genome_min = [Genome([0.0; 8]); 6];
    let mut genome_max = [Genome([0.0; 8]); 6];
    for i in 0..6 {
        if population[i] > 0 {
            for t in 0..8 {
                sum[i][t] /= population[i] as f32;
            }
            genome_mean[i] = Genome(sum[i]);
            genome_min[i] = Genome(min[i]);
            genome_max[i] = Genome(max[i]);
        }
    }

    Census { population, adults, juveniles, genome_mean, genome_min, genome_max }
}

/// One day's snapshot of the world's ecological state.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    /// Absolute 0-based day index.
    pub day: u32,
    pub biomass_total: f32,
    /// Mean vegetation over land cells (water excluded, rock included).
    pub veg_mean: f32,
    pub water_cells: usize,
    pub water_level: f32,
    /// Mean moisture over all cells, water treated as 1.0.
    pub moisture_mean: f32,
    pub seeds: usize,
    pub dens: usize,
    pub carcasses: usize,
    pub drought_regions: usize,
    pub drought_flags: [bool; 8],
    /// Mean vegetation over land cells, per region.
    pub region_veg: [f32; 8],
    /// Mean moisture over all cells (water = 1.0), per region.
    pub region_moist: [f32; 8],
    /// Per-species population.
    pub population: [u32; 6],
    /// Per-species adult count.
    pub adults: [u32; 6],
    /// Per-species juvenile count.
    pub juveniles: [u32; 6],
    pub deaths_starved: u32,
    pub deaths_thirst: u32,
    pub deaths_age: u32,
    /// Per-species genome statistics from the census (used by the S03 table).
    pub genome_mean: [Genome; 6],
    pub genome_min: [Genome; 6],
    pub genome_max: [Genome; 6],
}

/// Daily ring buffer; oldest samples are evicted once `cap` is exceeded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Series {
    cap: usize,
    buf: Vec<Sample>,
    /// Absolute day index of the first (oldest) sample.
    day0: u32,
}

impl Series {
    pub fn new(cap: usize) -> Self {
        Series { cap, buf: Vec::new(), day0: 0 }
    }

    pub fn push(&mut self, s: Sample) {
        if self.cap == 0 {
            return;
        }
        if self.buf.is_empty() {
            self.day0 = s.day;
        }
        self.buf.push(s);
        let excess = self.buf.len().saturating_sub(self.cap);
        if excess > 0 {
            self.buf.drain(0..excess);
            self.day0 = self.buf.first().map(|s| s.day).unwrap_or(self.day0);
        }
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// All samples, oldest first.
    pub fn samples(&self) -> &[Sample] {
        &self.buf
    }

    pub fn last(&self) -> Option<&Sample> {
        self.buf.last()
    }

    /// Absolute day index of the first (oldest) sample.
    pub fn day0(&self) -> u32 {
        self.day0
    }

    /// Serialize the series as CSV: a header row plus one row per sample.
    /// C3 appends per-species daily counts and deaths by cause (FR16).
    pub fn to_csv(&self, region_names: &[String]) -> String {
        let mut out = String::from("day,biomass_total,veg_mean,water_cells,water_level,moisture_mean,seeds,drought_regions");
        for name in region_names.iter().take(8) {
            out.push_str(&format!(",veg_{name}"));
        }
        for name in region_names.iter().take(8) {
            out.push_str(&format!(",moist_{name}"));
        }
        out.push_str(",vole,hare,deer,fox,wolf,lynx,d_starved,d_thirst,d_age");
        out.push('\n');
        for s in &self.buf {
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{}",
                s.day, s.biomass_total, s.veg_mean, s.water_cells, s.water_level, s.moisture_mean, s.seeds, s.drought_regions
            ));
            for i in 0..8 {
                out.push_str(&format!(",{}", s.region_veg[i]));
            }
            for i in 0..8 {
                out.push_str(&format!(",{}", s.region_moist[i]));
            }
            for i in 0..6 {
                out.push_str(&format!(",{}", s.population[i]));
            }
            out.push_str(&format!(",{},{},{}", s.deaths_starved, s.deaths_thirst, s.deaths_age));
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::creatures::place_founders;
    use crate::sim::params::{CreaturesParams, WorldParams};
    use crate::sim::rng::Rng;
    use crate::sim::world::World;

    #[test]
    fn census_per_species() {
        let w = World::generate(7, &WorldParams::default());
        let params = CreaturesParams::default();
        let mut store = CreatureStore::new();
        for c in place_founders(&w, &params, &mut Rng::new(5)) {
            store.insert(c);
        }
        let c = census(&store);
        for (i, id) in SpeciesId::ALL.iter().enumerate() {
            let want = params.initial_counts.get(id).copied().unwrap_or(0);
            assert_eq!(c.population[i], want, "{:?}", id);
            assert_eq!(c.adults[i] + c.juveniles[i], want, "{:?}", id);
            if want > 0 {
                for t in 0..8 {
                    assert!(c.genome_min[i].0[t] <= c.genome_mean[i].0[t]);
                    assert!(c.genome_mean[i].0[t] <= c.genome_max[i].0[t]);
                }
            }
        }
    }

    fn sample(day: u32) -> Sample {
        Sample {
            day,
            biomass_total: 0.0,
            veg_mean: 0.0,
            water_cells: 0,
            water_level: 0.0,
            moisture_mean: 0.0,
            seeds: 0,
            dens: 0,
            carcasses: 0,
            drought_regions: 0,
            drought_flags: [false; 8],
            region_veg: [0.0; 8],
            region_moist: [0.0; 8],
            population: [0; 6],
            adults: [0; 6],
            juveniles: [0; 6],
            deaths_starved: 0,
            deaths_thirst: 0,
            deaths_age: 0,
            genome_mean: [Genome([0.0; 8]); 6],
            genome_min: [Genome([0.0; 8]); 6],
            genome_max: [Genome([0.0; 8]); 6],
        }
    }

    #[test]
    fn series_ring_buffer() {
        let mut s = Series::new(3);
        assert!(s.is_empty());
        for day in 0..5 {
            s.push(sample(day));
        }
        // Only the last 3 samples remain, oldest first.
        assert_eq!(s.len(), 3);
        assert_eq!(s.day0(), 2);
        let days: Vec<u32> = s.samples().iter().map(|s| s.day).collect();
        assert_eq!(days, vec![2, 3, 4]);
    }

    #[test]
    fn daily_sampling_fields() {
        let mut s = Series::new(720);
        let mut x = sample(1);
        x.biomass_total = 12.5;
        x.veg_mean = 0.4;
        x.water_cells = 300;
        x.water_level = 0.5;
        x.drought_regions = 2;
        x.drought_flags[0] = true;
        s.push(x);
        let last = s.last().unwrap();
        assert_eq!(last.day, 1);
        assert_eq!(last.biomass_total, 12.5);
        assert_eq!(last.veg_mean, 0.4);
        assert_eq!(last.water_cells, 300);
        assert_eq!(last.water_level, 0.5);
        assert_eq!(last.drought_regions, 2);
        assert!(last.drought_flags[0]);
        assert!(!last.drought_flags[1]);
    }
}
