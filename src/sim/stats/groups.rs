//! Group-size statistics (C8 follow-up): how large the herds and packs that
//! sociality actually produces get, and the distribution of their sizes.
//!
//! A **group is a spatial fact, not an entity**, matching C8's decision that a
//! pack has no identity. Clustering is a deterministic greedy pass over the
//! living set in `CreatureStore::living` (slot) order, mirroring the cohesion
//! rule that `wander`/`patrol` apply:
//!
//! - A creature below `social.cohesion_min` never herds, so it is always alone.
//! - Otherwise it joins the nearest same-species group whose centroid lies
//!   inside its own sense ellipse, while that group stays inside the
//!   `1.5 × preferred_group` dispersal band of [`SocialParams::herding`] (the
//!   joiner's visible neighbours are the group's existing members).
//! - With no such group it starts a new one.
//!
//! Every living creature belongs to exactly one group, so a group of one is an
//! animal on its own; [`GroupCensus::mean`] and [`GroupCensus::max`] report the
//! multi-member groups only, which is what "how large do herds get" means.
//!
//! The pass draws no RNG, so the checksum and the save format are untouched. The
//! census is recomputed from positions at the day boundary (and after a load)
//! and never serialised, so it reads as of the last midnight, like the species
//! census beside it.

use crate::sim::creatures::CreatureStore;
use crate::sim::geom;
use crate::sim::params::SocialParams;

/// Buckets in a species' group-size histogram: index `k` counts groups of
/// `k + 1` members, so index 0 is the animals that are alone. Sizes at or above
/// `GROUP_HIST` fold into the last bucket.
pub const GROUP_HIST: usize = 16;

/// Group-size census over the living set, per species in roster order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupCensus {
    /// Groups with two or more members.
    pub groups: Vec<u32>,
    /// Living creatures that belong to one of those groups.
    pub members: Vec<u32>,
    /// Mean size of the multi-member groups (`0.0` when none forms).
    pub mean: Vec<f32>,
    /// Largest group size (`0` when no group forms).
    pub max: Vec<u16>,
    /// Group-size distribution: `hist[species][k]` counts groups of `k + 1`
    /// members (index 0 = alone), with `GROUP_HIST` and above in the last bucket.
    pub hist: Vec<[u32; GROUP_HIST]>,
}

impl GroupCensus {
    /// An empty census for `n_species`.
    pub fn new(n_species: usize) -> Self {
        Self {
            groups: vec![0; n_species],
            members: vec![0; n_species],
            mean: vec![0.0; n_species],
            max: vec![0; n_species],
            hist: vec![[0; GROUP_HIST]; n_species],
        }
    }

    /// Lone animals of a species (groups of one), from the histogram.
    pub fn solo(&self, i: usize) -> u32 {
        self.hist[i][0]
    }

    /// Members of multi-member groups as a share of `population` (`0.0`..`1.0`).
    pub fn grouped_share(&self, i: usize, population: u32) -> f32 {
        if population == 0 {
            return 0.0;
        }
        crate::cast!(self.members[i] => f32) / crate::cast!(population => f32)
    }
}

/// One in-progress group: the sum of member positions and the member count.
/// The centroid is derived on use, exactly as `Kin` does.
struct Cluster {
    sx: f32,
    sy: f32,
    len: u32,
}

impl Cluster {
    const fn new(x: usize, y: usize) -> Self {
        Self { sx: crate::cast!(x => f32), sy: crate::cast!(y => f32), len: 1 }
    }

    fn add(&mut self, x: usize, y: usize) {
        self.sx += crate::cast!(x => f32);
        self.sy += crate::cast!(y => f32);
        self.len += 1;
    }

    /// The group's rounded centroid, the same convention `Kin::centroid` uses.
    fn centroid(&self) -> (usize, usize) {
        (
            crate::cast!((self.sx / crate::cast!(self.len => f32)).round().max(0.0) => usize),
            crate::cast!((self.sy / crate::cast!(self.len => f32)).round().max(0.0) => usize),
        )
    }

    /// Most same-species neighbours this sociality still holds together: the
    /// `herding` rule's `kin_count <= 1.5 × preferred_group` bound. A joiner's
    /// neighbours are the existing members, so this is a cap on `len`, not on
    /// the resulting group size.
    fn kin_cap(sp: &SocialParams, sociality: f32) -> f32 {
        1.5 * sp.preferred_group(sociality)
    }
}

/// Count the groups of every species from the living set's current positions.
pub fn group_census(store: &CreatureStore, sp: &SocialParams, n_species: usize) -> GroupCensus {
    // One cluster list per species, filled in `living()` (slot) order so the
    // greedy assignment is deterministic.
    let mut clusters: Vec<Vec<Cluster>> = (0..n_species).map(|_| Vec::new()).collect();
    for c in store.living() {
        let i = c.species.index();
        let list = &mut clusters[i];
        let sociality = c.genome.sociality();
        let join = if sociality >= sp.cohesion_min {
            nearest_group(c.x, c.y, c.genome.sense_cells(), Cluster::kin_cap(sp, sociality), list)
        } else {
            None
        };
        match join {
            Some(k) => list[k].add(c.x, c.y),
            None => list.push(Cluster::new(c.x, c.y)),
        }
    }
    reduce(&clusters)
}

/// The nearest cluster a creature of this sense range can reach without taking
/// its visible neighbours past `kin_cap` (the `herding` bound).
fn nearest_group(x: usize, y: usize, sense: u16, kin_cap: f32, clusters: &[Cluster]) -> Option<usize> {
    let r = f32::from(sense);
    let mut best: Option<(usize, f32)> = None;
    for (k, cl) in clusters.iter().enumerate() {
        // Joining makes the creature's same-species neighbours the existing
        // members, so `herding` is satisfied exactly when `len <= kin_cap`.
        if crate::cast!(cl.len => f32) > kin_cap {
            continue;
        }
        let (cx, cy) = cl.centroid();
        let d = geom::dist(x, y, cx, cy);
        if d > r {
            continue;
        }
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((k, d));
        }
    }
    best.map(|(k, _)| k)
}

/// Fold the per-species clusters into the census: histogram, group count, the
/// member total and the mean/max over the multi-member groups.
fn reduce(clusters: &[Vec<Cluster>]) -> GroupCensus {
    let mut out = GroupCensus::new(clusters.len());
    for (i, list) in clusters.iter().enumerate() {
        let mut sum = 0u32;
        for cl in list {
            let bucket = crate::cast!(cl.len.saturating_sub(1) => usize).min(GROUP_HIST - 1);
            out.hist[i][bucket] += 1;
            if cl.len >= 2 {
                out.groups[i] += 1;
                sum += cl.len;
                out.max[i] = out.max[i].max(crate::cast!(cl.len.min(u32::from(u16::MAX)) => u16));
            }
        }
        out.members[i] = sum;
        out.mean[i] = if out.groups[i] == 0 {
            0.0
        } else {
            crate::cast!(sum => f32) / crate::cast!(out.groups[i] => f32)
        };
    }
    out
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests;
