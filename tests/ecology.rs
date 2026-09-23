//! C2 ecology acceptance tests.

use sim_fortress::sim::{EventKind, Params, Rainfall, Sim};

fn run_days(sim: &mut Sim, days: u64) {
    for _ in 0..(days * 24) {
        sim.step();
    }
}

#[test]
fn yearly_cycle_ratio() {
    // Pure ecology: no grazers (a breeding herd since C4 grazes winter
    // vegetation far below the seasonal cap and would swamp the ratio).
    let mut p = Params::default();
    p.species.clear_initial_counts();
    let mut sim = Sim::new(42, p);
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
    let csv = sim.series.to_csv(&names, sim.roster());
    let lines = csv.lines().count();
    assert_eq!(lines, 721, "expected 1 header + 720 data rows");
    assert!(csv.starts_with("day,biomass_total,veg_mean,water_cells"), "unexpected header");
}

// ---- C2 FR12 succession and trampling ----

fn forest_count(sim: &Sim) -> usize {
    sim.world.cells.iter().filter(|c| c.terrain == sim_fortress::sim::Terrain::Forest).count()
}

#[test]
fn ungrazed_meadows_close_into_forest() {
    // No grazers: every lush, moist meadow in a wooded biome is thriving, so
    // over five years forest spreads beyond the count worldgen placed.
    let mut p = Params::default();
    p.species.clear_initial_counts();
    let mut sim = Sim::new(42, p);
    let at_generation = forest_count(&sim);
    run_days(&mut sim, 5 * 360);
    let last = sim.series.last().expect("a sample");
    assert_eq!(last.forest_cells, forest_count(&sim), "the series tally is the map's count");
    assert!(last.forest_cells > at_generation + 50, "forest {} at generation, {} after five ungrazed years", at_generation, last.forest_cells);
    assert!(sim.events.iter().any(|e| e.text.starts_with("Scrub is closing over")), "a climb note was logged");
}

#[test]
fn grazed_ground_wears_and_recovers() {
    // The default roster: herds tread and graze their cells bare, so bare
    // ground exceeds the neutral run's by the end of year one, and by year
    // two ground they have left is climbing again.
    let mut on = Sim::new(42, Params::default());
    let mut off_p = Params::default();
    off_p.succession.neutral();
    let mut off = Sim::new(42, off_p);
    run_days(&mut on, 360);
    run_days(&mut off, 360);
    let (bare_on, bare_off) = (on.series.last().map_or(0, |s| s.bare_cells), off.series.last().map_or(0, |s| s.bare_cells));
    assert!(bare_on > bare_off, "bare cells after one year: {bare_on} with succession, {bare_off} without");
    assert!(on.events.iter().any(|e| e.text.starts_with("Grazing wears")), "a wear note was logged in year one");
    run_days(&mut on, 360);
    assert!(on.events.iter().any(|e| e.text.starts_with("Scrub is closing over")), "a climb note was logged by year two");
}
