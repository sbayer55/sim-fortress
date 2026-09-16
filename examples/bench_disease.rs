//! C7 disease bench (FR15): run one seed twice, disease off and on, and print
//! per year the populations, outbreaks, epidemic peaks, disease deaths and the
//! mean Resistance per host species, then the resistance delta across each
//! outbreak.
//!
//! `cargo run --release --example bench_disease -- [seed] [years] [key=value ...] [quiet]`
//!
//! Keys (disease balance table): `transmissibility`, `lethality`, `infectious_days`
//! (all three apply to every roster pathogen; prefix with `p0_`, `p1_`, `p2_` for one
//! slot), `emergence_per_day`, `reservoir_days`, `resist_hunger_cost`,
//! `kill_sick_bonus`, `spillover_chance`, `rain=dry|normal|wet`, `params=<file.toml>`,
//! `preset=<name>`.

// Developer tools, not shipped code: they are separate compilation roots and
// do not inherit the allow list in `src/lib.rs`. Indexing follows the same
// checked-loop pattern as the library, and `unwrap`/`expect`/`panic` are how a
// benchmark or diagnostic script is supposed to fail loudly.
#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim_fortress::sim::{EventKind, Params, Rainfall, Sim, SpeciesId, PRESETS};

// One linear benchmark/diagnostic driver: splitting it would only scatter the
// reporting it exists to print.
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut seed = 42u64;
    let mut years = 6u64;
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
        match k.as_str() {
            "params" => params = Params::from_toml(&std::fs::read_to_string(v).expect("read params")).expect("parse params"),
            "preset" => {
                let p = PRESETS.iter().find(|p| p.name.eq_ignore_ascii_case(v)).expect("preset name");
                params.apply_overlay(p.overlay).expect("preset overlay");
            }
            _ => {}
        }
    }
    for (k, v) in &overrides {
        let set_all = |d: &mut sim_fortress::sim::params::DiseaseParams, f: &dyn Fn(&mut sim_fortress::sim::params::PathogenParams)| {
            for p in &mut d.pathogens {
                f(p);
            }
        };
        let d = &mut params.disease;
        match k.as_str() {
            "params" | "preset" => {}
            "transmissibility" => set_all(d, &|p| p.transmissibility = v.parse().unwrap()),
            "lethality" => set_all(d, &|p| p.lethality_per_day = v.parse().unwrap()),
            "infectious_days" => set_all(d, &|p| p.infectious_days = v.parse().unwrap()),
            "emergence_per_day" => d.emergence_per_day = v.parse().unwrap(),
            "reservoir_days" => d.reservoir_days = v.parse().unwrap(),
            "resist_hunger_cost" => d.resist_hunger_cost = v.parse().unwrap(),
            "kill_sick_bonus" => d.kill_sick_bonus = v.parse().unwrap(),
            "spillover_chance" => d.spillover_chance = v.parse().unwrap(),
            "susceptibility_w" => d.susceptibility_w = v.parse().unwrap(),
            "resistance_founder_sd" => d.resistance_founder_sd = v.parse().unwrap(),
            "lethality_resist_w" => d.lethality_resist_w = v.parse().unwrap(),
            "vertical_transmission" => d.vertical_transmission = v.parse().unwrap(),
            "sick_hunger_factor" => d.sick_hunger_factor = v.parse().unwrap(),
            "fox" | "wolf" | "lynx" => params.species.set_initial_count(k, v.parse().unwrap()),
            "rain" => {
                params.world.rainfall = match v.as_str() {
                    "dry" => Rainfall::Dry,
                    "wet" => Rainfall::Wet,
                    _ => Rainfall::Normal,
                }
            }
            other => {
                // `p<slot>_<field>` for one roster pathogen.
                if let Some((slot, field)) = other.strip_prefix('p').and_then(|r| r.split_once('_')) {
                    let i: usize = slot.parse().expect("slot");
                    let p = &mut d.pathogens[i];
                    match field {
                        "transmissibility" => p.transmissibility = v.parse().unwrap(),
                        "lethality" => p.lethality_per_day = v.parse().unwrap(),
                        "infectious_days" => p.infectious_days = v.parse().unwrap(),
                        "incubation_days" => p.incubation_days = v.parse().unwrap(),
                        "immunity_days" => p.immunity_days = v.parse().unwrap(),
                        _ => panic!("unknown pathogen field {field}"),
                    }
                } else {
                    panic!("unknown key {other}");
                }
            }
        }
    }

    let mut off = params.clone();
    off.disease.enabled = false;
    let (rows_off, sim_off) = run(seed, years, &off);
    let (rows_on, sim_on) = run(seed, years, &params);

    println!("seed {seed}, {years} years — disease off vs on");
    println!("{:<4} {:>6} {:>6} {:>6} {:>5} {:>5} {:>5} | {:>6} {:>6} {:>6} {:>5} {:>5} {:>5} | {:>4} {:>4} {:>5} {:>5} {:>5} {:>5}",
        "year", "vole", "hare", "deer", "fox", "wolf", "lynx", "vole", "hare", "deer", "fox", "wolf", "lynx", "outb", "epid", "d.dis", "r.vol", "r.har", "r.dee");
    for (y, (a, b)) in rows_off.iter().zip(rows_on.iter()).enumerate() {
        println!(
            "{:<4} {:>6} {:>6} {:>6} {:>5} {:>5} {:>5} | {:>6} {:>6} {:>6} {:>5} {:>5} {:>5} | {:>4} {:>4} {:>5} {:>5.2} {:>5.2} {:>5.2}",
            y + 1, a.pop[0], a.pop[1], a.pop[2], a.pop[3], a.pop[4], a.pop[5],
            b.pop[0], b.pop[1], b.pop[2], b.pop[3], b.pop[4], b.pop[5],
            b.outbreaks, b.epidemics, b.disease_deaths, b.resist[0], b.resist[1], b.resist[2]
        );
    }
    println!("disease-off vole resistance: year1 {:.3} → end {:.3}", rows_off.first().map_or(0.0, |r| r.resist[0]), rows_off.last().map_or(0.0, |r| r.resist[0]));
    let _ = sim_off;
    if !quiet {
        println!("\noutbreaks:");
        for (i, o) in sim_on.disease.outbreaks.iter().enumerate() {
            let name = sim_on.disease.name(o.pathogen);
            let host = (0..o.species_cases.len()).max_by_key(|&s| o.species_cases[s]).unwrap_or(0);
            println!(
                "  #{:<2} {:<26} day {:>5} → {:>5}  cases {:>4} dead {:>4} rec {:>4} peak {:>4}{}  resist {} {:.3} → {:.3}",
                i, name, o.started_day, o.ended_day.map_or_else(|| "open".into(), |d| d.to_string()),
                o.cases, o.deaths, o.recovered, o.peak_active, if o.epidemic { " EPIDEMIC" } else { "" },
                sim_on.roster().name(SpeciesId::from_index(host)), o.resist_at_start[host], o.resist_at_end[host]
            );
        }
        let spill = sim_on.events.iter().filter(|e| e.kind == EventKind::Spillover).count();
        println!("spillover events in the ring: {spill}; strains: {}", sim_on.disease.pathogens.iter().filter(|p| p.is_strain()).count());
    }
}

struct YearRow {
    pop: Vec<u32>,
    outbreaks: usize,
    epidemics: usize,
    disease_deaths: u32,
    resist: Vec<f32>,
}

fn run(seed: u64, years: u64, params: &Params) -> (Vec<YearRow>, Sim) {
    let mut sim = Sim::new(seed, params.clone());
    let mut rows = Vec::new();
    for _ in 0..years {
        for _ in 0..360 * 24 {
            sim.step();
        }
        let census = sim_fortress::sim::stats::census(&sim.creatures, sim.roster().len());
        rows.push(YearRow {
            pop: census.population,
            outbreaks: sim.disease.outbreaks.len(),
            epidemics: sim.disease.outbreaks.iter().filter(|o| o.epidemic).count(),
            disease_deaths: sim.disease.stats.iter().map(|s| s.total_deaths).sum(),
            resist: census.genome_mean.iter().map(sim_fortress::sim::Genome::resistance).collect(),
        });
    }
    (rows, sim)
}
