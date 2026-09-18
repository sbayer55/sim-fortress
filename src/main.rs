//! Sim Fortress binary entry point.
//!
//! CLI is parsed by hand (no clap):
//!   sim-fortress                                      → live application (S00 title)
//!   sim-fortress --headless --seed N --ticks T        → headless run
//!   sim-fortress --headless --seed N --years Y        → headless run (years override ticks)
//!   sim-fortress --seeds 1-20 --years Y --summary     → sweep + summary.csv
//!   sim-fortress --profile --headless --seed N ...    → per-system timings
//!   sim-fortress --dump-params                        → defaults with comments (stdout)
//!   sim-fortress --headless ... --chronicle [--out F] → narrated run (C9; needs `--features ai`
//!                                                       and `[ai]` in ui.toml), chronicle.md
//!   sim-fortress --design-species "..." [--out F]     → a validated [[species]] overlay file
//!
//! `params.toml` in the cwd is applied automatically; `--params FILE` is an
//! overlay. `--saves-dir DIR` overrides the save directory (live app). The AI
//! flags exit 1 with one line when AI is off, unreachable or not compiled.

// The binary is a separate compilation root from `src/lib.rs`, so it does not
// inherit that file's allow list. The `indexing_slicing` sites here follow the
// same checked-loop pattern as the library.
#![allow(clippy::indexing_slicing)]

use std::path::Path;

use sim_fortress::sim::{self, Params, Roster, Sim};
use sim_fortress::ui;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if has_flag(&args, "--dump-params") {
        print!("{}", Params::dump_toml());
        return Ok(());
    }

    let summary = has_flag(&args, "--summary");
    let row = has_flag(&args, "--row");
    let profile = has_flag(&args, "--profile");
    let chronicle = has_flag(&args, "--chronicle");
    let out_file = flag_value(&args, "--out").map(str::to_string);
    let headless = has_flag(&args, "--headless") || has_flag(&args, "--seeds") || summary || row || profile || chronicle;
    let seed = flag_value(&args, "--seed").and_then(|s| s.parse().ok()).unwrap_or(0);
    let seeds = flag_value(&args, "--seeds").and_then(parse_seed_range);
    let ticks = flag_value(&args, "--ticks").and_then(|s| s.parse().ok()).unwrap_or(0);
    let years = flag_value(&args, "--years").and_then(|s| s.parse::<u64>().ok());
    let params_file = flag_value(&args, "--params");
    let width = flag_value(&args, "--width").and_then(|s| s.parse::<usize>().ok());
    let height = flag_value(&args, "--height").and_then(|s| s.parse::<usize>().ok());
    let csv = flag_value(&args, "--csv");
    let saves_dir = flag_value(&args, "--saves-dir").map(Path::new);

    if let Some(prompt) = flag_value(&args, "--design-species") {
        let params = load_params(params_file, width, height)?;
        return ai_exit(sim_fortress::ai::headless::design_species(prompt, out_file.as_deref(), &params));
    }
    if headless {
        return run_headless(seed, seeds, ticks, years, params_file, width, height, csv, &Outputs { summary, row, profile, chronicle, out_file });
    }

    let params = load_params(params_file, width, height)?;
    if has_flag(&args, "--header") {
        println!("{}", summary_header(&params.species));
        return Ok(());
    }

    let mut terminal = ratatui::init();
    let result = ui::app::run(&mut terminal, params, saves_dir, params_file.is_some());
    ratatui::restore();
    result
}

/// Which human-readable outputs the headless mode should emit.
struct Outputs {
    summary: bool,
    row: bool,
    profile: bool,
    /// C9: narrate the run season by season (needs the gateway).
    chronicle: bool,
    /// `--out FILE` for the chronicle.
    out_file: Option<String>,
}

/// AI flags fail with one line and exit 1 (ai-requirements R10), never a
/// silent fallback.
fn ai_exit(result: std::io::Result<()>) -> std::io::Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_headless(
    seed: u64,
    seeds: Option<(u64, u64)>,
    ticks: u64,
    years: Option<u64>,
    params_file: Option<&str>,
    width: Option<usize>,
    height: Option<usize>,
    csv: Option<&str>,
    out: &Outputs,
) -> std::io::Result<()> {
    let params = load_params(params_file, width, height)?;
    let ticks_per_year = 4 * u64::from(params.time.season_days) * u64::from(params.time.ticks_per_day);
    let ticks = years.map_or(ticks, |y| y * ticks_per_year);

    if out.chronicle {
        return ai_exit(sim_fortress::ai::headless::chronicle(seed, ticks, &params, out.out_file.as_deref()));
    }

    if let Some((first, last)) = seeds {
        // In-process sequential sweep (FR8).
        let mut rows: Vec<String> = Vec::new();
        for s in first..=last {
            let sim = run_one(s, ticks, &params, out.profile);
            rows.push(summary_row(&sim, s, sim_fortress::cast!(ticks => f64) / sim_fortress::cast!(ticks_per_year => f64)));
        }
        let mut out = summary_header(&params.species);
        out.push('\n');
        out.push_str(&rows.join("\n"));
        out.push('\n');
        print!("{out}");
        std::fs::write("summary.csv", &out)?;
        return Ok(());
    }

    if out.row {
        // One headerless CSV row (used by scripts/sweep.sh).
        let sim = run_one(seed, ticks, &params, out.profile);
        println!("{}", summary_row(&sim, seed, sim_fortress::cast!(ticks => f64) / sim_fortress::cast!(ticks_per_year => f64)));
        return Ok(());
    }

    let sim = run_one(seed, ticks, &params, out.profile);

    if out.summary {
        let mut out = summary_header(&params.species);
        out.push('\n');
        out.push_str(&summary_row(&sim, seed, sim_fortress::cast!(ticks => f64) / sim_fortress::cast!(ticks_per_year => f64)));
        out.push('\n');
        print!("{out}");
        std::fs::write("summary.csv", &out)?;
        return Ok(());
    }

    println!("seed={seed} ticks={ticks} checksum=0x{:016x}", sim.checksum());
    println!("living={} lineage_nodes={} soft_cap_notes={}", sim.creatures.len_living(), sim.lineage.len(), sim.soft_cap_crossings);
    for e in sim.events.tail(5) {
        println!("Y{} D{:03} {:02}:00 {} {}", e.year, e.day, e.hour, e.kind.label(), e.text);
    }
    if out.profile {
        print_profile(&sim);
    }
    if let Some(path) = csv {
        let names: Vec<String> = sim.world.regions.iter().map(|r| r.0.clone()).collect();
        std::fs::write(path, sim.series.to_csv(&names, sim.roster()))?;
    }
    Ok(())
}

fn run_one(seed: u64, ticks: u64, params: &Params, profile: bool) -> Sim {
    let mut sim = Sim::new(seed, params.clone());
    sim.profile_enabled = profile;
    for _ in 0..ticks {
        sim.step();
    }
    sim
}

fn summary_header(roster: &Roster) -> String {
    let mut cols = vec!["seed".to_string(), "years".to_string()];
    for id in roster.ids() {
        cols.push(roster.name(id).to_string());
    }
    cols.push("extinctions".to_string());
    cols.push("lag_days".to_string());
    for id in roster.ids() {
        cols.push(format!("speed_{}", roster.name(id)));
    }
    // C7 FR10
    cols.push("outbreaks".to_string());
    cols.push("epidemics".to_string());
    cols.push("spillovers".to_string());
    cols.push("disease_deaths".to_string());
    for id in roster.ids() {
        cols.push(format!("resistance_{}", roster.name(id)));
    }
    // Mutability: the evolvability trait's mean and how many animals its
    // sterility cost has taken out of the breeding pool.
    for id in roster.ids() {
        cols.push(format!("mutability_{}", roster.name(id)));
    }
    for id in roster.ids() {
        cols.push(format!("sterile_{}", roster.name(id)));
    }
    // Diet breadth: the grazing-range trait's mean (inert for predators).
    for id in roster.ids() {
        cols.push(format!("diet_breadth_{}", roster.name(id)));
    }
    // C8 follow-up: the group sizes the cohesion rule actually produces.
    for id in roster.ids() {
        let n = roster.name(id);
        cols.push(format!("group_mean_{n}"));
        cols.push(format!("group_max_{n}"));
    }
    cols.join(",")
}

fn summary_row(sim: &Sim, seed: u64, years: f64) -> String {
    let mut cols = vec![format!("{seed}"), format!("{years}")];
    let roster = sim.roster();
    let n = roster.len();
    for id in roster.ids() {
        cols.push(sim.species[id.index()].count.to_string());
    }
    let extinctions = sim.extinct.iter().filter(|&&e| e).count();
    cols.push(extinctions.to_string());
    let prey: Vec<f32> = sim
        .series
        .samples()
        .iter()
        .map(|s| sim_fortress::cast!(roster.prey_ids().map(|id| s.population[id.index()]).sum::<u32>() => f32))
        .collect();
    let pred: Vec<f32> = sim
        .series
        .samples()
        .iter()
        .map(|s| sim_fortress::cast!(roster.predator_ids().map(|id| s.population[id.index()]).sum::<u32>() => f32))
        .collect();
    let lag = sim::stats::peak_lag(&prey, &pred).map(|l| l.to_string()).unwrap_or_default();
    cols.push(lag);
    let census = sim::stats::census(&sim.creatures, n);
    for i in 0..n {
        cols.push(format!("{:.3}", census.genome_mean[i].speed()));
    }
    let d = &sim.disease;
    cols.push(d.outbreaks.len().to_string());
    cols.push(d.outbreaks.iter().filter(|o| o.epidemic).count().to_string());
    cols.push(d.pathogens.iter().filter(|p| p.is_strain()).count().to_string());
    cols.push(d.stats.iter().map(|s| s.total_deaths).sum::<u32>().to_string());
    for i in 0..n {
        cols.push(format!("{:.3}", census.genome_mean[i].resistance()));
    }
    for i in 0..n {
        cols.push(format!("{:.3}", census.genome_mean[i].mutability()));
    }
    for i in 0..n {
        cols.push(census.sterile[i].to_string());
    }
    for i in 0..n {
        cols.push(format!("{:.3}", census.genome_mean[i].diet_breadth()));
    }
    for i in 0..n {
        cols.push(format!("{:.2}", sim.group_stats.mean[i]));
        cols.push(sim.group_stats.max[i].to_string());
    }
    cols.join(",")
}

fn print_profile(sim: &Sim) {
    let p = sim.profile;
    let ticks = sim.time.tick.max(1);
    let per1k = |ns: u64| sim_fortress::cast!(ns => f64) / 1e9 / sim_fortress::cast!(ticks => f64) * 1000.0;
    println!("profile (seconds per 1 000 ticks over {ticks} ticks):");
    println!("  behavior:      {:>8.4}", per1k(p.behavior_ns));
    println!("  day boundary:  {:>8.4}", per1k(p.day_boundary_ns));
    println!("  ecology:       {:>8.4}", per1k(p.ecology_ns));
    println!("  migration/ext: {:>8.4}", per1k(p.migration_ns));
    println!("  spatial:       {:>8.4}", per1k(p.spatial_ns));
    println!("  disease:       {:>8.4}   (contagion is inside behavior; this is the daily update)", per1k(p.disease_ns));
    println!("  total step:    {:>8.4}", per1k(p.step_ns));
}

/// Return the value that follows `--flag` (if any).
fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

/// Parse an inclusive seed range: `1-20` or `1..20`.
fn parse_seed_range(s: &str) -> Option<(u64, u64)> {
    if let Some((a, b)) = s.split_once("..") {
        let a: u64 = a.trim().parse().ok()?;
        let b: u64 = b.trim().parse().ok()?;
        return Some((a, b.max(a)));
    }
    if let Some((a, b)) = s.split_once('-') {
        let a: u64 = a.trim().parse().ok()?;
        let b: u64 = b.trim().parse().ok()?;
        return Some((a, b.max(a)));
    }
    let single: u64 = s.trim().parse().ok()?;
    Some((single, single))
}

/// Build `Params`: defaults, then `params.toml` (cwd, if present), then
/// `--params` overlay, then `--width`/`--height` overrides (they win last).
fn load_params(params_file: Option<&str>, width: Option<usize>, height: Option<usize>) -> std::io::Result<Params> {
    let mut params = Params::default();
    if Path::new("params.toml").exists() {
        let text = std::fs::read_to_string("params.toml")?;
        params.apply_overlay(&text).map_err(invalid_input)?;
    }
    if let Some(path) = params_file {
        let text = std::fs::read_to_string(path)?;
        params.apply_overlay(&text).map_err(invalid_input)?;
    }
    if let Some(w) = width {
        params.world.width = w.max(1);
    }
    if let Some(h) = height {
        params.world.height = h.max(1);
    }
    Ok(params)
}

fn invalid_input(e: String) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, e)
}
