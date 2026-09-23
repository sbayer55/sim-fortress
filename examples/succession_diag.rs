//! Throwaway: how many cells differ in terrain between a default run and the
//! succession-neutral run of the same seed, and the note counts.
//! `cargo run --release --example succession_diag -- <seed> <years>`
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
use sim_fortress::sim::{Params, Sim};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(42);
    let years: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let mut on = Sim::new(seed, Params::default());
    let mut p = Params::default();
    p.succession.neutral();
    let mut off = Sim::new(seed, p);
    for y in 1..=years {
        for _ in 0..360 * 24 {
            on.step();
            off.step();
        }
        let differ = on.world.cells.iter().zip(&off.world.cells).filter(|(a, b)| a.terrain != b.terrain).count();
        let climbs = on.events.iter().filter(|e| e.text.starts_with("Scrub is closing")).count();
        let wears = on.events.iter().filter(|e| e.text.starts_with("Grazing wears")).count();
        let (fo, bo) = (on.series.last().unwrap().forest_cells, on.series.last().unwrap().bare_cells);
        let (ff, bf) = (off.series.last().unwrap().forest_cells, off.series.last().unwrap().bare_cells);
        println!("seed {seed} year {y}: {differ} cells differ; on forest {fo} bare {bo}; off forest {ff} bare {bf}; notes in ring: {climbs} climb, {wears} wear");
    }
}
