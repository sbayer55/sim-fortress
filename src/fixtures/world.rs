//! Static terrain grid built from layered value noise.

use super::rng::Rng;

pub const W: usize = 150;
pub const H: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Terrain {
    DeepWater,
    ShallowWater,
    Sand,
    Dirt,
    GrassSparse,
    Grass,
    GrassDense,
    Forest,
    Rock,
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

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub terrain: Terrain,
    pub elevation: f32,
    pub moisture: f32,
    /// Standing vegetation biomass 0..=1.
    pub vegetation: f32,
    /// Synthetic "how many prey pass through here" 0..=1.
    pub prey_pressure: f32,
    /// Synthetic "how many predators pass through here" 0..=1.
    pub pred_pressure: f32,
}

pub struct World {
    pub cells: Vec<Cell>,
    pub dens: Vec<(usize, usize)>,
    pub carcasses: Vec<(usize, usize)>,
    pub seeds: Vec<(usize, usize)>,
    /// Named regions used by the ecology/scarcity tables: (name, x0, y0, x1, y1).
    pub regions: Vec<(&'static str, usize, usize, usize, usize)>,
}

impl World {
    pub fn cell(&self, x: usize, y: usize) -> &Cell {
        &self.cells[y * W + x]
    }
    pub fn width(&self) -> usize {
        W
    }
    pub fn height(&self) -> usize {
        H
    }
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < W && (y as usize) < H
    }
    pub fn region_name(&self, x: usize, y: usize) -> &'static str {
        self.regions
            .iter()
            .find(|(_, x0, y0, x1, y1)| x >= *x0 && x < *x1 && y >= *y0 && y < *y1)
            .map(|r| r.0)
            .unwrap_or("The Wilds")
    }
}

/// Smooth value noise on a coarse lattice.
struct Noise {
    lattice: Vec<f32>,
    lw: usize,
    lh: usize,
    scale: f32,
}

impl Noise {
    fn new(rng: &mut Rng, scale: f32) -> Self {
        let lw = (W as f32 / scale).ceil() as usize + 2;
        let lh = (H as f32 / scale).ceil() as usize + 2;
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

pub fn generate(seed: u64) -> World {
    let mut rng = Rng::new(seed);
    let e1 = Noise::new(&mut rng, 22.0);
    let e2 = Noise::new(&mut rng, 9.0);
    let e3 = Noise::new(&mut rng, 4.0);
    let m1 = Noise::new(&mut rng, 18.0);
    let m2 = Noise::new(&mut rng, 6.0);
    let v1 = Noise::new(&mut rng, 5.0);

    let mut cells = Vec::with_capacity(W * H);
    for y in 0..H {
        for x in 0..W {
            let (fx, fy) = (x as f32, y as f32 * 2.0); // cells are ~2:1, stretch y
            let mut elevation = 0.6 * e1.at(fx, fy) + 0.3 * e2.at(fx, fy) + 0.1 * e3.at(fx, fy);
            // A river valley running diagonally, and a lake in the south-east.
            let river = ((fx * 0.5 - fy * 0.35 - 12.0).sin() * 6.0 + (fx - 75.0) * 0.26 - (fy - 40.0)).abs();
            if river < 3.0 {
                elevation -= (3.0 - river) * 0.09;
            }
            let lake = ((fx - 122.0).powi(2) / 180.0 + (fy - 58.0).powi(2) / 140.0).sqrt();
            if lake < 1.2 {
                elevation -= (1.2 - lake) * 0.5;
            }
            let elevation = elevation.clamp(0.0, 1.0);
            let moisture = (0.65 * m1.at(fx, fy) + 0.35 * m2.at(fx, fy) + (0.5 - elevation) * 0.5).clamp(0.0, 1.0);
            let terrain = if elevation < 0.24 {
                Terrain::DeepWater
            } else if elevation < 0.31 {
                Terrain::ShallowWater
            } else if elevation < 0.35 {
                Terrain::Sand
            } else if elevation > 0.80 {
                Terrain::Rock
            } else if moisture < 0.28 {
                Terrain::Dirt
            } else if moisture < 0.42 {
                Terrain::GrassSparse
            } else if moisture < 0.58 {
                Terrain::Grass
            } else if moisture < 0.70 {
                Terrain::GrassDense
            } else {
                Terrain::Forest
            };
            let vegetation = match terrain {
                Terrain::DeepWater | Terrain::ShallowWater | Terrain::Rock => 0.0,
                Terrain::Sand => 0.05 * v1.at(fx, fy),
                Terrain::Dirt => 0.15 * v1.at(fx, fy) + 0.05,
                Terrain::GrassSparse => 0.25 + 0.2 * v1.at(fx, fy),
                Terrain::Grass => 0.45 + 0.25 * v1.at(fx, fy),
                Terrain::GrassDense => 0.65 + 0.3 * v1.at(fx, fy),
                Terrain::Forest => 0.55 + 0.25 * v1.at(fx, fy),
            };
            cells.push(Cell {
                terrain,
                elevation,
                moisture,
                vegetation: vegetation.clamp(0.0, 1.0),
                prey_pressure: 0.0,
                pred_pressure: 0.0,
            });
        }
    }

    // Pressure fields: a few hot spots blurred over the map.
    let prey_spots = [(40.0, 14.0, 14.0), (84.0, 22.0, 12.0), (24.0, 30.0, 9.0), (110.0, 8.0, 10.0), (132.0, 30.0, 8.0)];
    let pred_spots = [(66.0, 18.0, 10.0), (98.0, 30.0, 8.0), (32.0, 8.0, 7.0), (128.0, 12.0, 6.0)];
    for y in 0..H {
        for x in 0..W {
            let c = &mut cells[y * W + x];
            if !c.terrain.walkable() {
                continue;
            }
            let field = |spots: &[(f32, f32, f32)]| {
                spots
                    .iter()
                    .map(|(sx, sy, r)| {
                        let d = ((x as f32 - sx).powi(2) / 4.0 + (y as f32 - sy).powi(2)).sqrt();
                        (1.0 - d / r).max(0.0)
                    })
                    .fold(0.0f32, f32::max)
            };
            c.prey_pressure = (field(&prey_spots) * (0.6 + 0.4 * c.vegetation)).clamp(0.0, 1.0);
            c.pred_pressure = field(&pred_spots).clamp(0.0, 1.0);
        }
    }

    let mut world = World {
        cells,
        dens: Vec::new(),
        carcasses: Vec::new(),
        seeds: Vec::new(),
        regions: vec![
            ("Northmarch", 0, 0, 50, 14),
            ("Ashen Ridge", 50, 0, 100, 12),
            ("Sunfall Coast", 100, 0, 150, 16),
            ("Reedwater Vale", 0, 14, 50, 28),
            ("The Long Meadow", 50, 12, 100, 28),
            ("Lakeshore", 100, 16, 150, 40),
            ("Southern Thicket", 0, 28, 50, 40),
            ("Fenlands", 50, 28, 100, 40),
        ],
    };

    let place = |rng: &mut Rng, n: usize, ok: &dyn Fn(&Cell) -> bool| -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut tries = 0;
        while out.len() < n && tries < 5000 {
            tries += 1;
            let x = rng.below(W);
            let y = rng.below(H);
            if ok(&world.cells[y * W + x]) {
                out.push((x, y));
            }
        }
        out
    };
    let dens = place(&mut rng, 9, &|c| matches!(c.terrain, Terrain::GrassDense | Terrain::Forest | Terrain::Dirt));
    let carcasses = place(&mut rng, 7, &|c| c.terrain.walkable() && !c.terrain.is_water());
    let seeds = place(&mut rng, 14, &|c| matches!(c.terrain, Terrain::Dirt | Terrain::GrassSparse));
    world.dens = dens;
    world.carcasses = carcasses;
    world.seeds = seeds;
    world
}
