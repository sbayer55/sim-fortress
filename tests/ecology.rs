//! C2 ecology acceptance tests.

use sim_fortress::sim::{EventKind, Params, Rainfall, Sim};

fn run_days(sim: &mut Sim, days: u64) {
    for _ in 0..(days * 24) {
        sim.step();
    }
}

#[test]
fn yearly_cycle_ratio() {
    let mut sim = Sim::new(42, Params::default());
    run_days(&mut sim, 720);

    // Every cell stays within 0..1.
    for c in &sim.world.cells {
        assert!((0.0..=1.0).contains(&c.moisture), "moisture {}", c.moisture);
        assert!((0.0..=1.0).contains(&c.vegetation), "vegetation {}", c.vegetation);
    }

    let samples = sim.series.samples();
    let summer_max = samples.iter().filter(|s| (450..540).contains(&s.day)).map(|s| s.veg_mean).fold(f32::MIN, f32::max);
    let winter_min = samples.iter().filter(|s| (630..720).contains(&s.day)).map(|s| s.veg_mean).fold(f32::MAX, f32::min);
    let ratio = summer_max / winter_min.max(1e-6);
    assert!((1.4..=3.0).contains(&ratio), "summer/winter ratio {ratio} out of 1.4..3.0");
}

#[test]
fn dry_world_has_drought() {
    for seed in 1..=5 {
        let mut p = Params::default();
        p.world.rainfall = Rainfall::Dry;
        let mut sim = Sim::new(seed, p);
        run_days(&mut sim, 720);
        let droughts = sim.events.iter().filter(|e| e.kind == EventKind::Drought).count();
        assert!(droughts >= 1, "seed {seed}: no drought in a dry world");
    }
}

#[test]
fn wet_world_has_none() {
    for seed in 1..=5 {
        let mut p = Params::default();
        p.world.rainfall = Rainfall::Wet;
        let mut sim = Sim::new(seed, p);
        run_days(&mut sim, 720);
        let droughts = sim.events.iter().filter(|e| e.kind == EventKind::Drought).count();
        assert_eq!(droughts, 0, "seed {seed}: drought in a wet world");
    }
}

#[test]
fn daily_update_is_deterministic() {
    let mut a = Sim::new(42, Params::default());
    let mut b = Sim::new(42, Params::default());
    run_days(&mut a, 720);
    run_days(&mut b, 720);
    assert_eq!(a.checksum(), b.checksum());
}

#[test]
fn csv_row_count() {
    let mut sim = Sim::new(42, Params::default());
    run_days(&mut sim, 720);
    let names: Vec<String> = sim.world.regions.iter().map(|r| r.0.clone()).collect();
    let csv = sim.series.to_csv(&names);
    let lines = csv.lines().count();
    assert_eq!(lines, 721, "expected 1 header + 720 data rows");
    assert!(csv.starts_with("day,biomass_total,veg_mean,water_cells"), "unexpected header");
}
