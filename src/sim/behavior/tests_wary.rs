//! Tests for the C5 `FR5b` wary tier: the low-exertion avoidance of a predator
//! that is not currently hunting the prey.

use crate::sim::creatures::{CreatureStore, DeathTallies, Goal, RestReason};
use crate::sim::events::{EventKind, EventRing};
use crate::sim::params::{CreaturesParams, PredationParams, SocialParams};
use crate::sim::spatial::SpatialIndex;
use crate::sim::species::SpeciesId;

use super::flush_wary;
use super::movement::move_toward;
use super::tests::{all_grass_world, day_time, test_creature, tick_once};
use super::threat::mark_threats;

/// A satiated wolf the prey can see: not a danger, so the prey goes Wary.
#[test]
fn unhunted_predator_triggers_wary() {
    let mut w = all_grass_world();
    let mut store = CreatureStore::new();
    let mut wolf = test_creature(20, 5);
    wolf.species = SpeciesId::Wolf;
    wolf.genome = SpeciesId::Wolf.base_genome();
    wolf.genome.0[5] = 0.0; // no camouflage
    wolf.hunger = 0.1; // below hunt_hunger_min: seen, but not a danger
    let mut hare = test_creature(22, 5);
    hare.species = SpeciesId::Hare;
    hare.genome = SpeciesId::Hare.base_genome();
    hare.genome.0[2] = 0.6; // sense > wolf camouflage
    store.insert(wolf);
    let hare_id = store.insert(hare);
    let pp = PredationParams::default();
    tick_once(&mut store, &mut w, &day_time(12), &pp);
    let h = store.get(hare_id).unwrap();
    assert_eq!(h.goal, Goal::Wary, "a non-danger predator is avoided, not fled");
    assert_eq!(h.wary_count, 1, "one wary encounter");
    assert_eq!(h.chased, 0, "the wary tier is not a chase");
    assert!(h.wary_by.is_some(), "the away-vector is set");
    assert!(h.target.is_some_and(|(tx, _)| tx > 22), "the waypoint is away from the wolf: {:?}", h.target);
}

/// A hungry predator inside `chase_trigger_cheb` stays a flee trigger: the
/// tiers never overlap.
#[test]
fn hunted_prey_flees_and_never_turns_wary() {
    let mut w = all_grass_world();
    let mut store = CreatureStore::new();
    let mut wolf = test_creature(20, 5);
    wolf.species = SpeciesId::Wolf;
    wolf.genome = SpeciesId::Wolf.base_genome();
    wolf.genome.0[5] = 0.0;
    wolf.hunger = 0.9; // hungry: a danger within chase_trigger_cheb
    let mut hare = test_creature(22, 5);
    hare.species = SpeciesId::Hare;
    hare.genome = SpeciesId::Hare.base_genome();
    hare.genome.0[2] = 0.6;
    store.insert(wolf);
    let hare_id = store.insert(hare);
    let pp = PredationParams::default();
    tick_once(&mut store, &mut w, &day_time(12), &pp);
    let h = store.get(hare_id).unwrap();
    assert_eq!(h.goal, Goal::Flee, "a danger escalates straight to Flee");
    assert_eq!(h.chased, 1);
    assert_eq!(h.wary_count, 0, "wary is never entered on the flee tick");
}

/// `FR5b` pre-empts every goal except a forced rest.
#[test]
fn wary_pre_empts_drinking() {
    let mut w = all_grass_world();
    let mut store = CreatureStore::new();
    let mut wolf = test_creature(20, 5);
    wolf.species = SpeciesId::Wolf;
    wolf.genome = SpeciesId::Wolf.base_genome();
    wolf.genome.0[5] = 0.0;
    wolf.hunger = 0.1;
    let mut hare = test_creature(22, 5);
    hare.species = SpeciesId::Hare;
    hare.genome = SpeciesId::Hare.base_genome();
    hare.genome.0[2] = 0.6;
    hare.goal = Goal::Drink;
    hare.thirst = 0.9;
    store.insert(wolf);
    let hare_id = store.insert(hare);
    let pp = PredationParams::default();
    tick_once(&mut store, &mut w, &day_time(12), &pp);
    let h = store.get(hare_id).unwrap();
    assert_eq!(h.goal, Goal::Wary, "a thirsty prey still gives the wolf room");
    assert_eq!(h.wary_count, 1);
}

/// A creature at energy 0 is exempt: forced rest is the one goal wary yields to.
#[test]
fn forced_rest_is_not_interrupted() {
    let mut w = all_grass_world();
    let mut store = CreatureStore::new();
    let mut wolf = test_creature(20, 5);
    wolf.species = SpeciesId::Wolf;
    wolf.genome = SpeciesId::Wolf.base_genome();
    wolf.genome.0[5] = 0.0;
    wolf.hunger = 0.1;
    let mut hare = test_creature(22, 5);
    hare.species = SpeciesId::Hare;
    hare.genome = SpeciesId::Hare.base_genome();
    hare.genome.0[2] = 0.6;
    hare.goal = Goal::Rest;
    hare.rest_reason = Some(RestReason::Forced);
    hare.energy = 0.0;
    store.insert(wolf);
    let hare_id = store.insert(hare);
    let pp = PredationParams::default();
    tick_once(&mut store, &mut w, &day_time(12), &pp);
    let h = store.get(hare_id).unwrap();
    assert_eq!(h.goal, Goal::Rest, "an exhausted prey keeps resting");
    assert_eq!(h.wary_count, 0, "the forced rest never enters the tier");
}

/// `wary_distance = 0.0` is the kill switch for the whole tier.
#[test]
fn wary_distance_zero_disables_the_tier() {
    let mut w = all_grass_world();
    let mut store = CreatureStore::new();
    let mut wolf = test_creature(20, 5);
    wolf.species = SpeciesId::Wolf;
    wolf.genome = SpeciesId::Wolf.base_genome();
    wolf.genome.0[5] = 0.0;
    wolf.hunger = 0.1;
    let mut hare = test_creature(22, 5);
    hare.species = SpeciesId::Hare;
    hare.genome = SpeciesId::Hare.base_genome();
    hare.genome.0[2] = 0.6;
    store.insert(wolf);
    let hare_id = store.insert(hare);
    let pp = PredationParams { wary_distance: 0.0, ..PredationParams::default() };
    tick_once(&mut store, &mut w, &day_time(12), &pp);
    let h = store.get(hare_id).unwrap();
    assert_ne!(h.goal, Goal::Wary, "the tier is off");
    assert_eq!(h.wary_count, 0);
    assert!(h.wary_by.is_none());
}

/// The retained away-vector survives undetected ticks inside the release
/// radius and is dropped past it (hysteresis).
#[test]
fn wary_is_retained_within_the_release_radius() {
    let w = all_grass_world();
    let mut store = CreatureStore::new();
    let mut prey = test_creature(22, 5);
    prey.species = SpeciesId::Hare;
    prey.genome = SpeciesId::Hare.base_genome();
    prey.goal = Goal::Wary;
    prey.wary_until = 10;
    prey.wary_by = Some((20, 5, SpeciesId::Wolf));
    let id = store.insert(prey);
    let pp = PredationParams::default();
    let sp = SocialParams::default();
    let mut idx = SpatialIndex::new(&w);
    idx.rebuild(&store, &w);
    // No predator on the map at all: only the retained vector can keep it.
    mark_threats(&mut store, &idx, &w, 1, &pp, &sp);
    assert!(store.get(id).unwrap().wary_by.is_some(), "retained within wary_distance x release");

    store.get_mut(id).unwrap().x = 45; // dist 12.5 > 6.0 x 1.5
    idx.rebuild(&store, &w);
    mark_threats(&mut store, &idx, &w, 1, &pp, &sp);
    assert!(store.get(id).unwrap().wary_by.is_none(), "cleared past the release radius");
}

/// The wary tier is deliberately cheaper than flee: half the steps, the wander
/// energy rate, not the doubled flee rate.
#[test]
fn wary_costs_less_exertion_than_flee() {
    let w = all_grass_world();
    let cp = CreaturesParams { move_speed_base: 2.0, move_speed_per_trait: 0.0, ..CreaturesParams::default() };
    let pp = PredationParams::default();
    let run = |goal: Goal| {
        let mut c = test_creature(0, 5);
        c.target = Some((30, 5));
        c.goal = goal;
        let e0 = c.energy;
        move_toward(&mut c, &w, &day_time(12), &cp, &pp, 1.0);
        (c.x, e0 - c.energy)
    };
    let (wx, wcost) = run(Goal::Wander);
    let (yx, ycost) = run(Goal::Wary);
    let (fx, fcost) = run(Goal::Flee);
    assert!(yx < fx, "wary takes fewer steps than flee: {yx} vs {fx}");
    assert!(ycost < fcost, "wary costs less energy than flee: {ycost} vs {fcost}");
    assert!(ycost <= wcost + 1e-6, "a wary step costs the wander rate: {ycost} vs {wcost}");
    assert_eq!(wx, fx, "flee keeps the wander step count");
}

/// One aggregate event per region per day, the dominant pair naming the line.
#[test]
fn day_flush_is_one_event_per_region() {
    let w = all_grass_world();
    let time = day_time(0);
    let mut events = EventRing::new(16);
    let mut tallies = DeathTallies::default();
    tallies.wary_today.insert((0, SpeciesId::Vole, SpeciesId::Wolf), 5);
    tallies.wary_today.insert((0, SpeciesId::Hare, SpeciesId::Wolf), 3);
    tallies.wary_today.insert((1, SpeciesId::Deer, SpeciesId::Lynx), 2);
    flush_wary(&w, &mut events, &time, &mut tallies);
    assert!(tallies.wary_today.is_empty(), "the day's tally is drained");
    let kinds: Vec<EventKind> = events.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![EventKind::Wary, EventKind::Wary], "one event per region");

    let first = events.iter().next().unwrap();
    assert_eq!(first.species, Some(SpeciesId::Vole), "the dominant prey names the event");
    assert!(first.text.contains("Voles give wolves room"), "{}", first.text);
    assert!(first.text.contains("(8 wary encounters)"), "{}", first.text);
    assert_eq!(first.detail, "0:0:4:8", "region:prey:predator:total");

    let second = events.iter().nth(1).unwrap();
    assert_eq!(second.detail, "1:2:5:2", "regions emit in ascending order");
}

/// The daily tally does not survive into the next day.
#[test]
fn wary_tally_resets_at_the_next_day() {
    let mut tallies = DeathTallies::default();
    tallies.wary_today.insert((0, SpeciesId::Vole, SpeciesId::Wolf), 1);
    let tallies = tallies.next_day();
    assert!(tallies.wary_today.is_empty(), "a fresh day has no wary encounters");
}
