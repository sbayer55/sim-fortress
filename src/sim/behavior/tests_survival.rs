//! S16 survival tallies: droughts eased and winters survived.

use super::survival::{droughts_eased, winter_survived};
use super::tests::{test_creature, test_world};
use crate::sim::creatures::CreatureStore;
use crate::sim::world::World;

/// One cell in region `ri`, scanning row-major.
fn cell_in_region(world: &World, ri: usize) -> Option<(usize, usize)> {
    (0..world.height).flat_map(|y| (0..world.width).map(move |x| (x, y))).find(|&(x, y)| world.region_index(x, y) == ri)
}

#[test]
fn a_drought_easing_bumps_only_the_living_standing_in_that_region() {
    let world = test_world();
    let (ax, ay) = cell_in_region(&world, 0).unwrap_or((0, 0));
    let (bx, by) = cell_in_region(&world, 1).unwrap_or((0, 0));
    assert_ne!(world.region_index(ax, ay), world.region_index(bx, by), "the test world needs two regions");
    let mut store = CreatureStore::new();
    let in_a = store.insert(test_creature(ax, ay));
    let in_b = store.insert(test_creature(bx, by));
    let mut dead = test_creature(ax, ay);
    dead.alive = false;
    let corpse = store.insert(dead);
    let mut before = [false; 8];
    before[0] = true;
    // Still in drought: nothing changes.
    droughts_eased(&mut store, &world, before, before);
    assert_eq!(store.get(in_a).map(|c| c.droughts_survived), Some(0));
    // Region 0 eases.
    droughts_eased(&mut store, &world, before, [false; 8]);
    assert_eq!(store.get(in_a).map(|c| c.droughts_survived), Some(1));
    assert_eq!(store.get(in_b).map(|c| c.droughts_survived), Some(0));
    assert_eq!(store.get(corpse).map(|c| c.droughts_survived), Some(0), "carcasses survive nothing");
    // A drought starting is not an easing.
    droughts_eased(&mut store, &world, [false; 8], before);
    assert_eq!(store.get(in_a).map(|c| c.droughts_survived), Some(1));
}

#[test]
fn spring_bumps_every_living_animal_and_the_events_sum() {
    let mut store = CreatureStore::new();
    let a = store.insert(test_creature(1, 1));
    let mut dead = test_creature(2, 2);
    dead.alive = false;
    let corpse = store.insert(dead);
    winter_survived(&mut store);
    winter_survived(&mut store);
    assert_eq!(store.get(a).map(|c| c.winters_survived), Some(2));
    assert_eq!(store.get(corpse).map(|c| c.winters_survived), Some(0));
    let mut c = test_creature(0, 0);
    c.infections_survived = 1;
    c.escaped = 2;
    c.contests_won = 3;
    c.droughts_survived = 4;
    c.winters_survived = 5;
    assert_eq!(c.survival_events(), 15);
}
