//! Sim Fortress binary entry point.
//!
//! CLI is parsed by hand (no clap):
//!   sim-fortress                              → live application (S09)
//!   sim-fortress --prototypes [ID]            → static prototype viewer
//!   sim-fortress --headless --seed N --ticks T [--params f]  → headless run

use sim_fortress::sim::{Params, Sim};
use sim_fortress::{prototypes, ui};

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--headless") {
        return run_headless(&args);
    }

    let mut terminal = ratatui::init();
    let result = if let Some(pos) = args.iter().position(|a| a == "--prototypes") {
        let id = args.get(pos + 1).map(String::as_str);
        prototypes::viewer::run(&mut terminal, id)
    } else {
        ui::app::run(&mut terminal)
    };
    ratatui::restore();
    result
}

fn run_headless(args: &[String]) -> std::io::Result<()> {
    let mut seed = 0u64;
    let mut ticks = 0u64;
    let mut params_file: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--headless" => {}
            "--seed" => {
                seed = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
                i += 1;
            }
            "--ticks" => {
                ticks = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
                i += 1;
            }
            "--params" => {
                params_file = args.get(i + 1).map(String::as_str);
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }

    let params = match params_file {
        Some(path) => {
            let text = std::fs::read_to_string(path)?;
            Params::from_toml(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))?
        }
        None => Params::default(),
    };

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
