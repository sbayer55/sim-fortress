//! Print a generated world as ASCII: terrain by default (`F` a waterfall,
//! `w` a dry wash), elevation shading
//! with a fourth argument of `elev`, biomes with `biome`, or regions with
//! `region` (one letter per region, names listed after the map).
//!
//! `cargo run --release --example dump_world -- [width] [height] [seed] [elev|biome|region] [age]`

// Developer tool, not shipped code: a separate compilation root that does not
// inherit the allow list in `src/lib.rs`. The one index is clamped to the
// shade table's length.
#![allow(clippy::indexing_slicing)]
use sim_fortress::sim::params::WorldParams;
use sim_fortress::sim::world::{AgeRegime, Terrain, World};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let w: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(150);
    let h: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(40);
    let seed: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let mode = args.get(4).map_or("terrain", String::as_str);
    let elev = mode == "elev";
    let age: u8 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or_else(|| WorldParams::default().age);
    let started = std::time::Instant::now();
    let world = World::generate(seed, &WorldParams { width: w, height: h, age, ..WorldParams::default() });
    let took = started.elapsed();
    let scale = (w.div_ceil(150)).max(h.div_ceil(40)).max(1);
    let shades: Vec<char> = " .:-=+*#%@".chars().collect();
    for y in (0..h).step_by(scale) {
        let line: String = (0..w)
            .step_by(scale)
            .map(|x| {
                let cell = world.cell(x, y);
                if mode == "biome" {
                    return char::from(b"TtFgsSdw"[sim_fortress::cast!(cell.biome => usize).min(7)]);
                }
                if mode == "region" {
                    return char::from(b"ABCDEFGH"[world.region_index(x, y).min(7)]);
                }
                if elev {
                    if cell.terrain.is_water() {
                        return '~';
                    }
                    let i = sim_fortress::cast!((cell.elevation * 9.99).floor() => usize).min(9);
                    return shades[i];
                }
                if world.is_fall(x, y) {
                    return 'F';
                }
                if cell.dried_from.is_some() {
                    return 'w';
                }
                match cell.terrain {
                    Terrain::DeepWater => '#',
                    Terrain::ShallowWater => '~',
                    Terrain::Sand => '.',
                    Terrain::Dirt => ',',
                    Terrain::GrassSparse | Terrain::Grass | Terrain::GrassDense => '"',
                    Terrain::Forest => 'T',
                    Terrain::Rock => '^',
                    Terrain::Marsh => 'm',
                }
            })
            .collect();
        println!("{line}");
    }
    if mode == "region" {
        for (i, r) in world.regions.iter().enumerate() {
            eprintln!("{} {}: {} cells", char::from(b"ABCDEFGH"[i.min(7)]), r.0, world.region_size(i));
        }
    }
    for e in &world.history {
        eprintln!("{}", e.describe(age));
    }
    for f in &world.names.features {
        eprintln!("{:<10} {:<24} {:>4} cells at {:?}", f.kind.label(), f.name, f.cells, f.anchor);
    }
    for line in sim_fortress::sim::world::chronicle(&world, age) {
        eprintln!("| {line}");
    }
    eprintln!("generated {w}x{h} seed {seed} age {age} ({} land) in {took:?}", AgeRegime::for_age(age).name);
}
