//! Daily time series (a ring buffer of per-day snapshots), the per-species
//! statistics record (C4 FR5) and the lineage store re-export.

use serde::{Deserialize, Serialize};

use crate::sim::creatures::{CreatureStore, DeathTallies};
use crate::sim::species::{Genome, SpeciesId, TRAIT_NAMES};

pub use crate::sim::lineage::{Lineage, LineageNode, Tree, TreeItem};

/// Trait histogram: 8 traits × 12 buckets.
pub type Hist = [[u16; 12]; 8];

/// Histogram bucket for a trait value: `min(floor(v × 12), 11)`.
pub fn hist_bucket(v: f32) -> usize {
    ((v * 12.0).floor().max(0.0) as usize).min(11)
}

/// Per-species population and genome statistics, computed from the living set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Census {
    pub population: [u32; 6],
    pub adults: [u32; 6],
    pub juveniles: [u32; 6],
    pub genome_mean: [Genome; 6],
    pub genome_min: [Genome; 6],
    pub genome_max: [Genome; 6],
    /// Trait histograms per species (C4 FR5).
    pub hist: [Hist; 6],
    /// Highest generation among living members.
    pub max_generation: [u32; 6],
    /// Sum of generations of living members (for the CSV mean).
    pub generation_sum: [u64; 6],
}

impl Census {
    pub fn generation_mean(&self, i: usize) -> f32 {
        if self.population[i] == 0 {
            0.0
        } else {
            self.generation_sum[i] as f32 / self.population[i] as f32
        }
    }

    /// Total living prey (voles + hares + deer).
    pub fn prey_total(&self) -> u32 {
        self.population[0] + self.population[1] + self.population[2]
    }
}

/// Count living creatures per species and reduce their genomes to mean/min/max.
pub fn census(store: &CreatureStore) -> Census {
    let mut population = [0u32; 6];
    let mut adults = [0u32; 6];
    let mut juveniles = [0u32; 6];
    let mut sum = [[0.0f32; 8]; 6];
    let mut min = [[1.0f32; 8]; 6];
    let mut max = [[0.0f32; 8]; 6];
    let mut hist = [[[0u16; 12]; 8]; 6];
    let mut max_generation = [0u32; 6];
    let mut generation_sum = [0u64; 6];

    for c in store.living() {
        let i = c.species.index();
        population[i] += 1;
        if c.adult {
            adults[i] += 1;
        } else {
            juveniles[i] += 1;
        }
        max_generation[i] = max_generation[i].max(c.generation);
        generation_sum[i] += c.generation as u64;
        for t in 0..8 {
            let v = c.genome.0[t];
            sum[i][t] += v;
            min[i][t] = min[i][t].min(v);
            max[i][t] = max[i][t].max(v);
            hist[i][t][hist_bucket(v)] = hist[i][t][hist_bucket(v)].saturating_add(1);
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

    Census { population, adults, juveniles, genome_mean, genome_min, genome_max, hist, max_generation, generation_sum }
}

/// One species' live record (C4 FR5). Counters are incremental during the day;
/// the rest is refreshed from the census at the day boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpeciesStats {
    pub species: SpeciesId,
    pub count: u32,
    pub adults: u32,
    pub juveniles: u32,
    pub births_today: u32,
    pub deaths_today: u32,
    pub births_yesterday: u32,
    pub deaths_yesterday: u32,
    /// All-time peak count.
    pub peak: u32,
    pub first_birth_day: Option<u32>,
    /// High-water mark of the max generation among living members.
    pub generation: u32,
    /// Last 30 daily counts, oldest first.
    pub trend: Vec<u16>,
    pub mean: Genome,
    pub min: Genome,
    pub max: Genome,
    pub hist: Hist,
    /// Last 12 samples of the mean genome, `(generation, mean)`, oldest first.
    pub drift: Vec<(u32, Genome)>,
    pub last_drift_generation: u32,
}

impl SpeciesStats {
    pub fn new(species: SpeciesId) -> Self {
        SpeciesStats {
            species,
            count: 0,
            adults: 0,
            juveniles: 0,
            births_today: 0,
            deaths_today: 0,
            births_yesterday: 0,
            deaths_yesterday: 0,
            peak: 0,
            first_birth_day: None,
            generation: 0,
            trend: Vec::new(),
            mean: species.base_genome(),
            min: species.base_genome(),
            max: species.base_genome(),
            hist: [[0; 12]; 8],
            drift: Vec::new(),
            last_drift_generation: 0,
        }
    }

    /// All six records in `SpeciesId::ALL` order, seeded from an initial census.
    pub fn all(c: &Census, day: u32, drift_every: u32) -> [SpeciesStats; 6] {
        let mut out = SpeciesId::ALL.map(SpeciesStats::new);
        for (i, s) in out.iter_mut().enumerate() {
            s.refresh(c, i, day, drift_every);
            // The founding census is the first drift sample.
            if s.count > 0 && s.drift.is_empty() {
                s.drift.push((s.generation, s.mean));
                s.last_drift_generation = s.generation;
            }
        }
        out
    }

    /// 30-day change in percent (`None` when the trend has fewer than two points
    /// or started from zero).
    pub fn change_pct(&self) -> Option<f32> {
        let first = *self.trend.first()? as f32;
        let last = *self.trend.last()? as f32;
        if self.trend.len() < 2 || first == 0.0 {
            return None;
        }
        Some((last - first) / first * 100.0)
    }

    /// Refresh the census-derived fields for species index `i` on day `day`.
    fn refresh(&mut self, c: &Census, i: usize, day: u32, drift_every: u32) {
        self.count = c.population[i];
        self.adults = c.adults[i];
        self.juveniles = c.juveniles[i];
        self.peak = self.peak.max(self.count);
        self.generation = self.generation.max(c.max_generation[i]);
        if self.count > 0 {
            self.mean = c.genome_mean[i];
            self.min = c.genome_min[i];
            self.max = c.genome_max[i];
        }
        self.hist = c.hist[i];
        self.trend.push(self.count.min(u16::MAX as u32) as u16);
        if self.trend.len() > 30 {
            let excess = self.trend.len() - 30;
            self.trend.drain(0..excess);
        }
        if self.count > 0 && self.generation.saturating_sub(self.last_drift_generation) >= drift_every.max(1) {
            self.drift.push((self.generation, self.mean));
            self.last_drift_generation = self.generation;
            if self.drift.len() > 12 {
                let excess = self.drift.len() - 12;
                self.drift.drain(0..excess);
            }
        }
        let _ = day;
    }
}

/// Day-boundary update of all six records: roll today's counters into
/// `yesterday`, refresh from the census, and reset the live counters (the
/// tallies themselves are reset by `Sim`).
pub fn update_species_daily(stats: &mut [SpeciesStats; 6], c: &Census, tallies: &DeathTallies, day: u32, drift_every: u32) {
    for (i, s) in stats.iter_mut().enumerate() {
        s.births_today = tallies.births[i];
        s.deaths_today = tallies.deaths[i];
        if s.births_today > 0 && s.first_birth_day.is_none() {
            s.first_birth_day = Some(day);
        }
        s.refresh(c, i, day, drift_every);
        s.births_yesterday = s.births_today;
        s.deaths_yesterday = s.deaths_today;
        s.births_today = 0;
        s.deaths_today = 0;
    }
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
    /// Per-species births during the day (C4 FR12).
    pub births: [u32; 6],
    /// Per-species deaths during the day (all causes).
    pub deaths: [u32; 6],
    pub generation_mean: [f32; 6],
    pub generation_max: [u32; 6],
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
        // C4 FR12: births, generation stats (all species) and trait means (prey).
        for id in SpeciesId::ALL {
            out.push_str(&format!(",births_{}", id.name().to_lowercase()));
        }
        for id in SpeciesId::ALL {
            out.push_str(&format!(",deaths_{}", id.name().to_lowercase()));
        }
        for id in SpeciesId::ALL {
            let n = id.name().to_lowercase();
            out.push_str(&format!(",{n}_generation_mean,{n}_generation_max"));
        }
        for id in SpeciesId::ALL.iter().take(3) {
            let n = id.name().to_lowercase();
            for t in TRAIT_NAMES {
                out.push_str(&format!(",{n}_{}_mean", t.to_lowercase()));
            }
        }
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
            for i in 0..6 {
                out.push_str(&format!(",{}", s.births[i]));
            }
            for i in 0..6 {
                out.push_str(&format!(",{}", s.deaths[i]));
            }
            for i in 0..6 {
                out.push_str(&format!(",{},{}", s.generation_mean[i], s.generation_max[i]));
            }
            for i in 0..3 {
                for t in 0..8 {
                    out.push_str(&format!(",{}", s.genome_mean[i].0[t]));
                }
            }
            out.push('\n');
        }
        out
    }
}

/// C5 FR11: the lag (in days) at which the predator series best tracks the prey
/// series. Skips the first 360 days, smooths both with a 30-day centred moving
/// average, mean-subtracts and returns the `argmax` Pearson correlation lag
/// `L ∈ 0..=120` (lowest index on ties), or `None` when either series has fewer
/// than two local maxima.
pub fn peak_lag(prey: &[f32], pred: &[f32]) -> Option<u32> {
    if prey.len() <= 360 || pred.len() <= 360 {
        return None;
    }
    let prey = &prey[360..];
    let pred = &pred[360..];
    let n = prey.len().min(pred.len());
    if n < 31 {
        return None;
    }
    let ps = centred_ma(&prey[..n], 30);
    let qs = centred_ma(&pred[..n], 30);
    if local_maxima(&ps).len() < 2 || local_maxima(&qs).len() < 2 {
        return None;
    }
    let ps = mean_subtract(&ps);
    let qs = mean_subtract(&qs);
    let mut best: Option<(u32, f32)> = None;
    for l in 0..=120u32 {
        if l as usize >= n {
            break;
        }
        let corr = pearson(&ps, &qs, l as usize);
        if best.is_none_or(|b| corr > b.1) {
            best = Some((l, corr));
        }
    }
    best.map(|(l, _)| l)
}

/// A sample that is ≥ every sample within ±45 days (lowest index on ties) and
/// ≥ 1.15 × the series mean.
pub fn local_maxima(v: &[f32]) -> Vec<usize> {
    let n = v.len();
    let mean = v.iter().sum::<f32>() / n.max(1) as f32;
    let mut out = Vec::new();
    for i in 0..n {
        let lo = i.saturating_sub(45);
        let hi = (i + 45).min(n - 1);
        let ok = (lo..i).all(|j| v[i] > v[j]) && ((i + 1)..=hi).all(|j| v[i] >= v[j]);
        if ok && v[i] >= 1.15 * mean {
            out.push(i);
        }
    }
    out
}

fn centred_ma(v: &[f32], window: usize) -> Vec<f32> {
    let n = v.len();
    let half = (window / 2) as isize;
    let mut out = vec![0.0f32; n];
    for i in 0..n {
        let lo = (i as isize - half).max(0) as usize;
        let hi = (i as isize + half + 1).min(n as isize) as usize;
        out[i] = v[lo..hi].iter().sum::<f32>() / (hi - lo) as f32;
    }
    out
}

fn mean_subtract(v: &[f32]) -> Vec<f32> {
    let mean = v.iter().sum::<f32>() / v.len().max(1) as f32;
    v.iter().map(|x| x - mean).collect()
}

/// Pearson correlation of `y[t + lag]` against `x[t]`, `t ∈ 0..(n − lag)`.
fn pearson(x: &[f32], y: &[f32], lag: usize) -> f32 {
    let n = x.len().min(y.len());
    let m = n.saturating_sub(lag);
    if m < 2 {
        return 0.0;
    }
    let (mut sx, mut sy, mut sxx, mut syy, mut sxy) = (0.0f32, 0.0, 0.0, 0.0, 0.0);
    for t in 0..m {
        let a = x[t];
        let b = y[t + lag];
        sx += a;
        sy += b;
        sxx += a * a;
        syy += b * b;
        sxy += a * b;
    }
    let m = m as f32;
    let cov = sxy - sx * sy / m;
    let varx = sxx - sx * sx / m;
    let vary = syy - sy * sy / m;
    if varx <= 0.0 || vary <= 0.0 {
        return 0.0;
    }
    cov / (varx * vary).sqrt()
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
            births: [0; 6],
            deaths: [0; 6],
            generation_mean: [0.0; 6],
            generation_max: [0; 6],
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
    fn histogram_buckets() {
        assert_eq!(hist_bucket(0.0), 0);
        assert_eq!(hist_bucket(0.02), 0);
        assert_eq!(hist_bucket(1.0 / 12.0), 1);
        assert_eq!(hist_bucket(0.5), 6);
        assert_eq!(hist_bucket(0.98), 11);
        assert_eq!(hist_bucket(1.0), 11);
        let w = World::generate(7, &WorldParams::default());
        let mut store = CreatureStore::new();
        for c in place_founders(&w, &CreaturesParams::default(), &mut Rng::new(5)) {
            store.insert(c);
        }
        let c = census(&store);
        for i in 0..6 {
            for t in 0..8 {
                let n: u32 = c.hist[i][t].iter().map(|&v| v as u32).sum();
                assert_eq!(n, c.population[i], "histogram {i}/{t} must count every living member");
            }
        }
    }

    #[test]
    fn species_record_incremental_equals_full() {
        // The incremental birth/death counters carried by the species record
        // must equal a full recount from the event log and the daily samples.
        let mut sim = crate::sim::Sim::new(42, crate::sim::Params::default());
        for _ in 0..24 * 120 {
            sim.step();
        }
        // Compare at a day boundary (the record is refreshed at midnight).
        while sim.time.hour() != 0 {
            sim.step();
        }
        for (i, s) in sim.species.iter().enumerate() {
            let id = SpeciesId::ALL[i];
            let full = census(&sim.creatures);
            assert_eq!(s.count, full.population[i], "{:?} count", id);
            assert_eq!(s.adults, full.adults[i], "{:?} adults", id);
            assert_eq!(s.juveniles, full.juveniles[i], "{:?} juveniles", id);
            assert_eq!(s.hist, full.hist[i], "{:?} histogram", id);
            let births_series: u32 = sim.series.samples().iter().map(|x| x.births[i]).sum();
            let deaths_series: u32 = sim.series.samples().iter().map(|x| x.deaths[i]).sum();
            let births_events: u32 = sim
                .events
                .iter()
                .filter(|e| e.kind == crate::sim::EventKind::Birth && e.species == Some(id))
                .map(|e| e.text.split_whitespace().skip_while(|w| *w != "bore").nth(1).and_then(|w| w.parse::<u32>().ok()).unwrap_or(0))
                .sum();
            assert_eq!(births_series, births_events, "{:?} births: series vs events", id);
            let last = sim.series.last().unwrap();
            assert_eq!(s.births_yesterday, last.births[i], "{:?} births_yesterday", id);
            assert_eq!(s.deaths_yesterday, last.deaths[i], "{:?} deaths_yesterday", id);
            assert!(s.peak >= s.count);
            assert!(s.generation >= full.max_generation[i]);
            let _ = deaths_series;
        }
    }

    #[test]
    fn drift_sample_cadence() {
        let w = World::generate(7, &WorldParams::default());
        let mut store = CreatureStore::new();
        for c in place_founders(&w, &CreaturesParams::default(), &mut Rng::new(5)) {
            store.insert(c);
        }
        let c = census(&store);
        let mut stats = SpeciesStats::all(&c, 0, 2);
        let vole = &stats[0];
        assert_eq!(vole.drift.len(), 1, "the founding census is the first sample");
        assert_eq!(vole.drift[0].0, 1);
        // Generation grows by one: no new sample; by two: a sample.
        let tallies = DeathTallies::default();
        let mut c2 = c;
        c2.max_generation[0] = 2;
        update_species_daily(&mut stats, &c2, &tallies, 1, 2);
        assert_eq!(stats[0].drift.len(), 1);
        c2.max_generation[0] = 3;
        update_species_daily(&mut stats, &c2, &tallies, 2, 2);
        assert_eq!(stats[0].drift.len(), 2);
        assert_eq!(stats[0].drift[1].0, 3);
        // Bounded to 12 samples.
        for g in 0..40u32 {
            c2.max_generation[0] = 5 + 2 * g;
            update_species_daily(&mut stats, &c2, &tallies, 3 + g, 2);
        }
        assert_eq!(stats[0].drift.len(), 12);
        assert_eq!(stats[0].trend.len(), 30, "trend keeps the last 30 daily counts");
    }

    fn lineage_creature(id: u32, gen: u32, parents: Option<(u32, u32)>, alive: bool) -> crate::sim::Creature {
        let w = World::generate(7, &WorldParams::default());
        let mut c = place_founders(&w, &CreaturesParams::default(), &mut Rng::new(1)).remove(0);
        c.id = crate::sim::CreatureId(id);
        c.generation = gen;
        c.parents = parents.map(|(m, f)| (crate::sim::CreatureId(m), crate::sim::CreatureId(f)));
        c.born_day = gen as i32 * 10;
        c.alive = alive;
        c
    }

    #[test]
    fn lineage_prune_keeps_ancestors() {
        let mut lin = Lineage::new();
        let mut store = CreatureStore::new();
        // g1: 1 (mother), 2 (father) → g2: 3 → g3: 4 (living) ; 5 is a dead g1 with no living descendants.
        for (id, gen, parents, alive) in [(1, 1, None, false), (2, 1, None, false), (5, 1, None, false), (3, 2, Some((1, 2)), false), (4, 3, Some((3, 3)), true)] {
            let c = lineage_creature(id, gen, parents, alive);
            lin.record(&c, 0.1);
            if !alive {
                lin.record_death(c.id, 50);
            }
            if alive {
                let mut cc = c.clone();
                cc.id = crate::sim::CreatureId(0);
                let got = store.insert(cc);
                // The store hands out its own ids; make the living creature's id match.
                assert_eq!(got, crate::sim::CreatureId(1));
            }
        }
        // Emulate species max generation 20 with keep 8: cutoff 12 → all g1..g3 dead nodes are candidates.
        let living_store = {
            // Build a store whose only living creature has id 4.
            let mut s = CreatureStore::new();
            for _ in 0..3 {
                let mut filler = lineage_creature(0, 1, None, false);
                filler.alive = false;
                s.insert(filler);
            }
            let alive = lineage_creature(0, 3, Some((3, 3)), true);
            let id = s.insert(alive);
            assert_eq!(id, crate::sim::CreatureId(4));
            s
        };
        let _ = store;
        let removed = lin.prune(&[20; 6], 8, &living_store);
        assert_eq!(removed, 1, "only the dead node with no living descendants is pruned");
        assert!(lin.get(crate::sim::CreatureId(5)).is_none());
        for id in [1, 2, 3, 4] {
            assert!(lin.get(crate::sim::CreatureId(id)).is_some(), "ancestor {id} of a living creature must survive pruning");
        }
    }

    #[test]
    fn lineage_root_depth_and_cap() {
        // A mother chain 1 → 2 → 3 → 4 → 5 (focus) with many siblings per level.
        let mut lin = Lineage::new();
        let mut next = 100u32;
        for id in 1..=5u32 {
            let parents = if id == 1 { None } else { Some((id - 1, id - 1)) };
            lin.record(&lineage_creature(id, id, parents, true), 0.1);
            if id > 1 {
                for _ in 0..200 {
                    lin.record(&lineage_creature(next, id, Some((id - 1, id - 1)), true), 0.1);
                    next += 1;
                }
            }
        }
        let tree = lin.tree(crate::sim::CreatureId(5), 3, 400).unwrap();
        assert_eq!(tree.root, crate::sim::CreatureId(2), "root is 3 generations up the mother line");
        assert!(tree.node_count <= 400, "tree has {} nodes", tree.node_count);
        let ids = tree.node_ids();
        for id in [2, 3, 4, 5] {
            assert!(ids.contains(&crate::sim::CreatureId(id)), "chain node {id} missing");
        }
        assert!(tree.items.iter().any(|it| matches!(it, TreeItem::More { .. })), "truncated branches show `… and N more`");
        // A shallow chain stops early.
        let t2 = lin.tree(crate::sim::CreatureId(2), 3, 400).unwrap();
        assert_eq!(t2.root, crate::sim::CreatureId(1));
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

    #[test]
    fn local_maxima_rule() {
        // A flat series has no local maxima (nothing rises 15 % above the mean).
        let flat = vec![1.0f32; 200];
        assert!(local_maxima(&flat).is_empty(), "a flat series has no local maxima");

        // Two broad peaks well separated (> 90 days apart): both are detected.
        let mut v = vec![0.5f32; 600];
        for i in 100..=140 {
            v[i] = 2.0;
        }
        for i in 400..=440 {
            v[i] = 2.5;
        }
        let maxima = local_maxima(&v);
        assert_eq!(maxima, vec![100, 400], "lowest index on each plateau: {maxima:?}");
    }

    #[test]
    fn peak_lag_on_synthetic_series() {
        // A predator series that is the prey series delayed by 20 days must yield lag 20.
        let n = 800usize;
        let mut prey = vec![0.0f32; n];
        for i in 0..n {
            prey[i] = ((i as f32 / 60.0).sin() + 1.0) * 100.0;
        }
        let mut pred = vec![0.0f32; n];
        pred[20..n].copy_from_slice(&prey[..n - 20]);
        assert_eq!(peak_lag(&prey, &pred), Some(20));

        // Fewer than two local maxima → None.
        let flat = vec![50.0f32; n];
        assert_eq!(peak_lag(&flat, &flat), None);
    }
}
