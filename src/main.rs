//! Sim Fortress binary entry point.
//!
//! CLI is parsed by hand (no clap):
//!   sim-fortress                                      → live application (S00 title)
//!   sim-fortress --headless --seed N --ticks T        → headless run
//!   sim-fortress --headless --seed N --years Y        → headless run (years override ticks)
//!   sim-fortress --seeds 1-20 --years Y --summary     → sweep + summary.csv
//!   sim-fortress --profile --headless --seed N ...    → per-system timings
//!   sim-fortress --dump-params                        → defaults with comments (stdout)
//!
//! `params.toml` in the cwd is applied automatically; `--params FILE` is an
//! overlay. `--saves-dir DIR` overrides the save directory (live app).

use std::path::Path;

use sim_fortress::sim::{self, Params, Sim, SpeciesId};
use sim_fortress::ui;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if has_flag(&args, "--dump-params") {
        print!("{}", Params::dump_toml());
        return Ok(());
    }
    if has_flag(&args, "--header") {
        println!("{}", summary_header());
        return Ok(());
    }

    let summary = has_flag(&args, "--summary");
    let row = has_flag(&args, "--row");
    let profile = has_flag(&args, "--profile");
    let headless = has_flag(&args, "--headless") || has_flag(&args, "--seeds") || summary || row || profile;
    let seed = flag_value(&args, "--seed").and_then(|s| s.parse().ok()).unwrap_or(0);
    let seeds = flag_value(&args, "--seeds").and_then(|s| parse_seed_range(s));
    let ticks = flag_value(&args, "--ticks").and_then(|s| s.parse().ok()).unwrap_or(0);
    let years = flag_value(&args, "--years").and_then(|s| s.parse::<u64>().ok());
    let params_file = flag_value(&args, "--params");
    let width = flag_value(&args, "--width").and_then(|s| s.parse::<usize>().ok());
    let height = flag_value(&args, "--height").and_then(|s| s.parse::<usize>().ok());
    let csv = flag_value(&args, "--csv");
    let saves_dir = flag_value(&args, "--saves-dir").map(Path::new);

    if headless {
        return run_headless(seed, seeds, ticks, years, params_file, width, height, csv, summary, row, profile);
    }

    let params = load_params(params_file, width, height)?;

    let mut terminal = ratatui::init();
    let result = ui::app::run(&mut terminal, params, saves_dir, params_file.is_some());
    ratatui::restore();
    result
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
    summary: bool,
    row: bool,
    profile: bool,
) -> std::io::Result<()> {
    let params = load_params(params_file, width, height)?;
    let ticks_per_year = 4 * params.time.season_days as u64 * params.time.ticks_per_day as u64;
    let ticks = years.map(|y| y * ticks_per_year).unwrap_or(ticks);

    if let Some((first, last)) = seeds {
        // In-process sequential sweep (FR8).
        let mut rows: Vec<String> = Vec::new();
        for s in first..=last {
            let sim = run_one(s, ticks, &params, profile);
            rows.push(summary_row(&sim, s, ticks as f64 / ticks_per_year as f64));
        }
        let mut out = String::from(summary_header());
        out.push('\n');
        out.push_str(&rows.join("\n"));
        out.push('\n');
        print!("{out}");
        std::fs::write("summary.csv", &out)?;
        return Ok(());
    }

    if row {
        // One headerless CSV row (used by scripts/sweep.sh).
        let sim = run_one(seed, ticks, &params, profile);
        println!("{}", summary_row(&sim, seed, ticks as f64 / ticks_per_year as f64));
        return Ok(());
    }

    let sim = run_one(seed, ticks, &params, profile);

    if summary {
        let mut out = String::from(summary_header());
        out.push('\n');
        out.push_str(&summary_row(&sim, seed, ticks as f64 / ticks_per_year as f64));
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
    if profile {
        print_profile(&sim);
    }
    if let Some(path) = csv {
        let names: Vec<String> = sim.world.regions.iter().map(|r| r.0.clone()).collect();
        std::fs::write(path, sim.series.to_csv(&names))?;
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

fn summary_header() -> String {
    let mut cols = vec!["seed".to_string(), "years".to_string()];
    for id in SpeciesId::ALL {
        cols.push(id.name().to_lowercase());
    }
    cols.push("extinctions".to_string());
    cols.push("lag_days".to_string());
    for id in SpeciesId::ALL {
        cols.push(format!("speed_{}", id.name().to_lowercase()));
    }
    cols.join(",")
}

fn summary_row(sim: &Sim, seed: u64, years: f64) -> String {
    let mut cols = vec![format!("{seed}"), format!("{years}")];
    for id in SpeciesId::ALL {
        cols.push(sim.species[id.index()].count.to_string());
    }
    let extinctions = sim.extinct.iter().filter(|&&e| e).count();
    cols.push(extinctions.to_string());
    let prey: Vec<f32> = sim
        .series
        .samples()
        .iter()
        .map(|s| (s.population[0] + s.population[1] + s.population[2]) as f32)
        .collect();
    let pred: Vec<f32> = sim
        .series
        .samples()
        .iter()
        .map(|s| (s.population[3] + s.population[4] + s.population[5]) as f32)
        .collect();
    let lag = sim::stats::peak_lag(&prey, &pred).map(|l| l.to_string()).unwrap_or_default();
    cols.push(lag);
    let census = sim::stats::census(&sim.creatures);
    for i in 0..6 {
        cols.push(format!("{:.3}", census.genome_mean[i].speed()));
    }
    cols.join(",")
}

fn print_profile(sim: &Sim) {
    let p = sim.profile;
    let ticks = sim.time.tick.max(1);
    let per1k = |ns: u64| ns as f64 / 1e9 / ticks as f64 * 1000.0;
    println!("profile (seconds per 1 000 ticks over {ticks} ticks):");
    println!("  behavior:      {:>8.4}", per1k(p.behavior_ns));
    println!("  day boundary:  {:>8.4}", per1k(p.day_boundary_ns));
    println!("  ecology:       {:>8.4}", per1k(p.ecology_ns));
    println!("  migration/ext: {:>8.4}", per1k(p.migration_ns));
    println!("  spatial:       {:>8.4}", per1k(p.spatial_ns));
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
