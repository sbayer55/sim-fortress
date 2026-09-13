//! Balance diagnostic (throwaway): per-year per-species counts and death causes.
//!   cargo run --release --example `balance_diag` -- <years>

// Throwaway diagnostic: it is a separate compilation root and does not inherit
// the allow list in `src/lib.rs`, where the `indexing_slicing` sites are
// justified by the same checked-loop pattern.
#![allow(clippy::indexing_slicing)]

use sim_fortress::sim::{EventKind, Params, Sim, SpeciesId};

fn main() {
    let years: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(3);
    let ticks_per_year = 360 * 24;
    let mut sim = Sim::new(42, Params::default());
    for y in 0..years {
        let start_total = sim.events.total();
        for _ in 0..ticks_per_year {
            sim.step();
        }
        let mut deaths = [[0u64; 5]; 6]; // [species][cause]
        for e in sim.events.iter().skip(sim_fortress::cast!(start_total => usize)) {
            let (sp, cause) = match (e.species, e.kind) {
                (Some(s), EventKind::DeathStarved) => (s.index(), 0),
                (Some(s), EventKind::DeathThirst) => (s.index(), 1),
                (Some(s), EventKind::DeathAge) => (s.index(), 2),
                (Some(s), EventKind::DeathPredation) => (s.index(), 3),
                (Some(_), EventKind::Birth) => {
                    continue;
                }
                _ => continue,
            };
            deaths[sp][cause] += 1;
        }
        println!("year {}:", y + 1);
        for (i, id) in SpeciesId::ALL.iter().enumerate() {
            let (s, t, a, p) = (deaths[i][0], deaths[i][1], deaths[i][2], deaths[i][3]);
            println!(
                "  {:<6} count {:>4}  deaths total {:>4} (starved {s}, thirst {t}, age {a}, predated {p})",
                id.name(),
                sim.species[i].count,
                deaths[i].iter().sum::<u64>(),
            );
        }
        let extinct: Vec<_> = SpeciesId::ALL.iter().filter(|id| sim.extinct[id.index()]).map(|id| id.name()).collect();
        println!("  extinct so far: {extinct:?}");
    }
}
