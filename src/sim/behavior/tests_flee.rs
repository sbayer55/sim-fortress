//! Tests for the flee response.

use crate::sim::creatures::{
    CreatureStore, Goal, 
};
use crate::sim::params::{CreaturesParams, PredationParams, SocialParams};
use crate::sim::spatial::SpatialIndex;
use crate::sim::species::{SpeciesId};
use super::movement::{move_toward };
use super::threat::mark_threats;
use super::tests::{all_grass_world, day_time, test_creature, test_world, tick_once};


#[test]
fn flee_query_is_predator_first() {
    let w = test_world();
    let mut store = CreatureStore::new();
    let mut wolf = test_creature(75, 20);
    wolf.species = SpeciesId::Wolf;
    wolf.genome = SpeciesId::Wolf.base_genome();
    wolf.hunger = 0.9; // hungry wolves are a danger within chase range
    let mut vole = test_creature(76, 20);
    vole.species = SpeciesId::Vole;
    vole.genome = SpeciesId::Vole.base_genome();
    store.insert(wolf);
    store.insert(vole);
    let mut idx = SpatialIndex::new(&w);
    idx.rebuild(&store, &w);
    // A second, nearer wolf with a higher id: the nearest detected threat wins
    // and both count toward predation risk.
    let mut near = test_creature(76, 20); // same cell → distance 0 (the first wolf is 0.5 away)
    near.species = SpeciesId::Wolf;
    near.genome = SpeciesId::Wolf.base_genome();
    near.genome.0[5] = 0.0;
    near.hunger = 0.9;
    store.insert(near);
    let mut idx = SpatialIndex::new(&w);
    idx.rebuild(&store, &w);
    mark_threats(&mut store, &idx, &w, 0, &PredationParams::default(), &SocialParams::default());
    let vole_id = store.living().find(|c| c.species == SpeciesId::Vole).unwrap().id;
    let v = store.get(vole_id).unwrap();
    assert!(v.threatened_by.is_some(), "nearby vole should be threatened by a wolf");
    assert_eq!(v.threatened_by.map(|t| (t.0, t.1)), Some((76, 20)), "nearest detected predator drives the away-vector");
    assert!(v.predation_risk >= 0.5 * 2.0 / 3.0 - 1e-6, "both predators in range count: {}", v.predation_risk);
}

#[test]
fn flee_reacts_within_one_tick() {
    let mut w = all_grass_world();
    let mut store = CreatureStore::new();
    let mut wolf = test_creature(20, 5);
    wolf.species = SpeciesId::Wolf;
    wolf.genome = SpeciesId::Wolf.base_genome();
    wolf.genome.0[5] = 0.0; // no camouflage
    wolf.hunger = 0.9; // hungry: a danger within chase range even before it hunts
    let mut hare = test_creature(22, 5); // dx 2 → distance 1
    hare.species = SpeciesId::Hare;
    hare.genome = SpeciesId::Hare.base_genome();
    hare.genome.0[2] = 0.6; // sense > wolf camouflage
    store.insert(wolf);
    let hare_id = store.insert(hare);
    let pp = PredationParams::default();
    tick_once(&mut store, &mut w, &day_time(12), &pp);
    let h = store.get(hare_id).unwrap();
    assert_eq!(h.goal, Goal::Flee, "detection pre-empts every goal within the tick");
    assert_eq!(h.chased, 1);
    assert_eq!(h.threats_by_species[SpeciesId::Wolf.index()], 1);
    assert!(h.x > 22, "moved away along the predator→prey vector: x = {}", h.x);
}

#[test]
fn flee_costs_energy() {
    let w = all_grass_world();
    let cp = CreaturesParams { move_speed_base: 2.0, move_speed_per_trait: 0.0, ..CreaturesParams::default() };
    let pp = PredationParams::default();
    let mut wander = test_creature(0, 5);
    wander.target = Some((30, 5));
    wander.goal = Goal::Wander;
    let e0 = wander.energy;
    move_toward(&mut wander, &w, &day_time(12), &cp, &pp, 1.0);
    let wander_cost = e0 - wander.energy;
    assert!(wander_cost > 0.0);

    let mut flee = test_creature(0, 5);
    flee.target = Some((30, 5));
    flee.goal = Goal::Flee;
    let e0 = flee.energy;
    move_toward(&mut flee, &w, &day_time(12), &cp, &pp, 1.0);
    let flee_cost = e0 - flee.energy;
    assert_eq!(flee.x, wander.x, "same steps taken");
    assert!((flee_cost - wander_cost * pp.flee_energy_factor).abs() < 1e-6, "flee steps cost flee_energy_factor × move_cost_energy");
}
