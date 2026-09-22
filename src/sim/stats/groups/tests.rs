//! Unit tests for the group-size census.

use super::*;
use crate::sim::species::testing::*;
use crate::sim::creatures::{place_founders, Creature, CreatureStore};
use crate::sim::params::{CreaturesParams, GeneticsParams, SocialParams, WorldParams};
use crate::sim::rng::Rng;
use crate::sim::species::IDX_SOCIALITY;
use crate::sim::world::World;
use crate::sim::{Params, Sim};

/// A living founder of `species` at `(x, y)` with the given Sociality and Sense.
fn creature_at(species: crate::sim::SpeciesId, x: usize, y: usize, sociality: f32, sense: f32) -> Creature {
    let w = World::generate(7, &WorldParams::default());
    let mut c = place_founders(&w, roster(), &CreaturesParams::default(), &GeneticsParams::default(), 0.20, &mut Rng::new(1)).remove(0);
    c.species = species;
    c.x = x;
    c.y = y;
    c.alive = true;
    c.genome.0[IDX_SOCIALITY] = sociality;
    c.genome.0[2] = sense; // Sense; `sense_cells() = 2 + sense x 10`
    c
}

fn store_of(creatures: &[Creature]) -> CreatureStore {
    let mut store = CreatureStore::new();
    for c in creatures {
        store.insert(c.clone());
    }
    store
}

/// Population implied by a species' histogram: a group of `k + 1` members counts
/// `k + 1` creatures.
fn implied_population(g: &GroupCensus, i: usize) -> u32 {
    g.hist[i].iter().enumerate().map(|(k, &n)| crate::cast!(k + 1 => u32) * n).sum()
}

#[test]
fn nearest_neighbour_mean_is_over_adults_only() {
    // Two adult foxes four columns apart (ellipse distance 2) and a pup in
    // between: the pup neither counts nor is counted.
    let mut a = creature_at(FOX, 10, 5, 0.1, 0.5);
    a.adult = true;
    let mut b = creature_at(FOX, 14, 5, 0.1, 0.5);
    b.adult = true;
    let mut pup = creature_at(FOX, 12, 5, 0.1, 0.5);
    pup.adult = false;
    let nn = nearest_neighbour_mean(&store_of(&[a, b, pup]), N_SPECIES);
    assert_eq!(nn[FOX.index()], 2.0);
    assert_eq!(nn[WOLF.index()], 0.0, "no adults: 0");
    let mut lone = creature_at(LYNX, 3, 3, 0.1, 0.5);
    lone.adult = true;
    assert_eq!(nearest_neighbour_mean(&store_of(&[lone]), N_SPECIES)[LYNX.index()], 0.0, "one adult: 0");
    let g = group_census(&store_of(&[creature_at(FOX, 10, 5, 0.1, 0.5)]), &SocialParams::default(), N_SPECIES);
    assert_eq!(g.nn_mean.len(), N_SPECIES);
}

#[test]
fn social_neighbours_form_one_group() {
    // Three deer well inside each other's sense range (sense 0.5 → 7 cells).
    let deer = DEER;
    let store = store_of(&[
        creature_at(deer, 20, 20, 0.70, 0.5),
        creature_at(deer, 22, 20, 0.70, 0.5),
        creature_at(deer, 20, 22, 0.70, 0.5),
    ]);
    let g = group_census(&store, &SocialParams::default(), N_SPECIES);
    let i = deer.index();
    assert_eq!(g.groups[i], 1);
    assert_eq!(g.members[i], 3);
    assert_eq!(g.max[i], 3);
    assert_eq!(g.mean[i], 3.0);
    assert_eq!(g.solo(i), 0);
    assert_eq!(g.grouped_share(i, 3), 1.0);
}

#[test]
fn asocial_creatures_are_always_alone() {
    // Below `cohesion_min`, two hares on the same cell are still two loners.
    let hare = HARE;
    let store = store_of(&[creature_at(hare, 20, 20, 0.25, 0.5), creature_at(hare, 20, 20, 0.25, 0.5)]);
    let g = group_census(&store, &SocialParams::default(), N_SPECIES);
    let i = hare.index();
    assert_eq!(g.groups[i], 0);
    assert_eq!(g.members[i], 0);
    assert_eq!(g.max[i], 0);
    assert_eq!(g.mean[i], 0.0);
    assert_eq!(g.solo(i), 2);
    assert_eq!(g.grouped_share(i, 2), 0.0);
}

#[test]
fn groups_break_beyond_sense() {
    // Two social deer 30 cells apart are two groups of one.
    let deer = DEER;
    let store = store_of(&[creature_at(deer, 10, 10, 0.70, 0.5), creature_at(deer, 40, 10, 0.70, 0.5)]);
    let g = group_census(&store, &SocialParams::default(), N_SPECIES);
    assert_eq!(g.groups[deer.index()], 0);
    assert_eq!(g.solo(deer.index()), 2);
}

#[test]
fn groups_respect_the_dispersal_cap() {
    // Sociality 0.35 with the default `group_size_max = 12` allows
    // `kin = 1.5 x 4.2 = 6.3` visible neighbours, so a crowd piles into groups
    // of seven and five (a joiner's neighbours are the existing members).
    let vole = VOLE;
    let crowd: Vec<Creature> = (0..12).map(|k| creature_at(vole, 20 + (k % 3), 20 + k.div_euclid(3), 0.35, 0.5)).collect();
    let g = group_census(&store_of(&crowd), &SocialParams::default(), N_SPECIES);
    let i = vole.index();
    assert_eq!(g.members[i], 12, "every living member is in a group");
    assert_eq!(g.groups[i], 2, "the crowd must split at the cap: {:?}", g.groups);
    assert_eq!(g.max[i], 7, "the cap is 1.5 x preferred neighbours, plus the joiner");
    let kin_cap = 1.5 * SocialParams::default().preferred_group(0.35);
    assert!(f32::from(g.max[i]) - 1.0 <= kin_cap, "no group may pass the herding band: {kin_cap}");
}

#[test]
fn histogram_counts_every_creature_exactly_once() {
    let store = store_of(&place_founders(
        &World::generate(7, &WorldParams::default()),
        roster(),
        &CreaturesParams::default(),
        &GeneticsParams::default(),
        0.20,
        &mut Rng::new(5),
    ));
    let g = group_census(&store, &SocialParams::default(), N_SPECIES);
    let c = crate::sim::stats::census(&store, N_SPECIES);
    for i in 0..6 {
        // The histogram counts groups; weighting each bucket by its size
        // recovers the population, so every member is in exactly one group.
        assert_eq!(implied_population(&g, i), c.population[i], "species {i}: every member is in exactly one group");
        assert_eq!(g.solo(i) + g.members[i], c.population[i], "solo + grouped = living");
    }
}

#[test]
fn sim_refreshes_group_stats_and_survives_a_save() {
    let mut sim = Sim::new(7, Params::default());
    // A fresh world already has a census (the founders).
    let living_population = |sim: &Sim| -> u32 { (0..6).map(|i| implied_population(&sim.group_stats, i)).sum() };
    assert_eq!(crate::cast!(living_population(&sim) => usize), sim.creatures.len_living());

    // The day boundary recomputes it from the living set. Step to the exact
    // midnight boundary: the behaviour ticks after it legitimately change the
    // population, so a later comparison would test something else.
    while sim.time.hour() != 0 {
        sim.step();
    }
    assert_eq!(crate::cast!(living_population(&sim) => usize), sim.creatures.len_living(), "the day boundary refreshes the census");
    // It is a midnight snapshot, so the explicit refresh is what matches the
    // positions after a few more behaviour ticks.
    sim.refresh_group_stats();
    assert_eq!(sim.group_stats, group_census(&sim.creatures, &sim.params.social, N_SPECIES));

    // Group stats are never serialised, so a load rebuilds them from positions.
    let dir = std::env::temp_dir().join(format!("simf-groups-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = crate::sim::save::save(&sim, "Groups", &dir).unwrap();
    let loaded = crate::sim::save::load(&path).unwrap().sim;
    assert_eq!(loaded.group_stats, group_census(&loaded.creatures, &loaded.params.social, N_SPECIES));
    assert_eq!(loaded.group_stats, sim.group_stats, "a load rebuilds the same census from the same positions");
    let _ = std::fs::remove_dir_all(&dir);
}
