//! Names for the world's features: the ocean, the larger lakes, the rivers
//! (trunk stems and their main tributaries) and the rock ranges, each with
//! its member cells, so the ticker, the look cursor and the map can refer
//! to a place by name. `lexicon` draws the names, `chronicle` narrates the
//! generation from them.
//!
//! Detection is index-ordered and draws no randomness. The names come from
//! their own stream (`seed ^ NAME_SALT`), so the world checksum never
//! depends on them and a name never changes because an earlier stage
//! changed its draws. Draw order: the style, then per feature in detection
//! order (ocean, lakes by size, river stems by drainage, tributaries by
//! drainage, ranges by size) the stem syllables and, for rivers and
//! tributaries, the template variant.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::sim::rng::Rng;

use super::classify::{Bodies, Channel, Water};
use super::flow::{Donors, Grid, Tier};
use super::relief::Relief;
use super::{Biome, Cell, Terrain, World};

mod chronicle;
mod lexicon;
#[cfg(test)]
mod tests;

pub use chronicle::{chronicle, summary, Summary, CHRONICLE_LINES, CHRONICLE_WIDTH};
pub use lexicon::NAME_FULL_MAX;

/// Salt on the world seed for the naming stream.
const NAME_SALT: u64 = 0x6E61_6D65_735F_7631;
/// Feature index of a cell that belongs to no named feature.
pub const NO_FEATURE: u16 = u16::MAX;
/// At most this many named rivers (stems and tributaries together).
const RIVER_CAP: usize = 12;
/// A river-tier stem or a tributary shorter than this stays unnamed; a
/// trunk is always named.
const STEM_MIN_CELLS: usize = 4;
/// A lake needs this many cells to earn a name.
const LAKE_MIN_CELLS: usize = 6;
const LAKE_CAP: usize = 8;
/// A rock cluster needs this many cells to be a range.
const RANGE_MIN_CELLS: usize = 8;
const RANGE_CAP: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureKind {
    Ocean,
    Lake,
    /// A river stem: a trunk, or a river-tier channel reaching the sea.
    River,
    /// A river-tier channel joining a stem.
    Tributary,
    Range,
}

impl FeatureKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ocean => "sea",
            Self::Lake => "lake",
            Self::River => "river",
            Self::Tributary => "tributary",
            Self::Range => "range",
        }
    }

    pub const fn is_water(self) -> bool {
        !matches!(self, Self::Range)
    }

    pub const fn is_river(self) -> bool {
        matches!(self, Self::River | Self::Tributary)
    }
}

/// One named feature.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feature {
    pub kind: FeatureKind,
    /// The display name: "the Ulmar Sea", "Lake Tavos", "the Kelderwater",
    /// "Brenna Brook", "the Skarn Fells".
    pub name: String,
    /// Member cells: a river's channel length, a lake's or range's area.
    pub cells: u32,
    /// Where a label goes: a river's mid-stem, otherwise the member nearest
    /// the centroid.
    pub anchor: (usize, usize),
    /// A river's head; the anchor otherwise.
    pub source: (usize, usize),
    /// The cell a river drains into (the mouth itself at the edge or in a
    /// pit); the anchor otherwise.
    pub mouth: (usize, usize),
    /// A tributary's stem, as an index into `Names::features`.
    pub parent: Option<u16>,
}

/// The named features of a world and which cell belongs to which.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Names {
    /// The syllable style the names are drawn in.
    pub style: u8,
    /// Ocean, lakes by size, river stems by drainage, tributaries by
    /// drainage, ranges by size.
    pub features: Vec<Feature>,
    /// Feature index of every cell, `NO_FEATURE` elsewhere. Empty means the
    /// world has no names (hand-built test worlds).
    pub map: Vec<u16>,
}

/// What the naming pass reads.
pub(super) struct Source<'a> {
    pub(super) grid: Grid,
    pub(super) relief: &'a Relief,
    pub(super) bodies: &'a Bodies,
    pub(super) cells: &'a [Cell],
}

/// A feature before it has a name.
struct Found {
    kind: FeatureKind,
    members: Vec<usize>,
    source: usize,
    mouth: usize,
    parent: Option<u16>,
    /// A range's noun ("Hills"); empty for the rest.
    noun: &'static str,
}

impl Found {
    fn area(kind: FeatureKind, members: Vec<usize>, grid: Grid) -> Self {
        let anchor = nearest_centroid(&members, grid);
        Self { kind, members, source: anchor, mouth: anchor, parent: None, noun: "" }
    }

    fn feature(&self, name: String, grid: Grid) -> Feature {
        let at = |i: usize| (i % grid.w, i.div_euclid(grid.w));
        let anchor = if self.kind.is_river() {
            self.members.get(self.members.len().div_euclid(2)).copied().unwrap_or(self.source)
        } else {
            nearest_centroid(&self.members, grid)
        };
        Feature {
            kind: self.kind,
            name,
            cells: crate::cast!(self.members.len() => u32),
            anchor: at(anchor),
            source: at(self.source),
            mouth: at(self.mouth),
            parent: self.parent,
        }
    }
}

/// Name the features of a world.
pub(super) fn build(seed: u64, src: &Source<'_>) -> Names {
    let mut map = vec![NO_FEATURE; src.grid.len()];
    let mut found = Vec::new();
    ocean(src, &mut found, &mut map);
    lakes(src, &mut found, &mut map);
    rivers(src, &mut found, &mut map);
    ranges(src, &mut found, &mut map);

    let mut rng = Rng::new(seed ^ NAME_SALT);
    let style = rng.below(lexicon::STYLES);
    let mut used = BTreeSet::new();
    let features = found
        .iter()
        .map(|f| {
            let name = lexicon::full_name(&mut rng, style, &mut used, f.kind, f.noun);
            f.feature(name, src.grid)
        })
        .collect();
    Names { style: crate::cast!(style => u8), features, map }
}

/// Claim `members` for the feature about to be pushed.
fn claim(found: &mut Vec<Found>, map: &mut [u16], f: Found) {
    let id = crate::cast!(found.len() => u16);
    for &i in &f.members {
        map[i] = id;
    }
    found.push(f);
}

fn ocean(src: &Source<'_>, found: &mut Vec<Found>, map: &mut [u16]) {
    let members: Vec<usize> = (0..src.grid.len()).filter(|&i| src.bodies.water[i] == Water::Ocean).collect();
    if !members.is_empty() {
        claim(found, map, Found::area(FeatureKind::Ocean, members, src.grid));
    }
}

fn lakes(src: &Source<'_>, found: &mut Vec<Found>, map: &mut [u16]) {
    let mut parts = components(src.grid, |i| src.bodies.water[i] == Water::Lake);
    parts.retain(|p| p.len() >= LAKE_MIN_CELLS);
    for members in parts.into_iter().take(LAKE_CAP) {
        claim(found, map, Found::area(FeatureKind::Lake, members, src.grid));
    }
}

fn ranges(src: &Source<'_>, found: &mut Vec<Found>, map: &mut [u16]) {
    let rock: Vec<usize> = (0..src.grid.len()).filter(|&i| src.cells[i].terrain == Terrain::Rock).collect();
    let mean_rock = mean_height(src, &rock);
    let mut parts = components(src.grid, |i| src.cells[i].terrain == Terrain::Rock && map[i] == NO_FEATURE);
    parts.retain(|p| p.len() >= RANGE_MIN_CELLS);
    for members in parts.into_iter().take(RANGE_CAP) {
        let cold = members.iter().filter(|&&i| matches!(src.cells[i].biome, Biome::Tundra | Biome::Taiga)).count();
        let noun = if cold * 2 > members.len() {
            "Fells"
        } else if mean_height(src, &members) >= mean_rock {
            "Mountains"
        } else {
            "Hills"
        };
        let mut f = Found::area(FeatureKind::Range, members, src.grid);
        f.noun = noun;
        claim(found, map, f);
    }
}

fn mean_height(src: &Source<'_>, cells: &[usize]) -> f32 {
    if cells.is_empty() {
        return 0.0;
    }
    cells.iter().map(|&i| src.relief.height[i]).sum::<f32>() / crate::cast!(cells.len() => f32)
}

/// Stems from every mouth, largest drainage first, then the tributaries
/// that join them, then the banks and fans borrowing their river's name.
fn rivers(src: &Source<'_>, found: &mut Vec<Found>, map: &mut [u16]) {
    let donors = Donors::new(&src.relief.recv);
    let mut mouths: Vec<usize> = (0..src.grid.len())
        .filter(|&i| {
            let r = src.relief.recv[i];
            tier(src, i).is_some_and(|t| t >= Tier::River) && (r == i || tier(src, r).is_none())
        })
        .collect();
    mouths.sort_by(|&a, &b| src.relief.acc[b].total_cmp(&src.relief.acc[a]).then(a.cmp(&b)));

    let mut joins = Vec::new();
    for m in mouths {
        if map[m] != NO_FEATURE {
            continue;
        }
        let id = crate::cast!(found.len() => u16);
        let mut local = Vec::new();
        let stem = walk_stem(src, &donors, m, map, id, &mut local);
        if tier(src, m) != Some(Tier::Trunk) && stem.len() < STEM_MIN_CELLS {
            unclaim(map, &stem);
            continue;
        }
        joins.extend(local);
        push_stem(found, FeatureKind::River, stem, src.relief.recv[m], None);
    }

    joins.sort_by(|&(a, _), &(b, _)| src.relief.acc[b].total_cmp(&src.relief.acc[a]).then(a.cmp(&b)));
    for (start, parent) in joins {
        if found.iter().filter(|f| f.kind.is_river()).count() >= RIVER_CAP {
            break;
        }
        if map[start] != NO_FEATURE {
            continue;
        }
        let id = crate::cast!(found.len() => u16);
        let stem = walk_stem(src, &donors, start, map, id, &mut Vec::new());
        if stem.len() < STEM_MIN_CELLS {
            unclaim(map, &stem);
            continue;
        }
        push_stem(found, FeatureKind::Tributary, stem, src.relief.recv[start], Some(parent));
    }
    borrow_banks(src, map);
}

fn push_stem(found: &mut Vec<Found>, kind: FeatureKind, stem: Vec<usize>, mouth: usize, parent: Option<u16>) {
    let source = stem.last().copied().unwrap_or(mouth);
    found.push(Found { kind, members: stem, source, mouth, parent, noun: "" });
}

fn tier(src: &Source<'_>, i: usize) -> Option<Tier> {
    match src.bodies.channel[i] {
        Channel::Course(t) => Some(t),
        _ => None,
    }
}

fn unclaim(map: &mut [u16], cells: &[usize]) {
    for &i in cells {
        map[i] = NO_FEATURE;
    }
}

/// Climb from `start` along the unclaimed channel donor with the most
/// drainage, claiming each cell for `id`; every other river-tier donor
/// met on the way is recorded in `joins` as a tributary candidate.
fn walk_stem(src: &Source<'_>, donors: &Donors, start: usize, map: &mut [u16], id: u16, joins: &mut Vec<(usize, u16)>) -> Vec<usize> {
    let mut stem = Vec::new();
    let mut c = start;
    loop {
        map[c] = id;
        stem.push(c);
        let open: Vec<usize> = donors.of(c).iter().copied().filter(|&d| map[d] == NO_FEATURE && tier(src, d).is_some()).collect();
        let Some(next) = open.iter().copied().max_by(|&a, &b| src.relief.acc[a].total_cmp(&src.relief.acc[b]).then(b.cmp(&a))) else {
            return stem;
        };
        joins.extend(open.iter().filter(|&&d| d != next && tier(src, d).is_some_and(|t| t >= Tier::River)).map(|&d| (d, id)));
        c = next;
    }
}

/// Banks and fans take the name of the channel beside them (two passes,
/// so a trunk's outer bank reaches its river through the inner one).
fn borrow_banks(src: &Source<'_>, map: &mut [u16]) {
    for pass in 0..2 {
        for i in 0..src.grid.len() {
            if map[i] != NO_FEATURE || !matches!(src.bodies.channel[i], Channel::Bank | Channel::Fan) {
                continue;
            }
            let mut best = None;
            src.grid.for_neighbours(i, |j, _| {
                let river = if pass == 0 { tier(src, j).is_some() } else { src.bodies.channel[j] != Channel::None };
                if best.is_none() && map[j] != NO_FEATURE && river {
                    best = Some(map[j]);
                }
            });
            if let Some(b) = best {
                map[i] = b;
            }
        }
    }
}

/// The 8-connected components of the cells `pred` accepts, largest first
/// (ties by first cell), each listed in discovery order.
fn components(grid: Grid, pred: impl Fn(usize) -> bool) -> Vec<Vec<usize>> {
    let n = grid.len();
    let mut seen = vec![false; n];
    let mut parts = Vec::new();
    for start in 0..n {
        if seen[start] || !pred(start) {
            continue;
        }
        let mut members = vec![start];
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            grid.for_neighbours(i, |j, _| {
                if !seen[j] && pred(j) {
                    seen[j] = true;
                    stack.push(j);
                    members.push(j);
                }
            });
        }
        parts.push(members);
    }
    parts.sort_by_key(|p| (std::cmp::Reverse(p.len()), p.first().copied().unwrap_or(0)));
    parts
}

/// The member nearest the members' centroid (rows count double).
fn nearest_centroid(members: &[usize], grid: Grid) -> usize {
    let n = members.len().max(1);
    let (sx, sy) = members.iter().fold((0usize, 0usize), |(sx, sy), &i| (sx + i % grid.w, sy + i.div_euclid(grid.w)));
    let (cx, cy) = (sx.div_euclid(n), sy.div_euclid(n));
    let key = |i: usize| (i % grid.w).abs_diff(cx).pow(2) + (2 * i.div_euclid(grid.w).abs_diff(cy)).pow(2);
    members.iter().copied().min_by_key(|&i| key(i)).unwrap_or(0)
}

impl World {
    /// The named feature covering `(x, y)`, if any.
    pub fn feature_at(&self, x: usize, y: usize) -> Option<&Feature> {
        if self.names.map.len() != self.cells.len() {
            return None;
        }
        let k = self.names.map[y * self.width + x];
        if k == NO_FEATURE {
            return None;
        }
        self.names.features.get(usize::from(k))
    }

    /// The feature at `(x, y)` or beside it: the cell's own, else the first
    /// water feature among its neighbours, else the first range.
    pub fn feature_near(&self, x: usize, y: usize) -> Option<&Feature> {
        if let Some(f) = self.feature_at(x, y) {
            return Some(f);
        }
        let (x, y) = (crate::cast!(x => i32), crate::cast!(y => i32));
        let around = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];
        let near = around
            .iter()
            .filter(|&&(dx, dy)| self.in_bounds(x + dx, y + dy))
            .filter_map(|&(dx, dy)| self.feature_at(crate::cast!(x + dx => usize), crate::cast!(y + dy => usize)));
        let mut range = None;
        for f in near {
            if f.kind.is_water() {
                return Some(f);
            }
            range = range.or(Some(f));
        }
        range
    }

    /// The name of the feature at `(x, y)`, if any.
    pub fn feature_name(&self, x: usize, y: usize) -> Option<&str> {
        self.feature_at(x, y).map(|f| f.name.as_str())
    }

    /// Every feature of `kind`, largest first.
    pub fn features_of_kind(&self, kind: FeatureKind) -> impl Iterator<Item = &Feature> + '_ {
        self.names.features.iter().filter(move |f| f.kind == kind)
    }

    /// Where `(x, y)` is, for a sentence: "by Lake Ulmar", "in the Skarn
    /// Fells", else "in <region>".
    pub fn place_name(&self, x: usize, y: usize) -> String {
        match self.feature_near(x, y) {
            Some(f) if f.kind.is_water() => format!("by {}", f.name),
            Some(f) => format!("in {}", f.name),
            None => format!("in {}", self.region_name(x, y)),
        }
    }
}
