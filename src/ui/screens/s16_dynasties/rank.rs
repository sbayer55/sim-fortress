//! The per-day rankings: every listed line, its standing in its region and
//! in the valley per stat, and every living animal's rank per stat within its
//! species.

use std::collections::BTreeMap;

use crate::sim::lineage::dynasties::{rank, DynastyView};
use crate::sim::{Sim, SpeciesId};

use super::Stat;

/// A line's rank per stat: `(rank, n)` in its region and among its species'
/// lines in the valley, and the valley mean and best per stat.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Standing {
    pub region: [(usize, usize); 6],
    pub valley: [(usize, usize); 6],
    pub valley_mean: [f32; 6],
    pub valley_best: [f32; 6],
    /// The best line per stat, as an index into the ranked list.
    pub valley_best_of: [usize; 6],
    /// Lines of the same species in the valley.
    pub kind_lines: usize,
}

/// Every living animal of one species: its stat values sorted descending, with
/// the mean and the best, for percentile lookups.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct LivingRanks {
    pub sorted: [Vec<f32>; 6],
    pub mean: [f32; 6],
    pub best: [f32; 6],
}

impl LivingRanks {
    /// `(rank, n)` of `v` among the species, 1-based.
    pub(super) fn rank(&self, stat: Stat, v: f32) -> (usize, usize) {
        let s = &self.sorted[stat.index()];
        rank_desc(s, v)
    }

    pub(super) const fn mean_of(&self, stat: Stat) -> f32 {
        self.mean[stat.index()]
    }

    pub(super) const fn best_of(&self, stat: Stat) -> f32 {
        self.best[stat.index()]
    }
}

/// The rankings for one sim day.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Ranked {
    pub day: u64,
    /// Lines with a living member, kills descending.
    pub dynasties: Vec<DynastyView>,
    /// Parallel to `dynasties`.
    pub standing: Vec<Standing>,
    pub living: BTreeMap<SpeciesId, LivingRanks>,
}

/// 1-based rank of `v` in a descending list, and the list's length.
pub(super) fn rank_desc(sorted_desc: &[f32], v: f32) -> (usize, usize) {
    let above = sorted_desc.iter().filter(|&&x| x > v).count();
    (above + 1, sorted_desc.len())
}

/// `top n%`: the share of the field at or above this rank, at least 1.
pub(super) fn top_pct(rank: usize, n: usize) -> u32 {
    if n == 0 {
        return 100;
    }
    crate::cast!((rank * 100).div_ceil(n).max(1) => u32)
}

fn mean(values: &[f32]) -> f32 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f32>() / crate::cast!(values.len() => f32)
    }
}

impl Ranked {
    pub(super) fn build(sim: &Sim) -> Self {
        let mut dynasties = rank(sim);
        dynasties.retain(|d| !d.members.is_empty());
        let standing = dynasties.iter().map(|d| standing_of(&dynasties, d)).collect();
        let living = living_ranks(&dynasties);
        Self { day: sim.time.day_index(), dynasties, standing, living }
    }
}

fn standing_of(all: &[DynastyView], d: &DynastyView) -> Standing {
    let mut st = Standing { region: [(1, 1); 6], valley: [(1, 1); 6], valley_mean: [0.0; 6], valley_best: [1.0; 6], valley_best_of: [0; 6], kind_lines: 0 };
    let same_region: Vec<&DynastyView> = all.iter().filter(|o| o.region == d.region).collect();
    let same_kind: Vec<(usize, &DynastyView)> = all.iter().enumerate().filter(|(_, o)| o.species == d.species).collect();
    st.kind_lines = same_kind.len();
    for stat in Stat::ALL {
        let k = stat.index();
        let v = stat.of(&d.totals);
        let mut region: Vec<f32> = same_region.iter().map(|o| stat.of(&o.totals)).collect();
        region.sort_by(|a, b| b.total_cmp(a));
        st.region[k] = rank_desc(&region, v);
        let mut valley: Vec<f32> = same_kind.iter().map(|(_, o)| stat.of(&o.totals)).collect();
        valley.sort_by(|a, b| b.total_cmp(a));
        st.valley[k] = rank_desc(&valley, v);
        st.valley_mean[k] = mean(&valley);
        let best = same_kind.iter().max_by(|(_, a), (_, b)| stat.of(&a.totals).total_cmp(&stat.of(&b.totals)));
        st.valley_best[k] = best.map_or(1.0, |(_, b)| stat.of(&b.totals)).max(1.0);
        st.valley_best_of[k] = best.map_or(0, |(i, _)| *i);
    }
    st
}

fn living_ranks(all: &[DynastyView]) -> BTreeMap<SpeciesId, LivingRanks> {
    let mut out: BTreeMap<SpeciesId, LivingRanks> = BTreeMap::new();
    for d in all {
        let entry = out.entry(d.species).or_default();
        for m in &d.members {
            for stat in Stat::ALL {
                entry.sorted[stat.index()].push(stat.of(&m.stats));
            }
        }
    }
    for r in out.values_mut() {
        for k in 0..6 {
            r.sorted[k].sort_by(|a, b| b.total_cmp(a));
            r.mean[k] = mean(&r.sorted[k]);
            r.best[k] = r.sorted[k].first().copied().unwrap_or(0.0).max(1.0);
        }
    }
    out
}
