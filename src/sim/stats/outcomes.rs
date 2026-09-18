//! Trait outcomes (S15 Traits & Fates): how each genome trait relates to what
//! happened to the creatures that carried it over the last 240 days.
//!
//! [`collect_lives`] turns the living set plus the lineage's life log into one
//! [`Life`] per creature of a species; everything else is a pure function over
//! that slice, so the screen and the tests share one definition of each
//! outcome. Nothing here is stored or serialised and no RNG is drawn.
//!
//! - **Outcomes** ([`Outcome`]): lifespan (the dead), young per year of adult
//!   life (adults of 45 days or more), escape rate for prey or kill rate for
//!   predators (two or more chases or hunts), and one 0/1 column per cause of
//!   death (the dead).
//! - **Crowding**: a life is *crowded* when its species' population on the day
//!   it died (today, for the living) was above the species' median over the
//!   last 240 daily samples.

use crate::sim::creatures::{adult_age_days, Cause};
use crate::sim::lineage::LIFE_WINDOW_DAYS;
use crate::sim::species::{Genome, Kind, SpeciesId, N_TRAITS};
use crate::sim::Sim;

/// A cell needs this many lives before it is shown.
pub const MIN_N: u32 = 30;
/// Adult days before young-per-year counts, so a new adult's zero is not read as a rate.
pub const MIN_ADULT_DAYS: u32 = 45;
/// Age bands in [`AgeProfile`].
pub const AGE_BANDS: usize = 18;
pub const N_OUTCOMES: usize = 9;

/// Which days' lives to count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Crowding {
    #[default]
    All,
    Crowded,
    Sparse,
}

impl Crowding {
    pub const ALL: [Self; 3] = [Self::All, Self::Crowded, Self::Sparse];

    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all days",
            Self::Crowded => "crowded",
            Self::Sparse => "sparse",
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::All => Self::Crowded,
            Self::Crowded => Self::Sparse,
            Self::Sparse => Self::All,
        }
    }

    const fn admits(self, crowded: Option<bool>) -> bool {
        match self {
            Self::All => true,
            Self::Crowded => matches!(crowded, Some(true)),
            Self::Sparse => matches!(crowded, Some(false)),
        }
    }
}

/// Alive now, or how the creature died.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fate {
    Alive,
    Died(Cause),
}

/// One creature's life, reduced to what the analysis needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Life {
    pub genome: Genome,
    /// Age now (living) or at death.
    pub age: u32,
    pub fate: Fate,
    /// `None` when the population on that day is not in the series.
    pub crowded: Option<bool>,
    pub young_per_year: Option<f32>,
    /// Escapes per chase (prey) or kills per hunt (predators).
    pub hunt_rate: Option<f32>,
}

impl Life {
    pub const fn dead(&self) -> bool {
        matches!(self.fate, Fate::Died(_))
    }
}

/// Whether more of an outcome is good for the creature, bad, or neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Good,
    Bad,
    Neutral,
}

/// One matrix column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Lifespan,
    Young,
    Hunt,
    Died(Cause),
}

pub const OUTCOMES: [Outcome; N_OUTCOMES] = [
    Outcome::Lifespan,
    Outcome::Young,
    Outcome::Hunt,
    Outcome::Died(Cause::Starved),
    Outcome::Died(Cause::Thirst),
    Outcome::Died(Cause::Age),
    Outcome::Died(Cause::Predation),
    Outcome::Died(Cause::Disease),
    Outcome::Died(Cause::Injury),
];

impl Outcome {
    /// The outcome's value for one life, or `None` when it does not apply.
    pub fn value(self, l: &Life) -> Option<f32> {
        match (self, l.fate) {
            (Self::Lifespan, Fate::Died(_)) => Some(crate::cast!(l.age => f32)),
            (Self::Young, _) => l.young_per_year,
            (Self::Hunt, _) => l.hunt_rate,
            (Self::Died(want), Fate::Died(got)) => Some(if want == got { 1.0 } else { 0.0 }),
            (Self::Lifespan | Self::Died(_), Fate::Alive) => None,
        }
    }

    /// Whether more of the outcome is good for the creature. Dying of old age
    /// is neutral: within a 240-day window a long-lived creature is *less*
    /// likely to have died of old age, so calling it good or bad would read a
    /// trait that works as one that hurts.
    pub const fn tone(self) -> Tone {
        match self {
            Self::Lifespan | Self::Young | Self::Hunt => Tone::Good,
            Self::Died(Cause::Age) => Tone::Neutral,
            Self::Died(_) => Tone::Bad,
        }
    }

    /// Column header, at most 7 cells.
    pub const fn header(self, kind: Kind) -> &'static str {
        match self {
            Self::Lifespan => "Life",
            Self::Young => "Young",
            Self::Hunt => match kind {
                Kind::Prey => "Escape",
                Kind::Predator => "Kills",
            },
            Self::Died(c) => match c {
                Cause::Starved => "Starve",
                Cause::Thirst => "Thirst",
                Cause::Age => "OldAge",
                Cause::Predation => "Preyed",
                Cause::Disease => "Disease",
                Cause::Injury => "Injury",
            },
        }
    }

    /// Words for the sidebar ("Speed against predation").
    pub const fn label(self, kind: Kind) -> &'static str {
        match self {
            Self::Lifespan => "lifespan",
            Self::Young => "young per year",
            Self::Hunt => match kind {
                Kind::Prey => "escape rate",
                Kind::Predator => "kill rate",
            },
            Self::Died(c) => match c {
                Cause::Starved => "starvation",
                Cause::Thirst => "thirst",
                Cause::Age => "old age",
                Cause::Predation => "predation",
                Cause::Disease => "disease",
                Cause::Injury => "injury",
            },
        }
    }
}

/// Every life of `species` in the window that `crowding` admits: the living
/// set first (slot order), then the life log (oldest first).
pub fn collect_lives(sim: &Sim, species: SpeciesId, crowding: Crowding) -> Vec<Life> {
    let today = crate::cast!(sim.time.day_index() => u32);
    let kind = sim.roster().kind(species);
    let sp = sim.roster().get(species);
    let gp = &sim.params.genetics;
    let crowd = CrowdLine::new(sim, species);
    let rates = |genome: &Genome, age: u32, offspring: u32, hunt: (u32, u32)| {
        let adult_days = age.saturating_sub(adult_age_days(sp, genome, gp));
        let young = (adult_days >= MIN_ADULT_DAYS).then(|| crate::cast!(offspring => f32) * 365.0 / crate::cast!(adult_days => f32));
        let hunt = (hunt.1 >= 2).then(|| crate::cast!(hunt.0 => f32) / crate::cast!(hunt.1 => f32));
        (young, hunt)
    };
    let pick = |kills: u32, attempts: u32, escaped: u32, chased: u32| match kind {
        Kind::Prey => (escaped, chased),
        Kind::Predator => (kills, attempts),
    };
    let mut out = Vec::new();
    for c in sim.creatures.living().filter(|c| c.species == species) {
        let age = c.age_days(sim.time.day_index());
        let (young_per_year, hunt_rate) = rates(&c.genome, age, c.offspring, pick(c.kills, c.attempts, c.escaped, c.chased));
        let life = Life { genome: c.genome, age, fate: Fate::Alive, crowded: crowd.on(today), young_per_year, hunt_rate };
        if crowding.admits(life.crowded) {
            out.push(life);
        }
    }
    let window = sim.lineage.lives().records().filter(|r| r.species == species && r.died_day.saturating_add(LIFE_WINDOW_DAYS) >= today);
    for r in window {
        let age = r.age_days();
        let (young_per_year, hunt_rate) = rates(&r.genome, age, r.offspring, pick(r.kills, r.attempts, r.escaped, r.chased));
        let life = Life { genome: r.genome, age, fate: Fate::Died(r.cause), crowded: crowd.on(r.died_day), young_per_year, hunt_rate };
        if crowding.admits(life.crowded) {
            out.push(life);
        }
    }
    out
}

/// A species' daily population in the series and its median over the window.
struct CrowdLine<'a> {
    samples: &'a [crate::sim::stats::Sample],
    day0: u32,
    idx: usize,
    median: Option<f32>,
}

impl<'a> CrowdLine<'a> {
    fn new(sim: &'a Sim, species: SpeciesId) -> Self {
        let samples = sim.series.samples();
        let idx = species.index();
        let recent = samples.len().saturating_sub(crate::cast!(LIFE_WINDOW_DAYS => usize));
        let pop: Vec<u32> = samples.iter().skip(recent).map(|s| s.population.get(idx).copied().unwrap_or(0)).collect();
        Self { samples, day0: sim.series.day0(), idx, median: median(&pop) }
    }

    /// Crowded on `day`: the population that day above the window median. A day
    /// whose midnight sample is not taken yet (today) reads the latest sample.
    fn on(&self, day: u32) -> Option<bool> {
        let median = self.median?;
        let i = crate::cast!(day.checked_sub(self.day0)? => usize);
        let sample = self.samples.get(i).or_else(|| self.samples.last().filter(|s| s.day < day))?;
        let pop = *sample.population.get(self.idx)?;
        Some(crate::cast!(pop => f32) > median)
    }
}

/// Median of a slice (mean of the two middle values), `None` when empty.
pub fn median(v: &[u32]) -> Option<f32> {
    let mut s = v.to_vec();
    s.sort_unstable();
    let n = s.len();
    let hi = *s.get(n.div_euclid(2))?;
    let lo = if n % 2 == 0 { *s.get(n.div_euclid(2) - 1)? } else { hi };
    Some((crate::cast!(lo => f32) + crate::cast!(hi => f32)) / 2.0)
}

/// Correlation between one trait and one outcome, and how many lives it rests on.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CellStat {
    pub r: f32,
    pub n: u32,
    /// Every life had the same outcome (say, nobody died of disease), so there
    /// is nothing to correlate: `r` is 0 and means "no variation", not "no link".
    pub flat: bool,
}

impl CellStat {
    /// Enough lives to show (`MIN_N`) and some variation in the outcome.
    pub const fn shown(self) -> bool {
        self.n >= MIN_N && !self.flat
    }
}

/// Trait × outcome correlations, `cells[trait][outcome]` in `OUTCOMES` order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Matrix {
    pub cells: [[CellStat; N_OUTCOMES]; N_TRAITS],
}

impl Matrix {
    pub fn cell(&self, t: usize, o: usize) -> CellStat {
        self.cells.get(t).and_then(|row| row.get(o)).copied().unwrap_or_default()
    }
}

/// Pearson r of every trait against every outcome over `lives`.
pub fn matrix(lives: &[Life]) -> Matrix {
    let mut m = Matrix::default();
    for (o, outcome) in OUTCOMES.iter().enumerate() {
        let pairs: Vec<(&Life, f32)> = lives.iter().filter_map(|l| outcome.value(l).map(|y| (l, y))).collect();
        let ys: Vec<f32> = pairs.iter().map(|p| p.1).collect();
        let n = crate::cast!(pairs.len() => u32);
        let flat = ys.iter().all(|&y| (y - ys.first().copied().unwrap_or(0.0)).abs() < f32::EPSILON);
        for t in 0..N_TRAITS {
            let xs: Vec<f32> = pairs.iter().map(|p| p.0.genome.0[t]).collect();
            m.cells[t][o] = CellStat { r: super::pearson(&xs, &ys, 0), n, flat };
        }
    }
    m
}

/// The `k` strongest shown cells by |r|, as `(trait, outcome, r)`; ties keep matrix order.
pub fn strongest(m: &Matrix, k: usize) -> Vec<(usize, usize, f32)> {
    let mut all: Vec<(usize, usize, f32)> = Vec::new();
    for t in 0..N_TRAITS {
        for o in 0..N_OUTCOMES {
            let c = m.cell(t, o);
            if c.shown() {
                all.push((t, o, c.r));
            }
        }
    }
    all.sort_by(|a, b| b.2.abs().total_cmp(&a.2.abs()));
    all.truncate(k);
    all
}

/// The two trait values that split `lives` into equal thirds (low < `.0` ≤ mid < `.1` ≤ high).
pub fn third_cuts(lives: &[Life], t: usize) -> (f32, f32) {
    let mut v: Vec<f32> = lives.iter().map(|l| l.genome.0[t]).collect();
    v.sort_by(f32::total_cmp);
    let at = |q: usize| v.get((v.len() * q).div_euclid(3)).copied().unwrap_or(0.5);
    (at(1), at(2))
}

/// 0, 1 or 2: the third a trait value falls in.
pub fn third_of(cuts: (f32, f32), v: f32) -> usize {
    usize::from(v >= cuts.0) + usize::from(v >= cuts.1)
}

/// Mean outcome and count per trait third.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ThirdStat {
    pub mean: f32,
    pub n: u32,
}

pub fn thirds(lives: &[Life], t: usize, outcome: Outcome) -> [ThirdStat; 3] {
    let cuts = third_cuts(lives, t);
    let mut sum = [0.0f32; 3];
    let mut n = [0u32; 3];
    for l in lives {
        if let Some(y) = outcome.value(l) {
            let k = third_of(cuts, l.genome.0[t]);
            sum[k] += y;
            n[k] += 1;
        }
    }
    [0, 1, 2].map(|k| ThirdStat { mean: if n[k] > 0 { sum[k] / crate::cast!(n[k] => f32) } else { 0.0 }, n: n[k] })
}

/// One age band of [`AgeProfile`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AgeBand {
    /// Living creatures whose current age falls in the band.
    pub living: u32,
    /// Mean trait value of those, when there are at least three.
    pub trait_mean: Option<f32>,
    pub deaths: u32,
    /// The commonest cause among the band's deaths and its count.
    pub top: Option<(Cause, u32)>,
}

/// A trait followed across age: `AGE_BANDS` bands of `band_days` each; the
/// last band also holds everything older.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgeProfile {
    pub band_days: u32,
    pub bands: [AgeBand; AGE_BANDS],
}

const CAUSES: [Cause; 6] = [Cause::Starved, Cause::Thirst, Cause::Age, Cause::Predation, Cause::Disease, Cause::Injury];

pub fn age_profile(lives: &[Life], t: usize) -> AgeProfile {
    let mut ages: Vec<u32> = lives.iter().map(|l| l.age).collect();
    ages.sort_unstable();
    let top_age = ages.get((ages.len() * 985).div_euclid(1000)).or_else(|| ages.last()).copied().unwrap_or(0);
    let band_days = top_age.div_ceil(crate::cast!(AGE_BANDS => u32)).max(1);
    let mut p = AgeProfile { band_days, bands: [AgeBand::default(); AGE_BANDS] };
    let mut sums = [0.0f32; AGE_BANDS];
    let mut causes = [[0u32; 6]; AGE_BANDS];
    for l in lives {
        let b = crate::cast!(l.age.div_euclid(band_days) => usize).min(AGE_BANDS - 1);
        match l.fate {
            Fate::Alive => {
                p.bands[b].living += 1;
                sums[b] += l.genome.0[t];
            }
            Fate::Died(c) => {
                p.bands[b].deaths += 1;
                if let Some(k) = CAUSES.iter().position(|&x| x == c) {
                    causes[b][k] += 1;
                }
            }
        }
    }
    for (b, band) in p.bands.iter_mut().enumerate() {
        band.trait_mean = (band.living >= 3).then(|| sums[b] / crate::cast!(band.living => f32));
        let (k, &n) = causes[b].iter().enumerate().fold((0, &0), |best, cur| if cur.1 > best.1 { cur } else { best });
        band.top = (n > 0).then(|| (CAUSES[k], n));
    }
    p
}

/// Living creatures needed before [`age_trend`] compares the young with the old.
pub const MIN_TREND_LIVING: usize = 20;

/// The mean trait of the youngest and the oldest fifth of the living, and the
/// standard error of their difference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AgeTrend {
    pub young: f32,
    pub old: f32,
    /// `sd × √(2 / n)` for the living trait's standard deviation `sd` and `n` per fifth.
    pub se: f32,
}

impl AgeTrend {
    /// The gap is more than twice its standard error: unlikely to be noise
    /// (roughly the 95% level), so worth putting into words.
    pub fn significant(self) -> bool {
        (self.old - self.young).abs() > 2.0 * self.se
    }
}

/// Compare the youngest and oldest fifth of the living, when there are at
/// least `MIN_TREND_LIVING` living.
pub fn age_trend(lives: &[Life], t: usize) -> Option<AgeTrend> {
    let mut living: Vec<&Life> = lives.iter().filter(|l| !l.dead()).collect();
    if living.len() < MIN_TREND_LIVING {
        return None;
    }
    living.sort_by_key(|l| l.age);
    let n = living.len().div_euclid(5);
    let mean = |s: &[&Life]| s.iter().map(|l| l.genome.0[t]).sum::<f32>() / crate::cast!(s.len() => f32);
    let all = mean(&living);
    let var = living.iter().map(|l| (l.genome.0[t] - all).powi(2)).sum::<f32>() / crate::cast!(living.len() - 1 => f32);
    let se = var.sqrt() * (2.0 / crate::cast!(n => f32)).sqrt();
    Some(AgeTrend { young: mean(living.get(..n)?), old: mean(living.get(living.len() - n..)?), se })
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests;
