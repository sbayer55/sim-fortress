//! Biomes: a Whittaker-style reading of each cell's climate, smoothed into
//! coherent patches.
//!
//! The table maps temperature and moisture to one of eight biomes. A
//! majority filter then removes single-cell speckle, and any patch still
//! smaller than `MIN_PATCH` is absorbed by the biome it shares the longest
//! border with, so every biome reads as an area with edges rather than a
//! per-cell label.

use serde::{Deserialize, Serialize};

use super::flow::Grid;

/// Patches smaller than this many cells (4-connected) are absorbed.
pub const MIN_PATCH: usize = 4;
/// Majority-filter passes over the raw labels.
const FILTER_PASSES: usize = 2;
/// Small-patch absorption rounds (each round can leave a merged patch that
/// is still small; a few rounds settle it).
const ABSORB_ROUNDS: usize = 4;
/// Land colder than this is tundra or taiga (about the coldest third).
const COLD: f32 = 0.18;
/// Land at least this warm is desert, savanna or hot forest (the warmest third).
const HOT: f32 = 0.50;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
#[serde(rename_all = "snake_case")]
pub enum Biome {
    Tundra = 0,
    Taiga = 1,
    TemperateForest = 2,
    Grassland = 3,
    Steppe = 4,
    Savanna = 5,
    Desert = 6,
    Wetland = 7,
}

impl Biome {
    pub const ALL: [Self; 8] = [
        Self::Tundra,
        Self::Taiga,
        Self::TemperateForest,
        Self::Grassland,
        Self::Steppe,
        Self::Savanna,
        Self::Desert,
        Self::Wetland,
    ];

    /// The Whittaker table: cold rows split by moisture into tundra and
    /// taiga, temperate rows run steppe → grassland → forest → wetland, and
    /// hot rows run desert → savanna → forest → wetland. The cuts sit on
    /// the land climate deciles of default worlds (temperature runs cold
    /// because of the lapse rate: its land median is about 0.33), so each
    /// biome covers a fair share of the map; retune with the ignored
    /// `print_biomes_and_regions` test.
    pub fn classify(temperature: f32, moisture: f32) -> Self {
        if temperature < COLD {
            if moisture < 0.42 {
                Self::Tundra
            } else {
                Self::Taiga
            }
        } else if temperature < HOT {
            if moisture < 0.30 {
                Self::Steppe
            } else if moisture < 0.50 {
                Self::Grassland
            } else if moisture < 0.75 {
                Self::TemperateForest
            } else {
                Self::Wetland
            }
        } else if moisture < 0.30 {
            Self::Desert
        } else if moisture < 0.55 {
            Self::Savanna
        } else if moisture < 0.75 {
            Self::TemperateForest
        } else {
            Self::Wetland
        }
    }

    /// `Biome as u8` back to `Biome`; out-of-range codes read as grassland.
    pub const fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Tundra,
            1 => Self::Taiga,
            2 => Self::TemperateForest,
            4 => Self::Steppe,
            5 => Self::Savanna,
            6 => Self::Desert,
            7 => Self::Wetland,
            _ => Self::Grassland,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Tundra => "tundra",
            Self::Taiga => "taiga",
            Self::TemperateForest => "temperate forest",
            Self::Grassland => "grassland",
            Self::Steppe => "steppe",
            Self::Savanna => "savanna",
            Self::Desert => "desert",
            Self::Wetland => "wetland",
        }
    }

    /// The noun a region named after this biome carries (`Northern Taiga`).
    /// Every noun is at most seven characters so a name fits sixteen cells.
    pub const fn region_noun(self) -> &'static str {
        match self {
            Self::Tundra => "Tundra",
            Self::Taiga => "Taiga",
            Self::TemperateForest => "Forest",
            Self::Grassland => "Plains",
            Self::Steppe => "Steppe",
            Self::Savanna => "Savanna",
            Self::Desert => "Desert",
            Self::Wetland => "Fens",
        }
    }

    /// Scale on a cell's vegetation cap: cold and parched biomes carry less
    /// standing biomass than the temperate ones, whatever the terrain.
    pub const fn vegetation_scale(self) -> f32 {
        match self {
            Self::Tundra | Self::Desert => 0.7,
            Self::Taiga | Self::Steppe => 0.85,
            Self::Savanna => 0.9,
            Self::TemperateForest | Self::Grassland | Self::Wetland => 1.0,
        }
    }

    /// Can the forest quantile place `Forest` terrain here? Tundra, steppe
    /// and desert stay treeless; the wettest of their cells become grass.
    pub const fn allows_forest(self) -> bool {
        !matches!(self, Self::Tundra | Self::Steppe | Self::Desert)
    }

    /// Too dry for a brook to run all year: its bed is a seasonal wash.
    pub const fn is_arid(self) -> bool {
        matches!(self, Self::Steppe | Self::Desert)
    }
}

/// Label every cell from the table, then smooth the labels into patches.
pub(super) fn label(grid: Grid, temperature: &[f32], moisture: &[f32]) -> Vec<Biome> {
    let mut labels: Vec<Biome> = temperature.iter().zip(moisture).map(|(&t, &m)| Biome::classify(t, m)).collect();
    let mut scratch = labels.clone();
    for _ in 0..FILTER_PASSES {
        majority_pass(grid, &labels, &mut scratch);
        std::mem::swap(&mut labels, &mut scratch);
    }
    for _ in 0..ABSORB_ROUNDS {
        if !absorb_small_patches(grid, &mut labels) {
            break;
        }
    }
    labels
}

/// One 3×3 majority pass: each cell takes the most common label around it,
/// keeping its own label on a tie it is part of (else the lowest biome).
fn majority_pass(grid: Grid, src: &[Biome], dst: &mut [Biome]) {
    for i in 0..grid.len() {
        let mut counts = [0u8; 8];
        counts[crate::cast!(src[i] => usize)] += 1;
        grid.for_neighbours(i, |j, _| counts[crate::cast!(src[j] => usize)] += 1);
        let best = counts.iter().copied().max().unwrap_or(0);
        let own = counts[crate::cast!(src[i] => usize)];
        dst[i] = if own == best {
            src[i]
        } else {
            let k = counts.iter().position(|&c| c == best).unwrap_or(0);
            Biome::from_code(crate::cast!(k => u8))
        };
    }
}

/// Relabel every 4-connected patch smaller than `MIN_PATCH` to the
/// neighbouring biome it shares the most border with. Returns whether
/// anything changed.
fn absorb_small_patches(grid: Grid, labels: &mut [Biome]) -> bool {
    let n = grid.len();
    let (w, h) = (grid.w, grid.h);
    let mut comp = vec![usize::MAX; n];
    let mut members: Vec<Vec<usize>> = Vec::new();
    let mut stack = Vec::new();
    for start in 0..n {
        if comp[start] != usize::MAX {
            continue;
        }
        let id = members.len();
        let mut cells = Vec::new();
        comp[start] = id;
        stack.push(start);
        while let Some(i) = stack.pop() {
            cells.push(i);
            for j in four_neighbours(i, w, h) {
                if comp[j] == usize::MAX && labels[j] == labels[i] {
                    comp[j] = id;
                    stack.push(j);
                }
            }
        }
        members.push(cells);
    }
    let mut changed = false;
    for cells in &members {
        if cells.len() >= MIN_PATCH {
            continue;
        }
        let own = labels[cells[0]];
        let mut border = [0u32; 8];
        for &i in cells {
            for j in four_neighbours(i, w, h) {
                if labels[j] != own {
                    border[crate::cast!(labels[j] => usize)] += 1;
                }
            }
        }
        let best = border.iter().copied().max().unwrap_or(0);
        if best == 0 {
            continue;
        }
        let k = border.iter().position(|&b| b == best).unwrap_or(0);
        let to = Biome::from_code(crate::cast!(k => u8));
        for &i in cells {
            labels[i] = to;
        }
        changed = true;
    }
    changed
}

/// In-bounds 4-neighbours of `i`, in a fixed order.
fn four_neighbours(i: usize, w: usize, h: usize) -> impl Iterator<Item = usize> {
    let (x, y) = (i % w, i.div_euclid(w));
    let up = (y > 0).then(|| i - w);
    let left = (x > 0).then(|| i - 1);
    let right = (x + 1 < w).then(|| i + 1);
    let down = (y + 1 < h).then(|| i + w);
    [up, left, right, down].into_iter().flatten()
}
