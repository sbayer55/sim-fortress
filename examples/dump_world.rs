use sim_fortress::sim::params::WorldParams;
use sim_fortress::sim::world::{Terrain, World};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let w: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(150);
    let h: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(40);
    let seed: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let world = World::generate(seed, &WorldParams { width: w, height: h, ..WorldParams::default() });
    let scale = (w.div_ceil(150)).max(h.div_ceil(40)).max(1);
    for y in (0..h).step_by(scale) {
        let line: String = (0..w).step_by(scale).map(|x| match world.cell(x, y).terrain {
            Terrain::DeepWater => '#', Terrain::ShallowWater => '~', Terrain::Sand => '.',
            Terrain::Dirt => ',', Terrain::GrassSparse | Terrain::Grass | Terrain::GrassDense => '"',
            Terrain::Forest => 'T', Terrain::Rock => '^',
        }).collect();
        println!("{line}");
    }
}
