//! Terrain grid: pure, deterministic world generation from a seed.
//!
//! Generation is a pipeline of sibling modules: `noise` supplies seeded
//! fields, `relief` builds a tectonic surface and ages it with a
//! landscape-evolution model (stream-power incision plus hillslope diffusion
//! for `age` epochs), `climate` sweeps a prevailing wind over it for
//! orographic rain and lays a temperature gradient, `flow` routes drainage
//! over the result, and `classify` cuts water, rock, sand, forest and grass
//! by quantile so the percentage targets hold on any seed.

use serde::{Deserialize, Serialize};

use crate::sim::params::WorldParams;
use crate::sim::rng::Rng;

mod classify;
mod climate;
mod flow;
mod noise;
mod relief;
#[cfg(test)]
mod tests;

pub use climate::Wind;

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

/// A named rectangle: (name, x0, y0, x1, y1), half-open on the upper edges.
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
    /// The prevailing wind that shaped the rain field.
    pub wind: Wind,
    /// Number of water cells at generation, used as the water-level series baseline.
    pub water_cells_at_generation: usize,
    /// Per cell: 8-adjacent to water, or marsh (a drinking spot). Refreshed by
    /// `refresh_shore` whenever water terrain changes; empty means "compute".
    pub shore: Vec<bool>,
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
        self.regions
            .iter()
            .find(|(_, x0, y0, x1, y1)| x >= *x0 && x < *x1 && y >= *y0 && y < *y1)
            .map_or("The Wilds", |r| r.0.as_str())
    }

    /// Index into `self.regions` covering `(x, y)`, or 0 as a fallback.
    pub fn region_index(&self, x: usize, y: usize) -> usize {
        self.regions
            .iter()
            .position(|(_, x0, y0, x1, y1)| x >= *x0 && x < *x1 && y >= *y0 && y < *y1)
            .unwrap_or(0)
    }

    /// Pure, deterministic world generation: seeded relief, aged by erosion,
    /// then classified by quantile so the `water_pct`/`forest_pct`/`rock_pct`
    /// targets are met on any seed.
    pub fn generate(seed: u64, params: &WorldParams) -> Self {
        let (w, h) = (params.width, params.height);
        let grid = flow::Grid { w, h };
        let mut rng = Rng::new(seed);
        let relief = relief::build(&mut rng, grid, params);
        let cells = classify::cells(&mut rng, grid, &relief, params);

        let water_cells_at_generation = cells.iter().filter(|c| c.terrain.is_water()).count();
        let mut world = Self {
            cells,
            width: w,
            height: h,
            dens: Vec::new(),
            carcasses: Vec::new(),
            seeds: Vec::new(),
            regions: build_regions(w, h),
            wind: relief.wind,
            water_cells_at_generation,
            shore: Vec::new(),
        };
        world.refresh_shore();
        world
    }
}

/// The eight fixed regions, scaled from the 150×40 reference and extended to the
/// world edge so they tile the world.
fn build_regions(w: usize, h: usize) -> Vec<RegionRect> {
    const DEFS: [(&str, usize, usize, usize, usize); 8] = [
        ("Northmarch", 0, 0, 50, 14),
        ("Ashen Ridge", 50, 0, 100, 12),
        ("Sunfall Coast", 100, 0, 150, 16),
        ("Reedwater Vale", 0, 14, 50, 28),
        ("The Long Meadow", 50, 12, 100, 28),
        ("Lakeshore", 100, 16, 150, 40),
        ("Southern Thicket", 0, 28, 50, 40),
        ("Fenlands", 50, 28, 100, 40),
    ];
    let scale = |v: usize, src: usize, dst: usize| (crate::cast!((crate::cast!(v => f64) * crate::cast!(dst => f64) / crate::cast!(src => f64)).round() => usize)).min(dst);
    DEFS
        .iter()
        .map(|&(name, x0, y0, x1, y1)| {
            let sx0 = scale(x0, 150, w);
            let sy0 = scale(y0, 40, h);
            let sx1 = if x1 == 150 { w } else { scale(x1, 150, w) };
            let sy1 = if y1 == 40 { h } else { scale(y1, 40, h) };
            (name.to_string(), sx0, sy0, sx1, sy1)
        })
        .collect()
}

