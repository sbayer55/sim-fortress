//! Terrain grid: pure, deterministic world generation from a seed.

use serde::{Deserialize, Serialize};

use crate::sim::params::WorldParams;
use crate::sim::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
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
}

impl Terrain {
    pub fn is_water(self) -> bool {
        matches!(self, Terrain::DeepWater | Terrain::ShallowWater)
    }

    pub fn walkable(self) -> bool {
        !matches!(self, Terrain::DeepWater | Terrain::Rock)
    }

    pub fn name(self) -> &'static str {
        match self {
            Terrain::DeepWater => "deep water",
            Terrain::ShallowWater => "shallow water",
            Terrain::Sand => "sand",
            Terrain::Dirt => "bare dirt",
            Terrain::GrassSparse => "sparse grass",
            Terrain::Grass => "grassland",
            Terrain::GrassDense => "meadow",
            Terrain::Forest => "forest",
            Terrain::Rock => "rock",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub terrain: Terrain,
    pub elevation: f32,
    pub moisture: f32,
    /// Standing vegetation biomass 0..=1.
    pub vegetation: f32,
    /// Synthetic "how many prey pass through here" 0..=1 (fixture decoration only).
    pub prey_pressure: f32,
    /// Synthetic "how many predators pass through here" 0..=1 (fixture decoration only).
    pub pred_pressure: f32,
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
}

impl World {
    pub fn cell(&self, x: usize, y: usize) -> &Cell {
        &self.cells[y * self.width + x]
    }

    pub fn cell_mut(&mut self, x: usize, y: usize) -> &mut Cell {
        &mut self.cells[y * self.width + x]
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height
    }

    pub fn region_name(&self, x: usize, y: usize) -> &str {
        self.regions
            .iter()
            .find(|(_, x0, y0, x1, y1)| x >= *x0 && x < *x1 && y >= *y0 && y < *y1)
            .map(|r| r.0.as_str())
            .unwrap_or("The Wilds")
    }

    /// Pure, deterministic world generation. Terrain thresholds are quantiles so
    /// the `water_pct`/`forest_pct`/`rock_pct` targets are met on any seed.
    pub fn generate(seed: u64, params: &WorldParams) -> World {
        let (w, h) = (params.width, params.height);
        let mut rng = Rng::new(seed);
        let e1 = Noise::new(&mut rng, 22.0, w, h);
        let e2 = Noise::new(&mut rng, 9.0, w, h);
        let e3 = Noise::new(&mut rng, 4.0, w, h);
        let m1 = Noise::new(&mut rng, 18.0, w, h);
        let m2 = Noise::new(&mut rng, 6.0, w, h);
        let v1 = Noise::new(&mut rng, 5.0, w, h);

        let total = w * h;
        let mut cells = Vec::with_capacity(total);
        for y in 0..h {
            for x in 0..w {
                // Noise is sampled in natural coordinates; river/lake features are
                // expressed in reference (150×40) coordinates scaled to the world.
                let (nx, ny) = (x as f32, y as f32 * 2.0);
                let (rx, ry) = (x as f32 * 150.0 / w as f32, y as f32 * 2.0 * 40.0 / h as f32);
                let mut elevation = 0.6 * e1.at(nx, ny) + 0.3 * e2.at(nx, ny) + 0.1 * e3.at(nx, ny);
                let river = ((rx * 0.5 - ry * 0.35 - 12.0).sin() * 6.0 + (rx - 75.0) * 0.26 - (ry - 40.0)).abs();
                if river < 3.0 {
                    elevation -= (3.0 - river) * 0.09;
                }
                let lake = ((rx - 122.0).powi(2) / 180.0 + (ry - 58.0).powi(2) / 140.0).sqrt();
                if lake < 1.2 {
                    elevation -= (1.2 - lake) * 0.5;
                }
                let elevation = elevation.clamp(0.0, 1.0);
                let moisture = (0.65 * m1.at(nx, ny) + 0.35 * m2.at(nx, ny) + (0.5 - elevation) * 0.5).clamp(0.0, 1.0);
                cells.push(Cell {
                    terrain: Terrain::Dirt,
                    elevation,
                    moisture,
                    vegetation: 0.0,
                    prey_pressure: 0.0,
                    pred_pressure: 0.0,
                });
            }
        }

        // Quantile thresholds. Sort indices by elevation ascending.
        let mut order: Vec<usize> = (0..total).collect();
        order.sort_by(|&a, &b| {
            cells[a]
                .elevation
                .partial_cmp(&cells[b].elevation)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.cmp(&b))
        });

        let water_count = ((params.water_pct as usize * total) / 100).min(total);
        let rock_count = ((params.rock_pct as usize * total) / 100).min(total);
        // Fixed 4 % sand band just above the water line.
        let sand_count = ((4 * total) / 100).min(total.saturating_sub(water_count).saturating_sub(rock_count));
        let deep_count = water_count * 2 / 3;
        let forest_count = (params.forest_pct as usize * total) / 100;

        for &i in &order[0..water_count] {
            cells[i].terrain = Terrain::ShallowWater;
        }
        for &i in &order[0..deep_count] {
            cells[i].terrain = Terrain::DeepWater;
        }
        for &i in &order[total - rock_count..total] {
            cells[i].terrain = Terrain::Rock;
        }
        for &i in &order[water_count..water_count + sand_count] {
            cells[i].terrain = Terrain::Sand;
        }

        // Forest: top `forest_pct` (of all cells) of the remaining land by moisture.
        let land: Vec<usize> = order[water_count + sand_count..total - rock_count].to_vec();
        let mut by_moisture = land.clone();
        by_moisture.sort_by(|&a, &b| {
            cells[b]
                .moisture
                .partial_cmp(&cells[a].moisture)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.cmp(&b))
        });
        let forest_take = forest_count.min(land.len());
        for &i in &by_moisture[0..forest_take] {
            cells[i].terrain = Terrain::Forest;
        }

        // Everything else is Dirt / GrassSparse / Grass / GrassDense by moisture bands.
        for i in 0..total {
            if cells[i].terrain == Terrain::Dirt {
                let m = cells[i].moisture;
                cells[i].terrain = if m < 0.28 {
                    Terrain::Dirt
                } else if m < 0.42 {
                    Terrain::GrassSparse
                } else if m < 0.58 {
                    Terrain::Grass
                } else {
                    Terrain::GrassDense
                };
            }
        }

        // Initial vegetation per terrain (static in C1).
        for i in 0..total {
            let (x, y) = (i % w, i / w);
            let v = v1.at(x as f32, y as f32 * 2.0);
            cells[i].vegetation = match cells[i].terrain {
                Terrain::DeepWater | Terrain::ShallowWater | Terrain::Rock => 0.0,
                Terrain::Sand => 0.05 * v,
                Terrain::Dirt => 0.15 * v + 0.05,
                Terrain::GrassSparse => 0.25 + 0.2 * v,
                Terrain::Grass => 0.45 + 0.25 * v,
                Terrain::GrassDense => 0.65 + 0.3 * v,
                Terrain::Forest => 0.55 + 0.25 * v,
            }
            .clamp(0.0, 1.0);
        }

        World {
            cells,
            width: w,
            height: h,
            dens: Vec::new(),
            carcasses: Vec::new(),
            seeds: Vec::new(),
            regions: build_regions(w, h),
        }
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
    let scale = |v: usize, src: usize, dst: usize| ((v as f64 * dst as f64 / src as f64).round() as usize).min(dst);
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

/// Smooth value noise on a coarse lattice.
struct Noise {
    lattice: Vec<f32>,
    lw: usize,
    lh: usize,
    scale: f32,
}

impl Noise {
    fn new(rng: &mut Rng, scale: f32, w: usize, h: usize) -> Self {
        let lw = (w as f32 / scale).ceil() as usize + 2;
        let lh = (h as f32 / scale).ceil() as usize + 2;
        let lattice = (0..lw * lh).map(|_| rng.f32()).collect();
        Noise { lattice, lw, lh, scale }
    }

    fn at(&self, x: f32, y: f32) -> f32 {
        let fx = x / self.scale;
        let fy = y / self.scale;
        let x0 = fx.floor() as usize;
        let y0 = fy.floor() as usize;
        let tx = smooth(fx - x0 as f32);
        let ty = smooth(fy - y0 as f32);
        let g = |x: usize, y: usize| self.lattice[(y.min(self.lh - 1)) * self.lw + x.min(self.lw - 1)];
        let a = g(x0, y0) + (g(x0 + 1, y0) - g(x0, y0)) * tx;
        let b = g(x0, y0 + 1) + (g(x0 + 1, y0 + 1) - g(x0, y0 + 1)) * tx;
        a + (b - a) * ty
    }
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(world: &World) -> [usize; 9] {
        let mut c = [0usize; 9];
        for cell in &world.cells {
            c[cell.terrain as usize] += 1;
        }
        c
    }

    #[test]
    fn size_bounds() {
        for (w, h) in [(100usize, 30usize), (200, 60), (150, 40)] {
            let world = World::generate(1, &WorldParams { width: w, height: h, ..WorldParams::default() });
            assert_eq!(world.width, w);
            assert_eq!(world.height, h);
            assert_eq!(world.cells.len(), w * h);
        }
    }

    #[test]
    fn target_percentages_20_seeds() {
        let params = WorldParams::default();
        let total = params.width * params.height;
        for seed in 1..=20 {
            let world = World::generate(seed, &params);
            let c = counts(&world);
            let pct = |n: usize| n as f32 / total as f32 * 100.0;
            let water = pct(c[0] + c[1]);
            let forest = pct(c[7]);
            let rock = pct(c[8]);
            assert!((water - 20.0).abs() < 3.0, "seed {seed} water {water:.2}");
            assert!((forest - 15.0).abs() < 3.0, "seed {seed} forest {forest:.2}");
            assert!((rock - 5.0).abs() < 3.0, "seed {seed} rock {rock:.2}");
        }
    }

    #[test]
    fn regions_cover_world() {
        for (w, h) in [(150usize, 40usize), (200, 60), (100, 30)] {
            let world = World::generate(7, &WorldParams { width: w, height: h, ..WorldParams::default() });
            // Every cell is covered by exactly one region (no gaps, no overlap).
            let area: usize = world.regions.iter().map(|r| (r.3 - r.1) * (r.4 - r.2)).sum();
            assert_eq!(area, w * h, "{w}x{h}");
            for y in 0..h {
                for x in 0..w {
                    assert_ne!(world.region_name(x, y), "The Wilds", "uncovered ({x},{y})");
                }
            }
        }
    }
}
