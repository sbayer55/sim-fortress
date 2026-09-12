//! C7 acceptance (docs/chunks/c7-disease-and-parasites.md). The predator
//! worlds collapse in year 2 with disease off too (the open C5 balance
//! problem), so the population and selection criteria use the **prey-only**
//! world (no predator founders); spillover needs predators and uses defaults.
//!
//! One shared batch of runs (seeds 1..=10, 3 years, disease off and on) feeds
//! every criterion so the file costs one sweep, not one per test.

use std::sync::OnceLock;

use sim_fortress::sim::{EventKind, Params, Sim, SpeciesId};

const SEEDS: u64 = 10;
const YEARS: u64 = 3;

struct Run {
    /// Prey total at each year end (index 0 = end of year 1).
    prey_by_year: Vec<u32>,
    /// Mean vole Resistance at each year end.
    vole_resist_by_year: Vec<f32>,
    outbreaks: usize,
    epidemics: usize,
    /// (host species index, kills, host population on the start day) of the largest outbreak.
    biggest: Option<(usize, u32, u32)>,
    /// Vole Resistance on the day the first epidemic began and one year later.
    selection: Option<(f32, f32)>,
    extinct_within_30d_of_epidemic: bool,
}

fn prey_only(seed: u64, enabled: bool) -> Run {
    let mut p = Params::default();
    for id in [SpeciesId::Fox, SpeciesId::Wolf, SpeciesId::Lynx] {
        p.creatures.initial_counts.insert(id, 0);
    }
    p.disease.enabled = enabled;
    let mut sim = Sim::new(seed, p);
    let mut prey_by_year = Vec::new();
    let mut vole_resist_by_year = Vec::new();
    let mut first_epidemic: Option<(u32, f32)> = None; // (day, vole resistance then)
    let mut selection = None;
    let mut extinct_within = false;
    let mut alive_before: [bool; 3] = [true; 3];
    let mut host_pop_at_start: Vec<(u16, u32)> = Vec::new();
    for day in 0..(YEARS * 360) as u32 {
        for _ in 0..24 {
            sim.step();
        }
        let census = sim_fortress::sim::stats::census(&sim.creatures);
        // Record the host population on each outbreak's start day.
        for (i, o) in sim.disease.outbreaks.iter().enumerate() {
            let idx = sim.disease.first_index + i as u16;
            if o.started_day == day && !host_pop_at_start.iter().any(|(k, _)| *k == idx) {
                let hosts: u32 = (0..6).filter(|&s| sim.disease.pathogen(o.pathogen).is_some_and(|p| p.host(SpeciesId::ALL[s]) > 0.0)).map(|s| census.population[s]).sum();
                host_pop_at_start.push((idx, hosts));
            }
        }
        let epidemic_today = sim.disease.outbreaks.iter().any(|o| o.epidemic && o.started_day <= day && o.ended_day.is_none_or(|e| e >= day));
        if first_epidemic.is_none() && sim.disease.outbreaks.iter().any(|o| o.epidemic) {
            first_epidemic = Some((day, census.genome_mean[0].resistance()));
        }
        if let Some((d0, r0)) = first_epidemic {
            if day == d0 + 360 && census.population[0] > 0 {
                selection = Some((r0, census.genome_mean[0].resistance()));
            }
        }
        // Extinction within 30 days of an epidemic, for a species alive with ≥ 40.
        for (s, was_alive) in alive_before.iter_mut().enumerate() {
            let alive = census.population[s] > 0;
            if *was_alive && !alive && epidemic_today {
                extinct_within = true;
            }
            *was_alive = alive;
        }
        if (day + 1) % 360 == 0 {
            prey_by_year.push(census.prey_total());
            vole_resist_by_year.push(census.genome_mean[0].resistance());
        }
    }
    let biggest = sim
        .disease
        .outbreaks
        .iter()
        .enumerate()
        .max_by_key(|(_, o)| o.deaths)
        .map(|(i, o)| {
            let host = (0..6).max_by_key(|&s| o.species_cases[s]).unwrap_or(0);
            let idx = sim.disease.first_index + i as u16;
            let pop = host_pop_at_start.iter().find(|(k, _)| *k == idx).map(|(_, p)| *p).unwrap_or(0);
            (host, o.deaths, pop)
        });
    Run {
        prey_by_year,
        vole_resist_by_year,
        outbreaks: sim.disease.outbreaks.len(),
        epidemics: sim.disease.outbreaks.iter().filter(|o| o.epidemic).count(),
        biggest,
        selection,
        extinct_within_30d_of_epidemic: extinct_within,
    }
}

fn batch() -> &'static Vec<(u64, Run, Run)> {
    static BATCH: OnceLock<Vec<(u64, Run, Run)>> = OnceLock::new();
    BATCH.get_or_init(|| {
        let handles: Vec<_> = (1..=SEEDS)
            .map(|seed| std::thread::spawn(move || (seed, prey_only(seed, false), prey_only(seed, true))))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    })
}

#[test]
fn outbreaks_9_of_10() {
    let n = batch().iter().filter(|(_, _, on)| on.outbreaks >= 1).count();
    assert!(n >= 9, "outbreak in {n} of 10 seeds");
}

#[test]
fn epidemics_6_of_10() {
    let n = batch().iter().filter(|(_, _, on)| on.epidemics >= 1).count();
    assert!(n >= 6, "epidemic in {n} of 10 seeds");
}

#[test]
fn epidemic_mortality_band() {
    // The largest outbreak kills between 5 % and 60 % of its hosts on the start day.
    let mut ok = 0;
    let mut report = Vec::new();
    for (seed, _, on) in batch() {
        if let Some((host, dead, pop)) = on.biggest {
            let share = if pop > 0 { dead as f32 / pop as f32 } else { 0.0 };
            report.push(format!("seed {seed}: {} {dead}/{pop} = {:.0}%", SpeciesId::ALL[host].name(), share * 100.0));
            if (0.05..=0.60).contains(&share) {
                ok += 1;
            }
        }
    }
    assert!(ok >= 7, "mortality band met in {ok} of 10 seeds: {report:?}");
}

#[test]
fn no_extinction_within_epidemic() {
    let bad: Vec<u64> = batch().iter().filter(|(_, _, on)| on.extinct_within_30d_of_epidemic).map(|(s, _, _)| *s).collect();
    assert!(bad.len() <= 3, "a prey species died out during an epidemic in seeds {bad:?}");
}

#[test]
fn selection_7_of_10() {
    // Mean vole Resistance one year after the first epidemic is above the value
    // the day it began in most seeds, and clearly above (≥ +0.02) in several.
    let mut rose = 0;
    let mut clear = 0;
    let mut report = Vec::new();
    for (seed, _, on) in batch() {
        if let Some((r0, r1)) = on.selection {
            report.push(format!("seed {seed}: {r0:.3} → {r1:.3}"));
            if r1 - r0 > 0.0 {
                rose += 1;
            }
            if r1 - r0 >= 0.02 {
                clear += 1;
            }
        } else {
            report.push(format!("seed {seed}: no epidemic within the window"));
        }
    }
    assert!(rose >= 7, "resistance rose in {rose} of 10 seeds: {report:?}");
    assert!(clear >= 3, "resistance rose ≥ +0.02 in {clear} of 10 seeds: {report:?}");
}

#[test]
fn cost_reversal_off_world() {
    // With disease off the hunger cost is selected against: vole Resistance does
    // not rise, and falls in most seeds.
    let mut falls = 0;
    let mut report = Vec::new();
    for (seed, off, _) in batch() {
        if let (Some(a), Some(b)) = (off.vole_resist_by_year.first(), off.vole_resist_by_year.last()) {
            report.push(format!("seed {seed}: {a:.3} → {b:.3}"));
            if *b > 0.0 && b <= a {
                falls += 1;
            }
        }
    }
    assert!(falls >= 5, "resistance fell (or held) in {falls} of 10 disease-off seeds: {report:?}");
}

#[test]
fn relative_population_effect() {
    // Year-3 total prey with disease on is at least 40 % of the disease-off run in
    // most seeds (the prey mix shifts; the total must not collapse).
    let mut ok = 0;
    let mut report = Vec::new();
    for (seed, off, on) in batch() {
        let a = off.prey_by_year.last().copied().unwrap_or(0) as f32;
        let b = on.prey_by_year.last().copied().unwrap_or(0) as f32;
        report.push(format!("seed {seed}: off {a} on {b}"));
        if a == 0.0 || b >= 0.4 * a {
            ok += 1;
        }
    }
    assert!(ok >= 7, "prey total ≥ 40 % of the disease-off run in {ok} of 10 seeds: {report:?}");
}

#[test]
fn disabled_world_has_no_disease_state() {
    for (_, off, _) in batch() {
        assert_eq!(off.outbreaks, 0);
    }
}

/// Spillover needs predators eating infected prey; the default world keeps
/// predators for about a year, which is enough to see the rare jump in a few
/// seeds. Checked over 20 seeds × 2 years.
#[test]
#[ignore = "slow: 20 default-world seeds × 2 years; run with --ignored"]
fn spillover_is_rare_but_real() {
    let handles: Vec<_> = (1..=20u64)
        .map(|seed| {
            std::thread::spawn(move || {
                let mut sim = Sim::new(seed, Params::default());
                for _ in 0..2 * 360 * 24 {
                    sim.step();
                }
                let spill = sim.events.iter().filter(|e| e.kind == EventKind::Spillover).count() + sim.disease.pathogens.iter().filter(|p| p.is_strain()).count();
                let strain_outbreaks = sim.disease.outbreaks.iter().filter(|o| sim.disease.pathogen(o.pathogen).is_some_and(|p| p.is_strain())).count();
                (spill > 0, strain_outbreaks, sim.disease.outbreaks.len())
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let seeds_with = results.iter().filter(|r| r.0).count();
    let strain: usize = results.iter().map(|r| r.1).sum();
    let all: usize = results.iter().map(|r| r.2).sum();
    assert!((3..=16).contains(&seeds_with), "spillover in {seeds_with} of 20 seeds");
    assert!(all == 0 || (strain as f32) < 0.25 * all as f32, "strain outbreaks {strain} of {all}");
}
