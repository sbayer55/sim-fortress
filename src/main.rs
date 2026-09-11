//! Sim Fortress binary entry point.
//!
//! CLI is parsed by hand (no clap):
//!   sim-fortress                                              → live application (S09)
//!   sim-fortress --prototypes [ID]                            → static prototype viewer
//!   sim-fortress --headless --seed N --ticks T [--params f]   → headless run
//!
//! World size can be configured with `--width` and `--height` (live and headless),
//! or via a `[world] width = … / height = …` table in a `--params` file.

use sim_fortress::sim::{Params, Sim};
use sim_fortress::{prototypes, ui};

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let headless = args.iter().any(|a| a == "--headless");
    let proto_pos = args.iter().position(|a| a == "--prototypes");
    let seed = flag_value(&args, "--seed").and_then(|s| s.parse().ok()).unwrap_or(0);
    let ticks = flag_value(&args, "--ticks").and_then(|s| s.parse().ok()).unwrap_or(0);
    let params_file = flag_value(&args, "--params");
    let width = flag_value(&args, "--width").and_then(|s| s.parse::<usize>().ok());
    let height = flag_value(&args, "--height").and_then(|s| s.parse::<usize>().ok());

    if headless {
        return run_headless(seed, ticks, params_file, width, height);
    }

    let params = load_params(params_file, width, height)?;

    let mut terminal = ratatui::init();
    let result = if let Some(pos) = proto_pos {
        let id = args.get(pos + 1).map(String::as_str);
        prototypes::viewer::run(&mut terminal, id)
    } else {
        ui::app::run(&mut terminal, params)
    };
    ratatui::restore();
    result
}

fn run_headless(seed: u64, ticks: u64, params_file: Option<&str>, width: Option<usize>, height: Option<usize>) -> std::io::Result<()> {
    let params = load_params(params_file, width, height)?;

    let mut sim = Sim::new(seed, params);
    for _ in 0..ticks {
        sim.step();
    }
    println!("seed={seed} ticks={ticks} checksum=0x{:016x}", sim.checksum());
    for e in sim.events.tail(5) {
        println!("Y{} D{:03} {:02}:00 {} {}", e.year, e.day, e.hour, e.kind.label(), e.text);
    }
    Ok(())
}

/// Return the value that follows `--flag` (if any).
fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

/// Build `Params` from an optional `--params` file, then apply `--width`/`--height`
/// overrides (they win over the file).
fn load_params(params_file: Option<&str>, width: Option<usize>, height: Option<usize>) -> std::io::Result<Params> {
    let mut params = match params_file {
        Some(path) => {
            let text = std::fs::read_to_string(path)?;
            Params::from_toml(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))?
        }
        None => Params::default(),
    };
    if let Some(w) = width {
        params.world.width = w.max(1);
    }
    if let Some(h) = height {
        params.world.height = h.max(1);
    }
    Ok(params)
}
