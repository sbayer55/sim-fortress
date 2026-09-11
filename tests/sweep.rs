//! C6 acceptance: a 20-seed, 10-year sweep keeps at least five species alive in
//! at least 14 seeds (aligned with C5's seed-42 criterion). Slow — run in release:
//! `cargo test --release --test sweep -- --ignored`.

use sim_fortress::sim::{Params, Sim, SpeciesId};

const TEN_YEARS: u64 = 10 * 360 * 24;

#[test]
#[ignore = "20-seed 10-year sweep; run in release with --ignored"]
fn twenty_seed_survival() {
    let mut alive_five = 0usize;
    for seed in 1..=20u64 {
        let mut sim = Sim::new(seed, Params::default());
        for _ in 0..TEN_YEARS {
            sim.step();
        }
        let alive = SpeciesId::ALL.iter().filter(|id| sim.species[id.index()].count > 0).count();
        eprintln!("seed {seed}: {alive} species alive at year 10");
        if alive >= 5 {
            alive_five += 1;
        }
    }
    assert!(alive_five >= 14, "only {alive_five}/20 seeds kept 5+ species alive at year 10");
}
