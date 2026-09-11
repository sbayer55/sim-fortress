//! Fixture decoration layered on top of `sim::World::generate`: pressure fields
//! and the dens/carcasses/seeds placements the live world does not have in C1.

use crate::sim::params::WorldParams;
use crate::sim::rng::Rng;
use crate::sim::world::{Terrain, World};

/// Build the fixture world: the same generator the live app uses, plus the
/// prototype-only decoration.
pub fn generate(seed: u64) -> World {
    let mut world = World::generate(seed, &WorldParams::default());
    apply_pressure(&mut world);
    place_resources(&mut world);
    world
}

fn apply_pressure(world: &mut World) {
    let (w, h) = (world.width, world.height);
    let prey_spots = [(40.0, 14.0, 14.0), (84.0, 22.0, 12.0), (24.0, 30.0, 9.0), (110.0, 8.0, 10.0), (132.0, 30.0, 8.0)];
    let pred_spots = [(66.0, 18.0, 10.0), (98.0, 30.0, 8.0), (32.0, 8.0, 7.0), (128.0, 12.0, 6.0)];
    for y in 0..h {
        for x in 0..w {
            let c = world.cell_mut(x, y);
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
}

fn place_resources(world: &mut World) {
    let mut rng = Rng::new(0xC0FFEE);
    let (w, h) = (world.width, world.height);
    let terrains: Vec<Terrain> = world.cells.iter().map(|c| c.terrain).collect();
    let place = |rng: &mut Rng, n: usize, ok: &dyn Fn(&Terrain) -> bool| -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut tries = 0;
        while out.len() < n && tries < 5000 {
            tries += 1;
            let x = rng.below(w);
            let y = rng.below(h);
            if ok(&terrains[y * w + x]) {
                out.push((x, y));
            }
        }
        out
    };
    world.dens = place(&mut rng, 9, &|t| matches!(t, Terrain::GrassDense | Terrain::Forest | Terrain::Dirt));
    world.carcasses = place(&mut rng, 7, &|t| t.walkable() && !t.is_water());
    world.seeds = place(&mut rng, 14, &|t| matches!(t, Terrain::Dirt | Terrain::GrassSparse));
}
