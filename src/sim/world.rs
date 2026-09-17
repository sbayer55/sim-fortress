//! Terrain grid: pure, deterministic world generation from a seed.
//!
//! Generation is a pipeline of sibling modules: `noise` supplies seeded
//! fields, `relief` builds a tectonic surface and ages it with a
//! landscape-evolution model (stream-power incision plus hillslope diffusion
//! for `age` epochs), `climate` sweeps a prevailing wind over it for
//! orographic rain and lays a temperature gradient, `flow` routes drainage
//! over the result, `classify` cuts water, rock, sand, forest and grass by
//! quantile so the percentage targets hold on any seed (reading each cell's
//! `biome` for what may grow there, widening rivers by drainage tier and
//! fanning deltas where a trunk meets the sea), and `regions` merges the
//! drainage basins into the eight named regions.

use serde::{Deserialize, Serialize};

use crate::sim::params::WorldParams;
use crate::sim::rng::Rng;

mod biome;
mod classify;
mod climate;
mod flow;
mod noise;
mod regions;
mod relief;
#[cfg(test)]
mod tests;

pub use biome::{Biome, MIN_PATCH};
pub use climate::Wind;
pub use regions::{NAME_MAX, REGION_COUNT};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
#[serde(rename_all = "snake_case")]
pub enum Terrain {
    DeepWater = 0,
    ShallowWater = 1,
    Sand = 2,
    Dirt = 3,
    GrassSparse = 4,
    Grass = 5,
    GrassDense = 6,
    Forest = 7,
    Rock = 8,
    /// Reed beds on flat, wet ground beside water: drinkable, slow to cross,
    /// dense cover.
    Marsh = 9,
}

impl Terrain {
    pub const fn is_water(self) -> bool {
        matches!(self, Self::DeepWater | Self::ShallowWater)
    }

    /// Map a serialised terrain code (`terrain as u8`, 0..=9) back to `Terrain`
    /// (C6 FR1 title-screen strips). Codes outside the range fall back to Rock.
    pub const fn from_code(code: u8) -> Self {
        match code {
            0 => Self::DeepWater,
            1 => Self::ShallowWater,
            2 => Self::Sand,
            3 => Self::Dirt,
            4 => Self::GrassSparse,
            5 => Self::Grass,
            6 => Self::GrassDense,
            7 => Self::Forest,
            9 => Self::Marsh,
            _ => Self::Rock,
        }
    }

    pub const fn walkable(self) -> bool {
        !matches!(self, Self::DeepWater | Self::Rock)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::DeepWater => "deep water",
            Self::ShallowWater => "shallow water",
            Self::Sand => "sand",
            Self::Dirt => "bare dirt",
            Self::GrassSparse => "sparse grass",
            Self::Grass => "grassland",
            Self::GrassDense => "meadow",
            Self::Forest => "forest",
            Self::Rock => "rock",
            Self::Marsh => "marsh",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub terrain: Terrain,
    /// The climate zone the cell lies in: what could grow here, and the
    /// tint the terrain is drawn with.
    pub biome: Biome,
    pub elevation: f32,
    pub moisture: f32,
    /// Climate temperature 0 (cold) ..= 1 (hot): latitude minus altitude.
    pub temperature: f32,
    /// Standing vegetation biomass 0..=1.
    pub vegetation: f32,
    /// Synthetic "how many prey pass through here" 0..=1 (fixture decoration only).
    pub prey_pressure: f32,
    /// Synthetic "how many predators pass through here" 0..=1 (fixture decoration only).
    pub pred_pressure: f32,
    /// Set to `Some(ShallowWater)` when a shallow-water cell dried to sand in a drought.
    pub dried_from: Option<Terrain>,
    /// C7: parasite contamination 0..=1, shed by carriers, decays daily.
    pub parasite_load: f32,
}

/// A named region's bounding box: (name, x0, y0, x1, y1), half-open.
///
/// Regions are drainage basins, so the box is only a bound: membership is
/// `World::region_index` / `World::region_cells`.
pub type RegionRect = (String, usize, usize, usize, usize);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct World {
    pub cells: Vec<Cell>,
    pub width: usize,
    pub height: usize,
    pub dens: Vec<(usize, usize)>,
    pub carcasses: Vec<(usize, usize)>,
    pub seeds: Vec<(usize, usize)>,
    pub regions: Vec<RegionRect>,
    /// Region index of every cell. Empty means "regions are plain
    /// rectangles": membership falls back to the boxes in `regions`.
    pub region_map: Vec<u8>,
    /// The prevailing wind that shaped the rain field.
    pub wind: Wind,
    /// Number of water cells at generation, used as the water-level series baseline.
    pub water_cells_at_generation: usize,
    /// Per cell: 8-adjacent to water, or marsh (a drinking spot). Refreshed by
    /// `refresh_shore` whenever water terrain changes; empty means "compute".
    pub shore: Vec<bool>,
    /// Waterfalls: rock cells a river drops over, sorted row-major `(x, y)`.
    /// The terrain is `Rock`; the list only changes how the cell is drawn
    /// and named.
    pub falls: Vec<(usize, usize)>,
}

impl World {
    /// Is `(x, y)` 8-adjacent to a water cell?
    pub fn is_shore(&self, x: usize, y: usize) -> bool {
        if self.shore.len() == self.cells.len() {
            return self.shore[y * self.width + x];
        }
        self.compute_shore(x, y)
    }

    fn compute_shore(&self, x: usize, y: usize) -> bool {
        // Marsh holds standing water of its own: a drinking spot in itself.
        if self.cell(x, y).terrain == Terrain::Marsh {
            return true;
        }
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (crate::cast!(x => i32) + dx, crate::cast!(y => i32) + dy);
                if self.in_bounds(nx, ny) && self.cell(crate::cast!(nx => usize), crate::cast!(ny => usize)).terrain.is_water() {
                    return true;
                }
            }
        }
        false
    }

    /// Recompute the shore cache (call after any water terrain change).
    pub fn refresh_shore(&mut self) {
        let mut shore = Vec::with_capacity(self.cells.len());
        for y in 0..self.height {
            for x in 0..self.width {
                shore.push(self.compute_shore(x, y));
            }
        }
        self.shore = shore;
    }

    pub fn cell(&self, x: usize, y: usize) -> &Cell {
        &self.cells[y * self.width + x]
    }

    /// Does a river fall over the rock at `(x, y)`?
    pub fn is_fall(&self, x: usize, y: usize) -> bool {
        self.falls.binary_search_by(|&(fx, fy)| (fy, fx).cmp(&(y, x))).is_ok()
    }

    /// The terrain name at `(x, y)`, naming a waterfall as such.
    pub fn terrain_name(&self, x: usize, y: usize) -> &'static str {
        if self.is_fall(x, y) {
            "waterfall"
        } else {
            self.cell(x, y).terrain.name()
        }
    }

    pub fn cell_mut(&mut self, x: usize, y: usize) -> &mut Cell {
        &mut self.cells[y * self.width + x]
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    pub const fn height(&self) -> usize {
        self.height
    }

    pub const fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (crate::cast!(x => usize)) < self.width && (crate::cast!(y => usize)) < self.height
    }

    pub fn region_name(&self, x: usize, y: usize) -> &str {
        self.region_at(x, y).map_or("The Wilds", |ri| self.regions[ri].0.as_str())
    }

    /// Index into `self.regions` covering `(x, y)`, or 0 as a fallback.
    pub fn region_index(&self, x: usize, y: usize) -> usize {
        self.region_at(x, y).unwrap_or(0)
    }

    fn region_at(&self, x: usize, y: usize) -> Option<usize> {
        if self.region_map.len() == self.cells.len() {
            let ri = usize::from(self.region_map[y * self.width + x]);
            return (ri < self.regions.len()).then_some(ri);
        }
        self.regions.iter().position(|(_, x0, y0, x1, y1)| x >= *x0 && x < *x1 && y >= *y0 && y < *y1)
    }

    /// Every cell of region `ri`, row-major.
    pub fn region_cells(&self, ri: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
        let (x0, y0, x1, y1) = self.regions.get(ri).map_or((0, 0, 0, 0), |r| (r.1, r.2, r.3.min(self.width), r.4.min(self.height)));
        (y0..y1).flat_map(move |y| (x0..x1).map(move |x| (x, y))).filter(move |&(x, y)| self.region_index(x, y) == ri)
    }

    /// The region's anchor cell for labels, camera moves and event
    /// positions: the member cell nearest the middle of its bounding box.
    pub fn region_centre(&self, ri: usize) -> (usize, usize) {
        let Some(r) = self.regions.get(ri) else { return (0, 0) };
        let (cx, cy) = ((r.1 + r.3).div_euclid(2), (r.2 + r.4).div_euclid(2));
        if self.region_map.len() != self.cells.len() {
            return (cx, cy);
        }
        // Rows count double: cells are twice as tall as they are wide.
        let key = |(x, y): (usize, usize)| x.abs_diff(cx).pow(2) + (2 * y.abs_diff(cy)).pow(2);
        self.region_cells(ri).min_by_key(|&p| key(p)).unwrap_or((cx, cy))
    }

    /// Do regions `a` and `b` share a border (a 4-adjacent cell pair)?
    pub fn regions_adjacent(&self, a: usize, b: usize) -> bool {
        if a == b || a >= self.regions.len() || b >= self.regions.len() {
            return false;
        }
        if self.region_map.len() != self.cells.len() {
            let (ra, rb) = (&self.regions[a], &self.regions[b]);
            let overlap_y = ra.2 < rb.4 && rb.2 < ra.4;
            let overlap_x = ra.1 < rb.3 && rb.1 < ra.3;
            return (ra.3 == rb.1 || rb.3 == ra.1) && overlap_y || (ra.4 == rb.2 || rb.4 == ra.2) && overlap_x;
        }
        let (small, other) = if self.region_size(a) <= self.region_size(b) { (a, b) } else { (b, a) };
        self.region_cells(small).any(|(x, y)| {
            (x > 0 && self.region_index(x - 1, y) == other)
                || (x + 1 < self.width && self.region_index(x + 1, y) == other)
                || (y > 0 && self.region_index(x, y - 1) == other)
                || (y + 1 < self.height && self.region_index(x, y + 1) == other)
        })
    }

    /// Cells in region `ri`.
    pub fn region_size(&self, ri: usize) -> usize {
        if self.region_map.len() == self.cells.len() {
            let code = crate::cast!(ri => u8);
            return self.region_map.iter().map(|&r| usize::from(r == code)).sum();
        }
        self.regions.get(ri).map_or(0, |r| (r.3 - r.1) * (r.4 - r.2))
    }

    /// Pure, deterministic world generation: seeded relief, aged by erosion,
    /// then classified by quantile so the `water_pct`/`forest_pct`/`rock_pct`
    /// targets are met on any seed.
    pub fn generate(seed: u64, params: &WorldParams) -> Self {
        let (w, h) = (params.width, params.height);
        let grid = flow::Grid { w, h };
        let mut rng = Rng::new(seed);
        let relief = relief::build(&mut rng, grid, params);
        let (cells, falls) = classify::cells(&mut rng, grid, &relief, params);
        let (regions, region_map) = regions::build(grid, &relief.basin, &relief.sea, &cells);

        let water_cells_at_generation = cells.iter().filter(|c| c.terrain.is_water()).count();
        let mut world = Self {
            cells,
            width: w,
            height: h,
            dens: Vec::new(),
            carcasses: Vec::new(),
            seeds: Vec::new(),
            regions,
            region_map,
            wind: relief.wind,
            water_cells_at_generation,
            shore: Vec::new(),
            falls,
        };
        world.refresh_shore();
        world
    }
}
