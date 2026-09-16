//! Tests for death attribution and carcasses.

use crate::sim::creatures::{
    Cause, CreatureStore, Death, DeathTallies, 
};
use crate::sim::events::{EventRing};
use crate::sim::species::testing::*;
use crate::sim::lineage::Lineage;
use crate::sim::params::{CreaturesParams, GeneticsParams  };
use crate::sim::rng::Rng;
use crate::sim::disease::{DiseaseState};
use crate::sim::params::DiseaseParams;
use super::day_boundary;
use super::death::maybe_die;
use super::tests::{day_time, test_creature, test_world};


#[test]
fn hp_death_attribution() {
    let mut w = test_world();
    let mut events = EventRing::new(100);
    let mut tallies = DeathTallies::new(N_SPECIES);
    let t = day_time(12);

    let mut starved = test_creature(75, 20);
    starved.hp = 0.0;
    starved.hunger = 1.5;
    starved.thirst = 0.2;
    maybe_die(&mut starved, &mut w, &mut events, &t, roster(), &mut tallies, &mut Lineage::new(), &DiseaseParams::default());
    assert!(!starved.alive);
    assert_eq!(starved.death.unwrap().cause, Cause::Starved);
    assert_eq!(tallies.starved, 1);

    let mut thirsty = test_creature(76, 20);
    thirsty.hp = 0.0;
    thirsty.hunger = 0.2;
    thirsty.thirst = 1.5;
    maybe_die(&mut thirsty, &mut w, &mut events, &t, roster(), &mut tallies, &mut Lineage::new(), &DiseaseParams::default());
    assert_eq!(thirsty.death.unwrap().cause, Cause::Thirst);
    assert_eq!(tallies.thirst, 1);
}

#[test]
fn age_death_at_day_boundary() {
    let mut w = test_world();
    let mut store = CreatureStore::new();
    let mut c = test_creature(75, 20);
    c.born_day = -2000; // age 2000 ≥ any max_age
    let id = store.insert(c);
    let mut events = EventRing::new(100);
    let mut tallies = DeathTallies::new(N_SPECIES);
    let t = day_time(0);
    day_boundary(&mut store, &mut w, &mut events, &t, roster(), &CreaturesParams::default(), &GeneticsParams::default(), &DiseaseParams::default(), &mut tallies, &mut Lineage::new(), &mut DiseaseState::new(&DiseaseParams::default(), roster()), &mut Rng::new(1));
    let c = store.get(id).unwrap();
    assert!(!c.alive);
    assert_eq!(c.death.unwrap().cause, Cause::Age);
    assert_eq!(tallies.age, 1);
    assert!(w.carcasses.contains(&(75, 20)));
}

#[test]
fn carcass_decay_frees_slot() {
    let mut w = test_world();
    let mut store = CreatureStore::new();
    let mut c = test_creature(75, 20);
    c.alive = false;
    c.decay = 0.99;
    c.death = Some(Death { cause: Cause::Starved, day: 0, killer: None, chase_ticks: 0 });
    w.carcasses.push((75, 20));
    let id = store.insert(c);
    let mut events = EventRing::new(100);
    let mut tallies = DeathTallies::new(N_SPECIES);
    day_boundary(&mut store, &mut w, &mut events, &day_time(0), roster(), &CreaturesParams::default(), &GeneticsParams::default(), &DiseaseParams::default(), &mut tallies, &mut Lineage::new(), &mut DiseaseState::new(&DiseaseParams::default(), roster()), &mut Rng::new(1));
    assert!(store.get(id).is_none(), "decayed carcass slot should be freed");
    assert!(!w.carcasses.contains(&(75, 20)));
}
