//! Tests for needs, movement, goals, dens and pressure.

use crate::sim::creatures::{
    CreatureStore, DeathTallies, Goal, RestReason,
};
use crate::sim::events::{EventRing};
use crate::sim::lineage::Lineage;
use crate::sim::params::{CreaturesParams, EcologyParams, GeneticsParams, PredationParams };
use crate::sim::rng::Rng;
use crate::sim::species::{SpeciesId};
use crate::sim::disease::{DiseaseState};
use crate::sim::params::DiseaseParams;
use crate::sim::world::{Terrain };
use super::{day_boundary, needs};
use super::movement::{move_toward };
use super::vitals::{maybe_make_den};
use super::death::pressure;
use super::tests::{all_grass_world, day_time, empty_index, plan, test_creature, test_world};

use crate::sim::creatures::{place_founders  };
use crate::sim::{Params, Sim};

#[test]
fn drink_goal_when_thirsty() {
    let w = test_world();
    let mut c = test_creature(75, 20);
    c.thirst = 0.9;
    c.last_water = Some((74, 20));
    let idx = empty_index(&w);
    let mut rng = Rng::new(1);
    plan(&mut c, &idx, &w, &day_time(12), &CreaturesParams::default(), &mut rng);
    assert_eq!(c.goal, Goal::Drink);
}

#[test]
fn graze_hysteresis() {
    let w = test_world();
    let idx = empty_index(&w);
    let mut rng = Rng::new(1);
    let cp = CreaturesParams::default();

    let mut c = test_creature(75, 20);
    c.hunger = 0.6;
    plan(&mut c, &idx, &w, &day_time(12), &cp, &mut rng);
    assert_eq!(c.goal, Goal::Graze, "hunger 0.6 should graze");

    // Still above the exit threshold (0.2) but below the entry (0.5): stay grazing.
    c.hunger = 0.3;
    plan(&mut c, &idx, &w, &day_time(12), &cp, &mut rng);
    assert_eq!(c.goal, Goal::Graze, "hysteresis should keep grazing at hunger 0.3");

    // Below the exit threshold: leave Graze.
    c.hunger = 0.1;
    plan(&mut c, &idx, &w, &day_time(12), &cp, &mut rng);
    assert_ne!(c.goal, Goal::Graze, "hunger 0.1 should stop grazing");
}

#[test]
fn rest_at_night() {
    let w = test_world();
    let mut c = test_creature(75, 20);
    c.energy = 0.8;
    c.hunger = 0.3;
    c.thirst = 0.3;
    let idx = empty_index(&w);
    let mut rng = Rng::new(1);
    plan(&mut c, &idx, &w, &day_time(22), &CreaturesParams::default(), &mut rng);
    assert_eq!(c.goal, Goal::Rest);
    assert_eq!(c.rest_reason, Some(RestReason::Night));
}

#[test]
fn forced_rest_at_zero_energy() {
    let w = test_world();
    let mut c = test_creature(75, 20);
    c.energy = 0.0;
    c.hunger = 0.9;
    c.thirst = 0.9;
    let idx = empty_index(&w);
    let mut rng = Rng::new(1);
    plan(&mut c, &idx, &w, &day_time(12), &CreaturesParams::default(), &mut rng);
    assert_eq!(c.goal, Goal::Rest);
    assert_eq!(c.rest_reason, Some(RestReason::Forced));
}

#[test]
fn fractional_movement_budget() {
    let w = all_grass_world();
    let mut c = test_creature(0, 5);
    c.target = Some((30, 5));
    c.goal = Goal::Wander;
    c.move_budget = 0.9;
    let cp = CreaturesParams { move_speed_base: 0.1, move_speed_per_trait: 1.0, ..CreaturesParams::default() };
    // speed = 0.1 + 1.0*genome.speed(0.45) = 0.55; budget 0.9+0.55 = 1.45 → 1 step.
    move_toward(&mut c, &w, &day_time(12), &cp, &PredationParams::default(), 1.0);
    assert_eq!(c.x, 1, "one step should be taken");
    assert!((c.move_budget - 0.45).abs() < 1e-4, "budget should carry the remainder, got {}", c.move_budget);
}

#[test]
fn needs_tick_rates() {
    let w = test_world();
    let mut c = test_creature(75, 20);
    let cp = CreaturesParams::default();
    let ep = EcologyParams::default();
    let t = day_time(12);
    let before_h = c.hunger;
    let before_t = c.thirst;
    let before_e = c.energy;
    needs(&mut c, &w, &t, &cp, &ep, &GeneticsParams::default(), &DiseaseParams::default(), 1.0);
    let season = ep.season_metabolism.get(&t.season()).copied().unwrap_or(1.0);
    let expect = cp.hunger_per_hour(c.genome.size(), c.genome.metabolism(), season);
    assert!((c.hunger - before_h - expect).abs() < 1e-6);
    assert!((c.thirst - before_t - cp.thirst_per_hour).abs() < 1e-6);
    assert!((c.energy - (before_e - cp.energy_awake_per_hour)).abs() < 1e-6);
}

#[test]
fn trail_cap() {
    let w = all_grass_world();
    let mut c = test_creature(0, 5);
    c.target = Some((40, 5));
    c.goal = Goal::Wander;
    let cp = CreaturesParams { move_speed_base: 2.0, move_speed_per_trait: 0.0, trail_len: 12, ..CreaturesParams::default() };
    // Walk far enough to exceed the trail cap.
    for _ in 0..20 {
        move_toward(&mut c, &w, &day_time(12), &cp, &PredationParams::default(), 1.0);
    }
    assert!(c.trail.len() <= cp.trail_len, "trail {} exceeds cap {}", c.trail.len(), cp.trail_len);
    assert_eq!(c.trail.len(), cp.trail_len);
}

#[test]
fn den_creation_capped() {
    let mut w = test_world();
    let mut events = EventRing::new(100);
    let mut rng = Rng::new(1);
    // Force den creation (chance 1.0), cap 1 per region.
    let cp = CreaturesParams { den_create_chance_per_rest_hour: 1.0, max_dens_per_region: 1, ..CreaturesParams::default() };
    let t = day_time(2); // night → rest

    // Resting creature on a bare Dirt cell (vegetation < 0.2).
    let mut c = test_creature(40, 5);
    c.goal = Goal::Rest;
    c.rest_reason = Some(RestReason::Night);
    c.target = None;
    w.cell_mut(40, 5).terrain = Terrain::Dirt;
    w.cell_mut(40, 5).vegetation = 0.0;
    maybe_make_den(&c, &mut w, &mut events, &t, &cp, &mut rng);
    assert_eq!(w.dens.len(), 1);

    // Second resting creature in the same region: capped.
    let mut c2 = test_creature(41, 5);
    c2.goal = Goal::Rest;
    c2.rest_reason = Some(RestReason::Night);
    c2.target = None;
    w.cell_mut(41, 5).terrain = Terrain::Dirt;
    w.cell_mut(41, 5).vegetation = 0.0;
    maybe_make_den(&c2, &mut w, &mut events, &t, &cp, &mut rng);
    assert_eq!(w.dens.len(), 1, "region den cap should hold");
}

#[test]
fn path_search_rounds_an_obstacle() {
    let mut w = all_grass_world();
    // A rock wall at x = 10 spanning rows 2..=8 with a gap at row 9.
    for y in 2..=8 {
        w.cell_mut(10, y).terrain = Terrain::Rock;
    }
    let mut c = test_creature(8, 5);
    c.target = Some((12, 5));
    c.goal = Goal::Drink;
    let cp = CreaturesParams { move_speed_base: 1.0, move_speed_per_trait: 0.0, ..CreaturesParams::default() };
    for _ in 0..30 {
        move_toward(&mut c, &w, &day_time(12), &cp, &PredationParams::default(), 1.0);
        if (c.x, c.y) == (12, 5) {
            break;
        }
    }
    assert_eq!((c.x, c.y), (12, 5), "creature should route around the wall via the gap");
    assert!(c.target.is_some(), "target is kept for non-wander goals");
}

#[test]
fn unreachable_target_is_dropped() {
    let mut w = all_grass_world();
    // Fully enclose the target.
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx != 0 || dy != 0 {
                w.cell_mut(crate::cast!((30 + dx) => usize), crate::cast!((5 + dy) => usize)).terrain = Terrain::Rock;
            }
        }
    }
    let mut c = test_creature(20, 5);
    c.target = Some((30, 5));
    c.goal = Goal::Drink;
    c.last_water = Some((30, 5));
    let cp = CreaturesParams { move_speed_base: 1.0, move_speed_per_trait: 0.0, ..CreaturesParams::default() };
    for _ in 0..40 {
        move_toward(&mut c, &w, &day_time(12), &cp, &PredationParams::default(), 1.0);
    }
    assert!(c.target.is_none(), "unreachable target must be dropped");
    assert!(c.last_water.is_none(), "an unreachable water memory is forgotten");
}

#[test]
fn movement_never_impassable() {
    let mut sim = Sim::new(42, Params::default());
    for _ in 0..24 * 120 {
        sim.step();
    }
    for c in sim.creatures.living() {
        let cell = sim.world.cell(c.x, c.y);
        assert!(cell.terrain.walkable(), "creature {} on {:?}", c.id.0, cell.terrain);
    }
}

#[test]
fn founders_are_placed() {
    let sim = Sim::new(42, Params::default());
    let total: u32 = sim.params.creatures.initial_counts.values().sum();
    assert_eq!(sim.creatures.len_living(), crate::cast!(total => usize));
    let _ = place_founders;
}

#[test]
fn pressure_decay() {
    let mut w = test_world();
    let mut store = CreatureStore::new();
    let mut events = EventRing::new(10);
    let mut tallies = DeathTallies::default();
    w.cell_mut(50, 10).prey_pressure = 1.0;
    w.cell_mut(50, 10).pred_pressure = 0.0;
    let cp = CreaturesParams::default();
    day_boundary(&mut store, &mut w, &mut events, &day_time(0), &cp, &GeneticsParams::default(), &DiseaseParams::default(), &mut tallies, &mut Lineage::new(), &mut DiseaseState::new(&DiseaseParams::default()), &mut Rng::new(1));
    assert!((w.cell(50, 10).prey_pressure - cp.pressure_decay_per_day).abs() < 1e-6);
}

#[test]
fn pressure_clamped() {
    let mut w = test_world();
    let cp = CreaturesParams { pressure_per_creature_tick: 1.0, ..CreaturesParams::default() };
    let mut wolf = test_creature(75, 20);
    wolf.species = SpeciesId::Wolf;
    pressure(&wolf, &mut w, &cp);
    assert_eq!(w.cell(75, 20).pred_pressure, 1.0, "predator traffic clamps at 1.0");
    let mut vole = test_creature(75, 20);
    vole.species = SpeciesId::Vole;
    pressure(&vole, &mut w, &cp);
    assert_eq!(w.cell(75, 20).prey_pressure, 1.0, "prey traffic clamps at 1.0");
}

#[test]
fn nocturnal_rest_by_day() {
    let w = test_world();
    let idx = empty_index(&w);
    let mut rng = Rng::new(1);
    let cp = CreaturesParams::default();
    // Fox (nocturnal) rests during the day.
    let mut fox = test_creature(75, 20);
    fox.species = SpeciesId::Fox;
    plan(&mut fox, &idx, &w, &day_time(12), &cp, &mut rng);
    assert_eq!(fox.goal, Goal::Rest, "nocturnal fox rests by day");
    // Wolf (diurnal) rests at night.
    let mut wolf = test_creature(75, 20);
    wolf.species = SpeciesId::Wolf;
    plan(&mut wolf, &idx, &w, &day_time(22), &cp, &mut rng);
    assert_eq!(wolf.goal, Goal::Rest, "diurnal wolf rests at night");
}
