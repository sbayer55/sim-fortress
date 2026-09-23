//! Unit tests for the daily ecology update (C2) and succession (C2 FR12).

#![allow(clippy::float_cmp)]

use super::*;
use crate::sim::{Cell, Params, Sim};

fn run_days(sim: &mut Sim, days: u64) {
    for _ in 0..(days * 24) {
        sim.step();
    }
}

#[test]
fn growth_saturates_at_season_target() {
    let mut sim = Sim::new(42, Params::default());
    run_days(&mut sim, 720);
    let eco = &sim.params.ecology;
    for c in &sim.world.cells {
        if c.terrain.is_water() {
            assert_eq!(c.vegetation, 0.0);
            continue;
        }
        let max_v = eco.max_vegetation.get(&c.terrain).copied().unwrap_or(0.0);
        assert!(c.vegetation <= max_v + 0.001, "{:?} veg {} exceeds cap {}", c.terrain, c.vegetation, max_v);
    }
    assert!(sim.series.last().unwrap().veg_mean > 0.1, "vegetation did not grow");
}

#[test]
fn winter_lowers_equilibrium() {
    let mut sim = Sim::new(42, Params::default());
    run_days(&mut sim, 720);
    let samples = sim.series.samples();
    let summer: f32 = samples.iter().filter(|s| (450..540).contains(&s.day)).map(|s| s.veg_mean).sum::<f32>() / 90.0;
    let winter: f32 = samples.iter().filter(|s| (630..720).contains(&s.day)).map(|s| s.veg_mean).sum::<f32>() / 90.0;
    assert!(winter < summer * 0.8, "winter {winter} not much below summer {summer}");
}

#[test]
fn moisture_equilibria_by_rainfall() {
    let means: Vec<f32> = [Rainfall::Dry, Rainfall::Normal, Rainfall::Wet]
        .iter()
        .map(|&rf| {
            let mut p = Params::default();
            p.world.rainfall = rf;
            p.species.clear_initial_counts(); // pure ecology test, no grazing
            let mut sim = Sim::new(42, p);
            run_days(&mut sim, 720);
            let (s, n) = sim
                .world
                .cells
                .iter()
                .filter(|c| !c.terrain.is_water())
                .fold((0.0f32, 0usize), |(s, n), c| (s + c.moisture, n + 1));
            s / crate::cast!(n => f32)
        })
        .collect();
    assert!(means[0] < means[1], "dry {} not < normal {}", means[0], means[1]);
    assert!(means[1] < means[2], "normal {} not < wet {}", means[1], means[2]);
    // The rain sequence follows the ecology RNG stream, which regrowth-site
    // sampling advances; C4's `growth_k` retune moved the dry mean from ~0.48 to ~0.56,
    // and the worldgen history phase (events reshape seed 42's regions) to ~0.60.
    assert!(means[0] < 0.65, "dry too wet: {}", means[0]);
    assert!(means[2] > 0.8, "wet too dry: {}", means[2]);
}

#[test]
fn drought_flags_after_n_days_and_recovers() {
    let mut sim = Sim::new(42, Params::default());
    for c in &mut sim.world.cells {
        if !c.terrain.is_water() {
            c.moisture = 0.0;
        }
    }
    run_days(&mut sim, 8);
    assert!(sim.drought.iter().any(|&f| f), "expected at least one region flagged");
    for c in &mut sim.world.cells {
        if !c.terrain.is_water() {
            c.moisture = 1.0;
        }
    }
    run_days(&mut sim, 1);
    assert!(sim.drought.iter().all(|&f| !f), "expected drought to clear");
}

#[test]
fn water_dries_and_refills_with_marker() {
    // Ecology only: creatures (grazing, and since C5 predation) perturb the
    // vegetation the rain stream is drawn against, so keep the world empty.
    let mut p = Params::default();
    p.species.clear_initial_counts();
    let mut sim = Sim::new(42, p);
    for c in &mut sim.world.cells {
        if !c.terrain.is_water() {
            c.moisture = 0.0;
        }
    }
    // Flag a drought, then keep running so water dries to sand.
    run_days(&mut sim, 20);
    let dried = sim.world.cells.iter().filter(|c| c.dried_from == Some(Terrain::ShallowWater)).count();
    assert!(dried > 0, "expected some shallow water to dry to sand");
    // Refill.
    for c in &mut sim.world.cells {
        if !c.terrain.is_water() {
            c.moisture = 1.0;
        }
    }
    // Refill is rate-limited to `water_changes_per_region_per_day` cells/day.
    run_days(&mut sim, 30);
    let still_dried = sim.world.cells.iter().filter(|c| c.dried_from == Some(Terrain::ShallowWater)).count();
    assert_eq!(still_dried, 0, "expected dried cells to refill");
}

#[test]
fn seeds_sprout_and_clear_on_dirt() {
    let mut p = Params::default();
    p.species.clear_initial_counts(); // pure ecology test, no grazing
    let mut sim = Sim::new(42, p);
    // Sites sprout on bare dirt and sparse grass; strip both so every
    // seed's world offers plenty of candidates.
    for c in &mut sim.world.cells {
        if matches!(c.terrain, Terrain::Dirt | Terrain::GrassSparse) {
            c.vegetation = 0.0;
        }
    }
    run_days(&mut sim, 10);
    assert!(!sim.world.seeds.is_empty(), "expected regrowth sites to sprout");
    // A site whose vegetation reaches the clear threshold is removed.
    let (sx, sy) = sim.world.seeds[0];
    sim.world.cells[sy * sim.world.width + sx].vegetation = 0.5;
    run_days(&mut sim, 1);
    assert!(!sim.world.seeds.contains(&(sx, sy)), "expected the site to clear");
}

#[test]
fn one_seed_note_per_region_per_day() {
    let mut sim = Sim::new(42, Params::default());
    for c in &mut sim.world.cells {
        if matches!(c.terrain, Terrain::Dirt | Terrain::GrassSparse) {
            c.vegetation = 0.0;
        }
    }
    let before = sim.events.len();
    run_days(&mut sim, 1);
    // Only the regrowth notes: den discoveries are notes too and land on
    // the same day on some worlds.
    let notes: Vec<&Event> = sim.events.iter().skip(before).filter(|e| e.kind == EventKind::Note && e.text.contains("regrowth site")).collect();
    assert!(!notes.is_empty());
    assert!(notes.len() <= 8, "{} notes in one day", notes.len());
    let mut texts: Vec<&str> = notes.iter().map(|e| e.text.as_str()).collect();
    texts.sort_unstable();
    texts.dedup();
    assert_eq!(texts.len(), notes.len(), "duplicate region notes in one day");
}

#[test]
fn region_means_exclude_water() {
    let cells = vec![
        Cell { terrain: Terrain::ShallowWater, biome: Biome::Grassland, elevation: 0.5, moisture: 0.5, temperature: 0.5, vegetation: 0.9, prey_pressure: 0.0, pred_pressure: 0.0, dried_from: None, parasite_load: 0.0, thrive_days: 0, wear_days: 0 },
        Cell { terrain: Terrain::Grass, biome: Biome::Grassland, elevation: 0.5, moisture: 0.5, temperature: 0.5, vegetation: 0.1, prey_pressure: 0.0, pred_pressure: 0.0, dried_from: None, parasite_load: 0.0, thrive_days: 0, wear_days: 0 },
        Cell { terrain: Terrain::Grass, biome: Biome::Grassland, elevation: 0.5, moisture: 0.5, temperature: 0.5, vegetation: 0.3, prey_pressure: 0.0, pred_pressure: 0.0, dried_from: None, parasite_load: 0.0, thrive_days: 0, wear_days: 0 },
    ];
    let world = World {
        cells,
        width: 3,
        height: 1,
        dens: vec![],
        carcasses: vec![],
        seeds: vec![],
        regions: vec![("R".to_string(), 0, 0, 3, 1)],
        region_map: vec![],
        wind: crate::sim::world::Wind::Westerly,
        water_cells_at_generation: 1,
        shore: vec![],
        falls: vec![],
        history: vec![],
        names: crate::sim::world::Names::default(),
        scent: vec![],
    };
    let mean = region_land_veg_mean(&world, 0);
    assert!((mean - 0.2).abs() < 1e-6, "land veg mean {mean} should exclude the water cell");
}

#[test]
fn drought_event_has_region_centre_pos() {
    let mut sim = Sim::new(42, Params::default());
    for c in &mut sim.world.cells {
        if !c.terrain.is_water() {
            c.moisture = 0.0;
        }
    }
    run_days(&mut sim, 8);
    let droughts: Vec<&Event> = sim.events.iter().filter(|e| e.kind == EventKind::Drought).collect();
    assert!(!droughts.is_empty());
    for e in droughts {
        let pos = e.pos.expect("drought event should carry a position");
        let is_centre = (0..sim.world.regions.len()).any(|ri| sim.world.region_centre(ri) == pos);
        assert!(is_centre, "pos {pos:?} is not a region centre");
    }
}

// ---- C2 FR12: succession and trampling ----

use crate::sim::world::Biome;

/// A creature-free sim so nothing but the ecology touches the cells.
fn empty_sim(seed: u64, p: Params) -> Sim {
    let mut p = p;
    p.species.clear_initial_counts();
    Sim::new(seed, p)
}

/// Index of the first cell of `terrain`, made lush, moist and untrodden in
/// a wooded biome so only the fields a test sets decide its fate.
fn prime(sim: &mut Sim, terrain: Terrain) -> usize {
    let idx = sim.world.cells.iter().position(|c| c.terrain == terrain).expect("terrain present");
    let c = &mut sim.world.cells[idx];
    c.biome = Biome::Grassland;
    c.moisture = 1.0;
    c.vegetation = 1.0;
    c.prey_pressure = 0.0;
    c.thrive_days = 0;
    c.wear_days = 0;
    idx
}

#[test]
fn trampled_cell_grows_toward_a_lower_target() {
    let mut on = empty_sim(42, Params::default());
    let mut off = on.clone();
    off.params.succession.neutral();
    let idx = prime(&mut on, Terrain::Grass);
    prime(&mut off, Terrain::Grass);
    on.world.cells[idx].prey_pressure = 1.0;
    off.world.cells[idx].prey_pressure = 1.0;
    run_days(&mut on, 3);
    run_days(&mut off, 3);
    assert!(on.world.cells[idx].vegetation < off.world.cells[idx].vegetation, "trampled {} vs untrampled {}", on.world.cells[idx].vegetation, off.world.cells[idx].vegetation);
    assert_eq!(on.world.cells[idx].terrain, Terrain::Grass, "trampling alone never flips a cell");
}

#[test]
fn thriving_meadow_climbs_after_climb_days() {
    let mut p = Params::default();
    p.succession.flip_chance = 1.0;
    let mut sim = empty_sim(42, p);
    let idx = prime(&mut sim, Terrain::GrassDense);
    let need = sim.params.succession.climb_days[&Terrain::Forest];
    sim.world.cells[idx].thrive_days = crate::cast!(need => u16) - 1;
    run_days(&mut sim, 1);
    let c = &sim.world.cells[idx];
    assert_eq!(c.terrain, Terrain::Forest, "one more thriving day ripens the cell and the roll is certain");
    assert_eq!((c.thrive_days, c.wear_days), (0, 0), "both counters reset on a flip");
    assert!(c.vegetation <= sim.params.ecology.max_vegetation[&Terrain::Forest], "vegetation clamped to the new cap: {}", c.vegetation);
}

#[test]
fn worn_grass_drops_after_wear_days() {
    let mut p = Params::default();
    p.succession.flip_chance = 1.0;
    let mut sim = empty_sim(42, p);
    let idx = prime(&mut sim, Terrain::Grass);
    let c = &mut sim.world.cells[idx];
    c.vegetation = 0.0;
    c.prey_pressure = 1.0; // decays to 0.85 at the day boundary, still above trample_high
    c.wear_days = crate::cast!(sim.params.succession.wear_days_needed => u16) - 1;
    run_days(&mut sim, 1);
    assert_eq!(sim.world.cells[idx].terrain, Terrain::GrassSparse);
    assert_eq!(sim.world.cells[idx].wear_days, 0);
}

#[test]
fn drought_alone_never_wears() {
    let mut p = Params::default();
    p.succession.flip_chance = 1.0;
    let mut sim = empty_sim(42, p);
    let idx = prime(&mut sim, Terrain::Grass);
    sim.world.cells[idx].vegetation = 0.0;
    sim.world.cells[idx].moisture = 0.0;
    run_days(&mut sim, 10);
    let c = &sim.world.cells[idx];
    assert_eq!(c.wear_days, 0, "bare but untrodden ground is not worn");
    assert_eq!(c.terrain, Terrain::Grass);
}

#[test]
fn relax_decays_both_counters() {
    let mut sim = empty_sim(42, Params::default());
    let idx = prime(&mut sim, Terrain::Grass);
    let c = &mut sim.world.cells[idx];
    // 0.2 decays 0.17, 0.144, 0.123: between trample_low and trample_high all three days.
    c.prey_pressure = 0.2;
    c.thrive_days = 10;
    c.wear_days = 4;
    run_days(&mut sim, 3);
    let c = &sim.world.cells[idx];
    assert_eq!((c.thrive_days, c.wear_days), (7, 1), "neither signal: both fall by relax_per_day");
}

#[test]
fn forest_needs_a_wooded_biome() {
    let mut p = Params::default();
    p.succession.flip_chance = 1.0;
    let mut sim = empty_sim(42, p);
    let idx = prime(&mut sim, Terrain::GrassDense);
    sim.world.cells[idx].biome = Biome::Tundra;
    sim.world.cells[idx].thrive_days = 1000;
    run_days(&mut sim, 2);
    let c = &sim.world.cells[idx];
    assert_eq!(c.terrain, Terrain::GrassDense, "meadow is the top of a treeless biome's ladder");
    assert!(c.thrive_days < 1000, "with no rung to climb into the cell is not thriving and the counter relaxes");
}

#[test]
fn climb_needs_moisture() {
    let mut p = Params::default();
    p.succession.flip_chance = 1.0;
    let mut sim = empty_sim(42, p);
    let idx = prime(&mut sim, Terrain::Grass);
    sim.world.cells[idx].moisture = 0.0;
    sim.world.cells[idx].thrive_days = 1000;
    run_days(&mut sim, 1);
    assert_eq!(sim.world.cells[idx].terrain, Terrain::Grass, "a parched cell never climbs");
}

#[test]
fn sand_marsh_rock_and_water_never_flip() {
    let mut p = Params::default();
    p.succession.flip_chance = 1.0;
    let mut sim = empty_sim(42, p);
    let before: Vec<Terrain> = sim.world.cells.iter().map(|c| c.terrain).collect();
    for c in &mut sim.world.cells {
        c.moisture = 1.0;
        c.thrive_days = 1000;
        c.wear_days = 1000;
    }
    run_days(&mut sim, 1);
    let mut flipped = 0;
    for (c, was) in sim.world.cells.iter().zip(&before) {
        if c.dried_from.is_some() || (c.terrain.is_water() && *was == Terrain::Sand) {
            continue; // a dry wash refilling under the moisture this test poured on: drought's rule, not ours
        }
        if !was.on_ladder() {
            assert_eq!(c.terrain, *was, "{} is off the ladder", was.name());
        } else if c.terrain != *was {
            flipped += 1;
        }
    }
    assert!(flipped > 0, "the ladder cells did flip");
}

#[test]
fn one_note_per_region_per_day_per_direction() {
    let mut p = Params::default();
    p.succession.flip_chance = 1.0;
    let mut sim = empty_sim(42, p);
    for c in &mut sim.world.cells {
        if c.terrain == Terrain::Grass {
            c.biome = Biome::Grassland;
            c.moisture = 1.0;
            c.vegetation = 1.0;
            c.prey_pressure = 0.0;
            c.thrive_days = 1000;
        }
    }
    let regions = sim.world.regions.len().min(8);
    run_days(&mut sim, 1);
    let notes = sim.events.iter().filter(|e| e.kind == EventKind::Note && e.text.starts_with("Scrub is closing over")).count();
    assert!(notes >= 1 && notes <= regions, "{notes} climb notes for {regions} regions");
    assert!(sim.events.iter().all(|e| !e.text.starts_with("Grazing wears")), "nothing wore today");
}

/// The neutral overlay leaves the vegetation target untouched to the bit and
/// makes no draw, so a run under it is the pre-succession run: the checksum
/// `sim::tests::checksum_is_fnv_stable` pinned before FR12 landed.
#[test]
fn neutral_succession_reproduces_the_old_checksum() {
    let mut p = Params::default();
    p.succession.neutral();
    let mut sim = Sim::new(7, p);
    for _ in 0..8640 {
        sim.step();
    }
    assert_eq!(sim.checksum(), 0xf9ea_eb02_3e27_c085);
    assert!(sim.events.iter().all(|e| !(e.text.starts_with("Scrub is closing") || e.text.starts_with("Grazing wears"))));
}
