//! C5 balance bench: one headless run, one summary line (plus a per-year table
//! unless `quiet`), with the predation levers overridable from the command line.
//!
//! `cargo run --release --example bench_c5 -- [seed] [years] [key=value ...] [quiet]`
//!
//! Keys: `kill_base`, `chase_max_ticks`, `chase_speed_bonus`, `hunt_cooldown_hours`,
//! `hunger_per_kill_base`, `hunger_per_kill_per_size`, `fox`, `wolf`, `lynx`
//! (predator founders), `rain=dry|normal|wet`, `params=<file.toml>`, and the
//! C5 FR13 territory levers `mark_per_tick`, `avoid_w`, `resident_bonus`,
//! `contest_injury` plus `territory=off` for the neutral control.
//!
//! The summary reports: species counts at each year end, how many species are alive
//! at year 5 and at the end, local maxima on the smoothed prey/predator totals,
//! `peak_lag`, and per-predator hunt success from the cumulative tallies.

// Developer tools, not shipped code: they are separate compilation roots and
// do not inherit the allow list in `src/lib.rs`. Indexing follows the same
// checked-loop pattern as the library, and `unwrap`/`expect`/`panic` are how a
// benchmark or diagnostic script is supposed to fail loudly.
#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim_fortress::sim::stats::{local_maxima, peak_lag};
use sim_fortress::sim::{Params, Rainfall, Sim};
use std::fmt::Write as _;

// One linear benchmark/diagnostic driver: splitting it would only scatter the
// reporting it exists to print.
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut seed = 42u64;
    let mut years = 5u64;
    let mut quiet = false;
    let mut positional = 0;
    let mut params = Params::default();
    let mut overrides: Vec<(String, String)> = Vec::new();
    for a in &args {
        if a == "quiet" {
            quiet = true;
        } else if let Some((k, v)) = a.split_once('=') {
            overrides.push((k.to_string(), v.to_string()));
        } else if positional == 0 {
            seed = a.parse().expect("seed");
            positional += 1;
        } else {
            years = a.parse().expect("years");
            positional += 1;
        }
    }
    for (k, v) in &overrides {
        if k == "params" {
            params = Params::from_toml(&std::fs::read_to_string(v).expect("read params")).expect("parse params");
        }
    }
    for (k, v) in &overrides {
        let p = &mut params.predation;
        match k.as_str() {
            "params" => {}
            "kill_base" => p.kill_base = v.parse().unwrap(),
            "chase_max_ticks" => p.chase_max_ticks = v.parse().unwrap(),
            "chase_speed_bonus" => p.chase_speed_bonus = v.parse().unwrap(),
            "hunt_cooldown_hours" => p.hunt_cooldown_hours = v.parse().unwrap(),
            "hunger_per_kill_base" => p.hunger_per_kill_base = v.parse().unwrap(),
            "hunger_per_kill_per_size" => p.hunger_per_kill_per_size = v.parse().unwrap(),
            "hunt_hunger_min" => p.hunt_hunger_min = v.parse().unwrap(),
            "kill_max" => p.kill_max = v.parse().unwrap(),
            "kill_min" => p.kill_min = v.parse().unwrap(),
            "flee_distance" => p.flee_distance = v.parse().unwrap(),
            "flee_ticks" => p.flee_ticks = v.parse().unwrap(),
            "eat_hours_base" => p.eat_hours_base = v.parse().unwrap(),
            "fox" | "wolf" | "lynx" => params.species.set_initial_count(k, v.parse().unwrap()),
            // Fallback levers outside the C5 balance table (predator reproduction).
            "pred_litter" => {
                let v: f32 = v.parse().unwrap();
                for s in params.species.0.iter_mut().filter(|s| s.kind == sim_fortress::sim::Kind::Predator) {
                    s.litter_max = v;
                }
            }
            "vole_litter" | "fox_litter" => {
                let name = k.trim_end_matches("_litter");
                if let Some(id) = params.species.id(name) {
                    params.species.get_mut(id).litter_max = v.parse().unwrap();
                }
            }
            "pred_cooldown" => {
                let v: u32 = v.parse().unwrap();
                for s in params.species.0.iter_mut().filter(|s| s.kind == sim_fortress::sim::Kind::Predator) {
                    s.mate_cooldown_days = v;
                }
            }
            // C5 FR13 territory levers and the neutral control.
            "mark_per_tick" => params.territory.mark_per_tick = v.parse().unwrap(),
            "avoid_w" => params.territory.avoid_w = v.parse().unwrap(),
            "resident_bonus" => params.territory.resident_bonus = v.parse().unwrap(),
            "contest_injury" => params.territory.contest_injury = v.parse().unwrap(),
            "territory" => {
                if v == "off" {
                    params.territory.neutral();
                }
            }
            "rain" => {
                params.world.rainfall = match v.as_str() {
                    "dry" => Rainfall::Dry,
                    "wet" => Rainfall::Wet,
                    _ => Rainfall::Normal,
                }
            }
            other => panic!("unknown key {other}"),
        }
    }
    params.stats.series_days = (sim_fortress::cast!(years => usize) + 1) * 360;

    let mut sim = Sim::new(seed, params);
    if let Ok(days) = std::env::var("DEBUG_DAYS") {
        let days: u64 = days.parse().unwrap();
        for _ in 0..days * 24 {
            sim.step();
        }
        for c in sim.creatures.living().filter(|c| sim.roster().kind(c.species) == sim_fortress::sim::Kind::Prey && c.thirst > 0.8).take(15) {
            println!(
                "{:?} {:?} at ({},{}) thirst {:.2} hunger {:.2} goal {:?} target {:?} threatened {:?} flee_until {} last_water {:?} replan_at {} tick {}",
                c.id, c.species, c.x, c.y, c.thirst, c.hunger, c.goal, c.target, c.threatened_by, c.flee_until, c.last_water, c.replan_at, sim.time.tick
            );
        }
        {
            use std::collections::BTreeMap;
            let mut goals: BTreeMap<String, u32> = BTreeMap::new();
            let (mut n, mut e, mut th, mut edge) = (0u32, 0.0f32, 0.0f32, 0u32);
            for c in sim.creatures.living().filter(|c| sim.roster().kind(c.species) == sim_fortress::sim::Kind::Prey) {
                *goals.entry(format!("{:?}", c.goal)).or_insert(0) += 1;
                n += 1;
                e += c.energy;
                th += c.thirst;
                if c.x < 2 || c.y < 2 || c.x + 2 >= sim.world.width || c.y + 2 >= sim.world.height {
                    edge += 1;
                }
            }
            println!("prey n={n} mean energy {:.2} mean thirst {:.2} at-edge {edge} goals {:?}", e / sim_fortress::cast!(n.max(1) => f32), th / sim_fortress::cast!(n.max(1) => f32), goals);
            let mut pg: BTreeMap<String, u32> = BTreeMap::new();
            for c in sim.creatures.living().filter(|c| sim.roster().kind(c.species) == sim_fortress::sim::Kind::Predator) {
                *pg.entry(format!("{:?}", c.goal)).or_insert(0) += 1;
            }
            println!("predator goals {pg:?}");
        }
        let worst = sim.creatures.living().filter(|c| sim.roster().kind(c.species) == sim_fortress::sim::Kind::Prey).max_by(|a, b| a.thirst.partial_cmp(&b.thirst).unwrap()).map(|c| c.id);
        if let Some(id) = worst {
            for _ in 0..8 {
                sim.step();
                if let Some(c) = sim.creatures.get(id) {
                    println!(
                        "trace {:?} at ({},{}) alive {} thirst {:.2} energy {:.2} goal {:?} target {:?} threatened {:?} budget {:.2} path {:?} replan_at {} tick {} water@target {:?}",
                        c.id, c.x, c.y, c.alive, c.thirst, c.energy, c.goal, c.target, c.threatened_by, c.move_budget, c.path, c.replan_at, sim.time.tick,
                        c.target.map(|(x, y)| sim.world.cell(x, y).terrain)
                    );
                }
            }
        }
        for c in sim.creatures.living().filter(|c| sim.roster().kind(c.species) == sim_fortress::sim::Kind::Predator) {
            println!(
                "{:?} {:?} hunger {:.2} thirst {:.2} energy {:.2} goal {:?} phase {:?} kills {} attempts {} cooldown_until {} tick {}",
                c.id, c.species, c.hunger, c.thirst, c.energy, c.goal, c.hunt_phase, c.kills, c.attempts, c.hunt_cooldown_until, sim.time.tick
            );
        }
        return;
    }
    let ticks_per_year = 360 * 24;
    let n = sim.roster().len();
    let mut year_end: Vec<Vec<u32>> = Vec::new();
    for _y in 0..years {
        for _ in 0..ticks_per_year {
            sim.step();
        }
        year_end.push(sim.species.iter().map(|s| s.count).collect());
        if year_end.last().unwrap().iter().all(|&n| n == 0) {
            break;
        }
    }

    let samples = sim.series.samples();
    let prey: Vec<f32> = samples.iter().map(|s| sim_fortress::cast!((s.population[0] + s.population[1] + s.population[2]) => f32)).collect();
    let pred: Vec<f32> = samples.iter().map(|s| sim_fortress::cast!((s.population[3] + s.population[4] + s.population[5]) => f32)).collect();
    let smooth = |v: &[f32]| -> Vec<f32> {
        let n = v.len();
        (0..n)
            .map(|i| {
                let lo = i.saturating_sub(15);
                let hi = (i + 16).min(n);
                v[lo..hi].iter().sum::<f32>() / sim_fortress::cast!((hi - lo) => f32)
            })
            .collect()
    };
    let (prey_max, pred_max) = if prey.len() > 360 {
        (local_maxima(&smooth(&prey[360..])).len(), local_maxima(&smooth(&pred[360..])).len())
    } else {
        (0, 0)
    };
    let lag = peak_lag(&prey, &pred);

    let alive_at = |y: usize| year_end.get(y - 1).map_or(0, |c| c.iter().filter(|&&n| n > 0).count());
    let alive5 = alive_at(5.min(sim_fortress::cast!(years => usize)));
    let alive_end = year_end.last().map_or(0, |c| c.iter().filter(|&&n| n > 0).count());

    let mut hunt = String::new();
    for id in sim.roster().predator_ids() {
        let k = sim.deaths.hunt_kills[id.index()];
        let a = sim.deaths.hunt_attempts[id.index()];
        let pct = if a > 0 { sim_fortress::cast!(k => f32) / sim_fortress::cast!(a => f32) * 100.0 } else { 0.0 };
        let _ = write!(hunt, " {}:{}/{}={:.0}%", sim.roster().name(id), k, a, pct);
    }

    if !quiet {
        let header: Vec<String> = sim.roster().ids().map(|id| format!("{:>5}", sim.roster().name(id))).collect();
        println!("year {} | births/deaths per species", header.join(" "));
        for (y, c) in year_end.iter().enumerate() {
            let lo = y * 360;
            let hi = ((y + 1) * 360).min(samples.len());
            let mut b = vec![0u32; n];
            let mut d = vec![0u32; n];
            for s in &samples[lo.min(samples.len())..hi] {
                for i in 0..n {
                    b[i] += s.births[i];
                    d[i] += s.deaths[i];
                }
            }
            let bd: Vec<String> = (0..n).map(|i| format!("{}/{}", b[i], d[i])).collect();
            let counts: Vec<String> = c.iter().map(|v| format!("{v:>5}")).collect();
            println!("{:4} {} | {}", y + 1, counts.join(" "), bd.join(" "));
        }
    }
    if !quiet {
        // Death causes per species from the (capacity-bounded) event ring.
        use sim_fortress::sim::EventKind;
        let mut causes = vec![[0u32; 4]; n];
        for e in sim.events.iter() {
            let Some(sp) = e.species else { continue };
            let k = match e.kind {
                EventKind::DeathStarved => 0,
                EventKind::DeathThirst => 1,
                EventKind::DeathAge => 2,
                EventKind::DeathPredation => 3,
                _ => continue,
            };
            causes[sp.index()][k] += 1;
        }
        for id in sim.roster().ids() {
            let c = causes[id.index()];
            println!("{:5} deaths in ring: starved {} thirst {} age {} predation {}", sim.roster().name(id), c[0], c[1], c[2], c[3]);
        }
    }
    // C5 FR13: contests fought and the adult nearest-neighbour spacing per predator.
    let mut territory = String::new();
    for id in sim.roster().predator_ids() {
        let _ = write!(territory, " {}:{}c/{:.1}nn", sim.roster().name(id), sim.deaths.contests[id.index()], sim.group_stats.nn_mean[id.index()]);
    }
    let ov: Vec<String> = overrides.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let last = year_end.last().cloned().unwrap_or_default();
    println!(
        "seed={seed} [{}] alive5={alive5} aliveEnd={alive_end} end={:?} maxima={prey_max}/{pred_max} lag={:?} hunt{hunt} territory{territory}",
        ov.join(" "),
        last,
        lag
    );
}
