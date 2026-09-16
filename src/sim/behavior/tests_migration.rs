//! Tests for migration.

use crate::sim::creatures::{
    CreatureStore, Goal, 
};
use crate::sim::events::{EventKind, EventRing};
use crate::sim::species::testing::*;
use crate::sim::params::{PredationParams };
use super::migration::{mean_pred_pressure, migrate_group, regions_adjacent};
use super::tests::{day_time, hungry_pack, land_cell_in, run_migration_days, test_creature, test_world};


#[test]
fn migration_destination() {
    let w = test_world();
    let mut store = CreatureStore::new();
    let (x, y) = land_cell_in(&w, 0);
    for _ in 0..5 {
        let mut h = test_creature(x, y);
        h.species = HARE;
        store.insert(h);
    }
    let mut events = EventRing::new(16);
    let time = day_time(0);
    migrate_group(&mut store, &w, &mut events, &time, roster(), HARE, 0);
    let ev = events.iter().find(|e| e.kind == EventKind::Migration).expect("one Migration event");
    assert!(ev.text.to_lowercase().contains("herd of 5 hares"), "{}", ev.text);
    let dest: usize = ev.detail.split('>').nth(1).unwrap().parse().unwrap();
    assert!(regions_adjacent(&w.regions[0], &w.regions[dest]), "destination shares an edge with the origin");
    // The destination maximises mean_vegetation × (1 − mean_pred_pressure).
    let score = |ri: usize| crate::sim::ecology::region_land_veg_mean(&w, &w.regions[ri]) * (1.0 - mean_pred_pressure(&w, &w.regions[ri]));
    for (ri, r) in w.regions.iter().enumerate() {
        if ri != 0 && regions_adjacent(&w.regions[0], r) {
            assert!(score(dest) >= score(ri), "dest {dest} beats {ri}");
        }
    }
    for h in store.living() {
        assert_eq!(h.goal, Goal::Migrate);
        let (tx, ty) = h.migrate_target.expect("target cell");
        assert_eq!(w.region_index(tx, ty), dest);
        assert!(w.cell(tx, ty).terrain.walkable());
        assert_eq!(h.migrate_until, time.tick + 2 * u64::from(time.ticks_per_day), "for up to 2 days");
    }
    assert_eq!(ev.pos, Some(((w.regions[0].1 + w.regions[0].3).div_euclid(2), (w.regions[0].2 + w.regions[0].4).div_euclid(2))), "pos = origin region centre");
}

#[test]
fn predator_migration_on_low_prey() {
    let (w, mut store) = hungry_pack();
    let mut events = EventRing::new(16);
    let mut time = day_time(0);
    let (mut cd, mut db) = (vec![0u64; N_SPECIES * 8], vec![0u32; N_SPECIES * 8]);
    let pp = PredationParams::default();
    run_migration_days(&w, &mut store, &mut events, &mut time, &mut cd, &mut db, pp.migrate_days - 1);
    assert!(!events.iter().any(|e| e.kind == EventKind::Migration), "needs migrate_days consecutive days");
    run_migration_days(&w, &mut store, &mut events, &mut time, &mut cd, &mut db, 1);
    let ev: Vec<_> = events.iter().filter(|e| e.kind == EventKind::Migration).collect();
    assert_eq!(ev.len(), 1);
    assert!(ev[0].text.to_lowercase().contains("pack of 4 wolves"), "{}", ev[0].text);
    assert!(store.living().all(|c| c.goal == Goal::Migrate ));
}

#[test]
fn migration_cooldown() {
    let (w, mut store) = hungry_pack();
    let mut events = EventRing::new(16);
    let mut time = day_time(0);
    let (mut cd, mut db) = (vec![0u64; N_SPECIES * 8], vec![0u32; N_SPECIES * 8]);
    let pp = PredationParams::default();
    run_migration_days(&w, &mut store, &mut events, &mut time, &mut cd, &mut db, pp.migrate_days);
    // Keep the pack in region 0 so the trigger keeps holding.
    let (x, y) = land_cell_in(&w, 0);
    let count = |events: &EventRing| events.iter().filter(|e| e.kind == EventKind::Migration).count();
    assert_eq!(count(&events), 1);
    for _ in 0..(pp.migrate_cooldown_days - 1) {
        for c in store.living_mut() {
            c.x = x;
            c.y = y;
        }
        run_migration_days(&w, &mut store, &mut events, &mut time, &mut cd, &mut db, 1);
    }
    assert_eq!(count(&events), 1, "no second migration within migrate_cooldown_days");
    for _ in 0..=pp.migrate_days {
        for c in store.living_mut() {
            c.x = x;
            c.y = y;
        }
        run_migration_days(&w, &mut store, &mut events, &mut time, &mut cd, &mut db, 1);
    }
    assert_eq!(count(&events), 2, "migrates again once the cooldown has passed");
}

// ---------------------------------------------------------------- C8 sociality
