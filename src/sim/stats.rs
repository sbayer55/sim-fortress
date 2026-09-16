//! Daily time series (a ring buffer of per-day snapshots), the per-species
//! statistics record (C4 FR5) and the lineage store re-export.

use serde::{Deserialize, Serialize};

use crate::sim::creatures::{CreatureStore, DeathTallies};
use crate::sim::params::Roster;
use crate::sim::species::{Genome, Kind, SpeciesId, TRAIT_NAMES};
use std::fmt::Write as _;

pub mod groups;
pub use crate::sim::lineage::{Lineage, LineageNode, Tree, TreeItem};
pub use groups::{group_census, GroupCensus, GROUP_HIST};

/// Trait histogram: 8 traits × 12 buckets.
pub type Hist = [[u16; 12]; Genome::LEN];

/// Histogram bucket for a trait value: `min(floor(v × 12), 11)`.
pub fn hist_bucket(v: f32) -> usize {
    (crate::cast!((v * 12.0).floor().max(0.0) => usize)).min(11)
}

/// Per-species population and genome statistics, computed from the living set.
#[derive(Clone, Debug, PartialEq)]
pub struct Census {
    pub population: Vec<u32>,
    pub adults: Vec<u32>,
    pub juveniles: Vec<u32>,
    pub genome_mean: Vec<Genome>,
    pub genome_min: Vec<Genome>,
    pub genome_max: Vec<Genome>,
    /// Trait histograms per species (C4 FR5).
    pub hist: Vec<Hist>,
    /// Highest generation among living members.
    pub max_generation: Vec<u32>,
    /// Sum of generations of living members (for the CSV mean).
    pub generation_sum: Vec<u64>,
    // ---- C7
    /// Living members with an infection (any stage).
    pub infected: Vec<u32>,
    /// Living members immune to at least one pathogen.
    pub immune: Vec<u32>,
    pub parasite_sum: Vec<f32>,
}

impl Census {
    pub fn generation_mean(&self, i: usize) -> f32 {
        if self.population[i] == 0 {
            0.0
        } else {
            crate::cast!(self.generation_sum[i] => f32) / crate::cast!(self.population[i] => f32)
        }
    }

    /// Total living prey (every species of kind prey).
    pub fn prey_total(&self, roster: &Roster) -> u32 {
        roster.prey_ids().map(|id| self.population[id.index()]).sum()
    }

    /// Mean parasite load per species.
    pub fn parasite_mean(&self, i: usize) -> f32 {
        if self.population[i] > 0 {
            self.parasite_sum[i] / crate::cast!(self.population[i] => f32)
        } else {
            0.0
        }
    }
}

/// Count living creatures per species and reduce their genomes to mean/min/max.
pub fn census(store: &CreatureStore, n_species: usize) -> Census {
    let n = n_species;
    let mut population = vec![0u32; n];
    let mut adults = vec![0u32; n];
    let mut juveniles = vec![0u32; n];
    let mut sum = vec![[0.0f32; Genome::LEN]; n];
    let mut min = vec![[1.0f32; Genome::LEN]; n];
    let mut max = vec![[0.0f32; Genome::LEN]; n];
    let mut hist = vec![[[0u16; 12]; Genome::LEN]; n];
    let mut max_generation = vec![0u32; n];
    let mut generation_sum = vec![0u64; n];
    let mut infected = vec![0u32; n];
    let mut immune = vec![0u32; n];
    let mut parasite_sum = vec![0.0f32; n];

    for c in store.living() {
        let i = c.species.index();
        population[i] += 1;
        if c.infection.is_some() {
            infected[i] += 1;
        }
        if c.immune_until.iter().any(|&u| u > 0) {
            immune[i] += 1;
        }
        parasite_sum[i] += c.parasite_load;
        if c.adult {
            adults[i] += 1;
        } else {
            juveniles[i] += 1;
        }
        max_generation[i] = max_generation[i].max(c.generation);
        generation_sum[i] += u64::from(c.generation);
        for t in 0..Genome::LEN {
            let v = c.genome.0[t];
            sum[i][t] += v;
            min[i][t] = min[i][t].min(v);
            max[i][t] = max[i][t].max(v);
            hist[i][t][hist_bucket(v)] = hist[i][t][hist_bucket(v)].saturating_add(1);
        }
    }

    let mut genome_mean = vec![Genome([0.0; Genome::LEN]); n];
    let mut genome_min = vec![Genome([0.0; Genome::LEN]); n];
    let mut genome_max = vec![Genome([0.0; Genome::LEN]); n];
    for i in 0..n {
        if population[i] > 0 {
            for t in 0..Genome::LEN {
                sum[i][t] /= crate::cast!(population[i] => f32);
            }
            genome_mean[i] = Genome(sum[i]);
            genome_min[i] = Genome(min[i]);
            genome_max[i] = Genome(max[i]);
        }
    }

    Census { population, adults, juveniles, genome_mean, genome_min, genome_max, hist, max_generation, generation_sum, infected, immune, parasite_sum }
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
    // ---- C7
    pub sick: u32,
    pub immune: u32,
    pub deaths_disease_today: u32,
    pub deaths_disease_yesterday: u32,
}

impl SpeciesStats {
    /// An empty record seeded with the species' base genome.
    pub const fn new(species: SpeciesId, base: Genome) -> Self {
        Self {
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
            mean: base,
            min: base,
            max: base,
            hist: [[0; 12]; Genome::LEN],
            drift: Vec::new(),
            last_drift_generation: 0,
            sick: 0,
            immune: 0,
            deaths_disease_today: 0,
            deaths_disease_yesterday: 0,
        }
    }

    /// One record per roster species, seeded from an initial census.
    pub fn all(c: &Census, roster: &Roster, day: u32, drift_every: u32) -> Vec<Self> {
        let mut out: Vec<Self> = roster.ids().map(|id| Self::new(id, roster.base_genome(id))).collect();
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
        let first = f32::from(*self.trend.first()?);
        let last = f32::from(*self.trend.last()?);
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
        self.sick = c.infected[i];
        self.immune = c.immune[i];
        self.trend.push(crate::cast!(self.count.min(u32::from(u16::MAX)) => u16));
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

/// Day-boundary update of every species record: roll today's counters into
/// `yesterday`, refresh from the census, and reset the live counters (the
/// tallies themselves are reset by `Sim`).
pub fn update_species_daily(stats: &mut [SpeciesStats], c: &Census, tallies: &DeathTallies, day: u32, drift_every: u32) {
    for (i, s) in stats.iter_mut().enumerate() {
        s.births_today = tallies.births[i];
        s.deaths_today = tallies.deaths[i];
        s.deaths_disease_today = tallies.disease_by_species[i];
        if s.births_today > 0 && s.first_birth_day.is_none() {
            s.first_birth_day = Some(day);
        }
        s.refresh(c, i, day, drift_every);
        s.births_yesterday = s.births_today;
        s.deaths_yesterday = s.deaths_today;
        s.births_today = 0;
        s.deaths_today = 0;
        s.deaths_disease_yesterday = s.deaths_disease_today;
        s.deaths_disease_today = 0;
    }
}

/// One day's snapshot of the world's ecological state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    pub population: Vec<u32>,
    /// Per-species adult count.
    pub adults: Vec<u32>,
    /// Per-species juvenile count.
    pub juveniles: Vec<u32>,
    pub deaths_starved: u32,
    pub deaths_thirst: u32,
    pub deaths_age: u32,
    /// Per-species genome statistics from the census (used by the S03 table).
    pub genome_mean: Vec<Genome>,
    pub genome_min: Vec<Genome>,
    pub genome_max: Vec<Genome>,
    /// Per-species births during the day (C4 FR12).
    pub births: Vec<u32>,
    /// Per-species deaths during the day (all causes).
    pub deaths: Vec<u32>,
    pub generation_mean: Vec<f32>,
    pub generation_max: Vec<u32>,
    // ---- C7 disease
    pub infected: Vec<u32>,
    pub immune: Vec<u32>,
    pub deaths_disease: u32,
    pub parasite_mean: Vec<f32>,
    pub active_by_pathogen: [u32; 8],
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
    pub const fn new(cap: usize) -> Self {
        Self { cap, buf: Vec::new(), day0: 0 }
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
            self.day0 = self.buf.first().map_or(self.day0, |s| s.day);
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
    pub const fn day0(&self) -> u32 {
        self.day0
    }

    /// Serialize the series as CSV: a header row plus one row per sample.
    /// C3 appends per-species daily counts and deaths by cause (FR16).
    /// CSV header row for [`Series::to_csv`].
    fn csv_header(region_names: &[String], roster: &Roster) -> String {
        let mut out = String::from("day,biomass_total,veg_mean,water_cells,water_level,moisture_mean,seeds,drought_regions");
        for name in region_names.iter().take(8) {
            let _ = write!(out, ",veg_{name}");
        }
        for name in region_names.iter().take(8) {
            let _ = write!(out, ",moist_{name}");
        }
        for id in roster.ids() {
            let _ = write!(out, ",{}", roster.name(id));
        }
        out.push_str(",d_starved,d_thirst,d_age");
        // C4 FR12: births, generation stats (all species) and trait means (prey).
        for id in roster.ids() {
            let _ = write!(out, ",births_{}", roster.name(id));
        }
        for id in roster.ids() {
            let _ = write!(out, ",deaths_{}", roster.name(id));
        }
        for id in roster.ids() {
            let n = roster.name(id);
            let _ = write!(out, ",{n}_generation_mean,{n}_generation_max");
        }
        for id in roster.prey_ids() {
            let n = roster.name(id);
            for t in TRAIT_NAMES {
                let _ = write!(out, ",{n}_{}_mean", t.to_lowercase());
            }
        }
        // C7 FR10: infections, disease deaths, parasite loads, active cases per pathogen slot.
        for id in roster.ids() {
            let _ = write!(out, ",infected_{}", roster.name(id));
        }
        out.push_str(",d_disease");
        for id in roster.ids() {
            let _ = write!(out, ",parasite_{}", roster.name(id));
        }
        for i in 0..8 {
            let _ = write!(out, ",active_p{i}");
        }
        out.push('\n');
        out
    }

    /// Append one sample's CSV row to `out`.
    fn write_csv_row(out: &mut String, s: &Sample, roster: &Roster) {
        let n = s.population.len();
        let _ = write!(out,
            "{},{},{},{},{},{},{},{}",
            s.day, s.biomass_total, s.veg_mean, s.water_cells, s.water_level, s.moisture_mean, s.seeds, s.drought_regions
        );
        for i in 0..8 {
            let _ = write!(out, ",{}", s.region_veg[i]);
        }
        for i in 0..8 {
            let _ = write!(out, ",{}", s.region_moist[i]);
        }
        for i in 0..n {
            let _ = write!(out, ",{}", s.population[i]);
        }
        let _ = write!(out, ",{},{},{}", s.deaths_starved, s.deaths_thirst, s.deaths_age);
        for i in 0..n {
            let _ = write!(out, ",{}", s.births[i]);
        }
        for i in 0..n {
            let _ = write!(out, ",{}", s.deaths[i]);
        }
        for i in 0..n {
            let _ = write!(out, ",{},{}", s.generation_mean[i], s.generation_max[i]);
        }
        for i in (0..n).filter(|&i| roster.kind(SpeciesId::from_index(i)) == Kind::Prey) {
            for t in 0..Genome::LEN {
                let _ = write!(out, ",{}", s.genome_mean[i].0[t]);
            }
        }
        for i in 0..n {
            let _ = write!(out, ",{}", s.infected[i]);
        }
        let _ = write!(out, ",{}", s.deaths_disease);
        for i in 0..n {
            let _ = write!(out, ",{}", s.parasite_mean[i]);
        }
        for i in 0..8 {
            let _ = write!(out, ",{}", s.active_by_pathogen[i]);
        }
        out.push('\n');
    }

    pub fn to_csv(&self, region_names: &[String], roster: &Roster) -> String {
        let mut out = Self::csv_header(region_names, roster);
        for s in &self.buf {
            Self::write_csv_row(&mut out, s, roster);
        }
        out
    }
}

/// C5 FR11: the lag (in days) at which the predator series best tracks the prey
///
/// series. Skips the first 360 days, smooths both with a 30-day centred moving
/// average, mean-subtracts and returns the `argmax` Pearson correlation lag
///
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
        if crate::cast!(l => usize) >= n {
            break;
        }
        let corr = pearson(&ps, &qs, crate::cast!(l => usize));
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
    let mean = v.iter().sum::<f32>() / crate::cast!(n.max(1) => f32);
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
    let half = crate::cast!((window.div_euclid(2)) => isize);
    let mut out = vec![0.0f32; n];
    for i in 0..n {
        let lo = crate::cast!((crate::cast!(i => isize) - half).max(0) => usize);
        let hi = crate::cast!((crate::cast!(i => isize) + half + 1).min(crate::cast!(n => isize)) => usize);
        out[i] = v[lo..hi].iter().sum::<f32>() / crate::cast!((hi - lo) => f32);
    }
    out
}

fn mean_subtract(v: &[f32]) -> Vec<f32> {
    let mean = v.iter().sum::<f32>() / crate::cast!(v.len().max(1) => f32);
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
    let m = crate::cast!(m => f32);
    let cov = sxy - sx * sy / m;
    let varx = sxx - sx * sx / m;
    let vary = syy - sy * sy / m;
    if varx <= 0.0 || vary <= 0.0 {
        return 0.0;
    }
    cov / (varx * vary).sqrt()
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests;
