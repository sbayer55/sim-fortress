//! Balance diagnostics: per-species births and deaths by cause per 30-day window.
//!
//! `cargo run --release --example diag -- [seed] [days] [params.toml]`

// Developer tools, not shipped code: they are separate compilation roots and
// do not inherit the allow list in `src/lib.rs`. Indexing follows the same
// checked-loop pattern as the library, and `unwrap`/`expect`/`panic` are how a
// benchmark or diagnostic script is supposed to fail loudly.
#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim_fortress::sim::{EventKind, Params, Sim};

// One linear benchmark/diagnostic driver: splitting it would only scatter the
// reporting it exists to print.
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(42);
    let days: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(360);
    let params = match args.get(3) {
        Some(p) => Params::from_toml(&std::fs::read_to_string(p).expect("read params")).expect("parse params"),
        None => Params::default(),
    };
    let mut sim = Sim::new(seed, params);
    println!("day  sp   pop  ad  juv  births  deaths  thirst~ age~ | hunger thirst energy  elig%");
    for day in 1..=days {
        for _ in 0..24 {
            sim.step();
        }
        if day % 30 == 0 {
            let mut births = [0u32; 3];
            let mut deaths = [[0u32; 3]; 3];
            let samples = sim.series.samples();
            for s in &samples[samples.len().saturating_sub(30)..] {
                for i in 0..3 {
                    births[i] += s.births[i];
                    deaths[i][0] += s.deaths[i];
                }
            }
            // Cause split from the recent events (thirst / age columns).
            for e in sim.events.iter().rev().take(3000) {
                let Some(sp) = e.species else { continue };
                let i = sp.index();
                if sim.roster().kind(sp) != sim_fortress::sim::Kind::Prey || u64::from(e.day) < day.saturating_sub(30) % 360 {
                    continue;
                }
                match e.kind {
                    EventKind::DeathThirst => deaths[i][1] += 1,
                    EventKind::DeathAge => deaths[i][2] += 1,
                    _ => {}
                }
            }
            // Age profile of recent starvation deaths that still have a carcass record.
            let mut juv = 0;
            let mut adult = 0;
            let mut ages: Vec<u32> = Vec::new();
            let mut veg_at: Vec<f32> = Vec::new();
            for c in sim.creatures.carcasses() {
                let Some(d) = c.death else { continue };
                if d.cause != sim_fortress::sim::Cause::Starved || sim.roster().kind(c.species) != sim_fortress::sim::Kind::Prey {
                    continue;
                }
                let age = sim_fortress::cast!((i64::from(d.day) - i64::from(c.born_day)).max(0) => u32);
                ages.push(age);
                if c.adult {
                    adult += 1;
                } else {
                    juv += 1;
                }
                veg_at.push(sim.world.cell(c.x, c.y).vegetation);
            }
            // Thirst deaths: did they know a water spot, and how far was water?
            let mut known = 0;
            let mut unknown = 0;
            let mut d_known: Vec<f32> = Vec::new();
            let mut d_nearest: Vec<f32> = Vec::new();
            let water: Vec<(usize, usize)> = (0..sim.world.height)
                .flat_map(|y| (0..sim.world.width).map(move |x| (x, y)))
                .filter(|&(x, y)| sim.world.cell(x, y).terrain.is_water())
                .collect();
            for c in sim.creatures.carcasses() {
                let Some(d) = c.death else { continue };
                if d.cause != sim_fortress::sim::Cause::Thirst || sim.roster().kind(c.species) != sim_fortress::sim::Kind::Prey {
                    continue;
                }
                if let Some((wx, wy)) = c.last_water {
                    known += 1;
                    d_known.push(sim_fortress::sim::dist(c.x, c.y, wx, wy));
                } else {
                    unknown += 1;
                }
                let nd = water.iter().map(|&(wx, wy)| sim_fortress::sim::dist(c.x, c.y, wx, wy)).fold(f32::INFINITY, f32::min);
                d_nearest.push(nd);
            }
            let avg = |v: &[f32]| if v.is_empty() { 0.0 } else { v.iter().sum::<f32>() / sim_fortress::cast!(v.len() => f32) };
            println!("     thirst carcasses: knew water {known} (mean dist {:.1}), unknown {unknown}; mean dist to nearest water {:.1}", avg(&d_known), avg(&d_nearest));
            ages.sort_unstable();
            let med = ages.get(ages.len().div_euclid(2)).copied().unwrap_or(0);
            let vmean = if veg_at.is_empty() { 0.0 } else { veg_at.iter().sum::<f32>() / sim_fortress::cast!(veg_at.len() => f32) };
            println!("     starved carcasses: juv {juv} adult {adult}; median age {med}d; ages {:?}; veg at carcass {vmean:.2}", &ages[..ages.len().min(20)]);
            for sp in sim.roster().prey_ids() {
                let i = sp.index();
                let living: Vec<_> = sim.creatures.living().filter(|c| c.species == sp).collect();
                let n = sim_fortress::cast!(living.len().max(1) => f32);
                let adults = living.iter().filter(|c| c.adult).count();
                let hunger: f32 = living.iter().map(|c| c.hunger).sum::<f32>() / n;
                let thirst: f32 = living.iter().map(|c| c.thirst).sum::<f32>() / n;
                let energy: f32 = living.iter().map(|c| c.energy).sum::<f32>() / n;
                let gp = &sim.params.genetics;
                let elig = sim_fortress::cast!(living
                    .iter()
                    .filter(|c| c.adult && c.hunger < gp.mate_hunger_max && c.thirst < gp.mate_thirst_max && c.energy > gp.mate_energy_min)
                    .count() => f32)
                    / n
                    * 100.0;
                println!(
                    "{:4} {:<4} {:5} {:3} {:4}  {:6}  {:7} {:6} {:4}  | {:.2}   {:.2}   {:.2}   {:4.0}",
                    day,
                    sim.roster().name(sp),
                    living.len(),
                    adults,
                    living.len() - adults,
                    births[i],
                    deaths[i][0],
                    deaths[i][1],
                    deaths[i][2],
                    hunger,
                    thirst,
                    energy,
                    elig
                );
            }
            // Creatures close to dying of thirst: where are they trying to go?
            for c in sim.creatures.living().filter(|c| c.thirst > 0.9).take(6) {
                let tgt = c.target.map(|(x, y)| {
                    let cell = sim.world.cell(x, y);
                    format!("({x},{y}) {:?} shore={} d={:.1}", cell.terrain, sim.world.is_shore(x, y), sim_fortress::sim::dist(c.x, c.y, x, y))
                });
                let moved = c.trail.iter().filter(|&&p| p != (c.x, c.y)).count();
                println!(
                    "     thirsty {} {} at ({},{}) {:?} goal={:?} energy={:.2} hp={:.2} target={:?} last_water={:?} trail_distinct={}",
                    c.name_str(sim.roster()), c.tag(sim.roster()), c.x, c.y, sim.world.cell(c.x, c.y).terrain, c.goal, c.energy, c.hp, tgt, c.last_water, moved
                );
            }
            let veg = sim.series.last().map_or(0.0, |s| s.veg_mean);
            println!("     veg_mean {veg:.3}");
        }
    }
}
