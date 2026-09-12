//! Run a seed headless for N days and write a save (for looking at a late
//! state in the TUI): `save_after <seed> <days> <dir> [prey_only]`.
use sim_fortress::sim::{save, Params, Sim, SpeciesId};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let seed: u64 = a[0].parse().unwrap();
    let days: u64 = a[1].parse().unwrap();
    let dir = std::path::PathBuf::from(&a[2]);
    let mut p = Params::default();
    if a.get(3).is_some_and(|s| s == "prey_only") {
        for id in [SpeciesId::Fox, SpeciesId::Wolf, SpeciesId::Lynx] {
            p.creatures.initial_counts.insert(id, 0);
        }
    }
    let mut sim = Sim::new(seed, p);
    for _ in 0..days * 24 {
        sim.step();
    }
    let path = save::save(&sim, "Plague Valley", &dir).unwrap();
    let n = sim.creatures.len_living().max(1) as f32;
    let load: f32 = sim.creatures.living().map(|c| c.parasite_load).sum::<f32>() / n;
    let heavy = sim.creatures.living().filter(|c| c.parasite_load >= 0.5).count();
    let fouled = sim.world.cells.iter().filter(|c| c.parasite_load >= 0.25).count();
    let cell_mean: f32 = sim.world.cells.iter().map(|c| c.parasite_load).sum::<f32>() / sim.world.cells.len() as f32;
    println!(
        "{} outbreaks, {} active; parasites: mean load {:.3}, heavy {} of {}, cells ≥.25 {} (cell mean {:.3}); saved {}",
        sim.disease.outbreaks.len(), sim.disease.stats.iter().map(|s| s.active).sum::<u32>(), load, heavy, n as u32, fouled, cell_mean, path.display()
    );
}
