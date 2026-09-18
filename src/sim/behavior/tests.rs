//! Shared fixtures for the behaviour tests.

use crate::sim::creatures::{
    Creature, CreatureId, CreatureStore, DeathTallies, Goal, HuntPhase, 
};
use crate::sim::events::{EventRing};
use crate::sim::species::testing::*;
use crate::sim::genetics::{TickView };
use crate::sim::lineage::Lineage;
use crate::sim::params::{CreaturesParams, DietParams, EcologyParams, GeneticsParams, PredationParams, SocialParams};
use crate::sim::rng::Rng;
use crate::sim::spatial::SpatialIndex;
use crate::sim::disease::{self, DiseaseState};
use crate::sim::params::DiseaseParams;
use crate::sim::time::Time;
use crate::sim::world::{Terrain, World};
use super::tick_creatures;
use super::goals::replan;
use super::migration::migration_daily;

use crate::sim::creatures::{Sex};
use crate::sim::params::{WorldParams};

pub(super) fn test_world() -> World {
    World::generate(7, &WorldParams::default())
}

pub(super) fn test_creature(x: usize, y: usize) -> Creature {
    Creature {
        id: CreatureId(0),
        species: VOLE,
        name: 0,
        sex: Sex::Female,
        x,
        y,
        born_day: -100,
        generation: 1,
        parents: None,
        genome: genome(VOLE),
        hp: 1.0,
        hunger: 0.3,
        thirst: 0.3,
        energy: 0.8,
        adult: true,
        sterile: false,
        goal: Goal::Wander,
        target: None,
        replan_at: 0,
        trail: Vec::new(),
        alive: true,
        death: None,
        decay: 0.0,
        mutations: Vec::new(),
        last_water: None,
        last_ate: None,
        last_drank: None,
        last_slept: None,
        move_budget: 0.0,
        path: Vec::new(),
        rest_reason: None,
        pregnant_due: None,
        cooldown_until: 0,
        mate_id: None,
        mother: None,
        offspring: 0,
        kills: 0,
        attempts: 0,
        chased: 0,
        escaped: 0,
        threats_by_species: vec![0; N_SPECIES],
        kills_by_species: vec![0; N_SPECIES],
        last_kill: None,
        chase_stats: (0, 0),
        chase_longest_year: 0,
        hunt_phase: HuntPhase::Stalk,
        hunt_target: None,
        chase_start_tick: None,
        hunt_cooldown_until: 0,
        eat_until: None,
        scavenge_target: None,
        flee_until: 0,
        threatened_by: None,
        predation_risk: 0.0,
        wary_until: 0,
        wary_by: None,
        wary_count: 0,
        kin_nearby: 0,
        migrate_until: 0,
            // ---- C7 disease / parasites
            infection: None,
            immune_until: [0; 8],
            parasite_load: 0.0,
            infections_survived: 0,
            died_infected: None,
        migrate_target: None,
        path_for: None,
    }
}

pub(super) fn empty_index(world: &World) -> SpatialIndex {
    SpatialIndex::new(world)
}

pub(super) fn plan(c: &mut Creature, idx: &SpatialIndex, w: &World, t: &Time, cp: &CreaturesParams, rng: &mut Rng) {
    replan(c, idx, w, t, roster(), cp, &GeneticsParams::default(), &PredationParams::default(), &TickView::empty(), rng, &DiseaseParams::default(), disease::REST_ENERGY, &SocialParams::default(), &DietParams::default());
}

pub(super) fn day_time(hour: u32) -> Time {
    // start_hour 6: hour = (tick + 6) % 24.
    let tick = (hour + 24 - 6) % 24;
    let mut t = Time::new(6, 90, 24, 6, 20);
    t.tick = u64::from(tick);
    t
}

pub(super) fn all_grass_world() -> World {
    let mut w = World::generate(7, &WorldParams { width: 60, height: 10, ..WorldParams::default() });
    for c in &mut w.cells {
        c.terrain = Terrain::Grass;
        c.vegetation = 0.5;
    }
    w.refresh_shore();
    w
}

/// Run one full creature tick over `store` in `w` at noon.
pub(super) fn tick_once(store: &mut CreatureStore, w: &mut World, time: &Time, pp: &PredationParams) -> EventRing {
    let mut idx = SpatialIndex::new(w);
    idx.rebuild(store, w);
    let mut events = EventRing::new(64);
    let mut tallies = DeathTallies::new(N_SPECIES);
    let mut noted = false;
    tick_creatures(
        store,
        &idx,
        w,
        &mut events,
        time,
        roster(),
        &CreaturesParams::default(),
        &EcologyParams::default(),
        &GeneticsParams::default(),
        pp,
        &DiseaseParams::default(),
        &SocialParams::default(),
        &DietParams::default(),
        &mut Rng::new(1),
        &mut tallies,
        &mut Lineage::new(),
        &mut noted,
        &mut DiseaseState::new(&DiseaseParams::default(), roster()),
        &mut Rng::new(2),
    );
    events
}

/// A walkable land cell inside region `ri` of `w`.
pub(super) fn land_cell_in(w: &World, ri: usize) -> (usize, usize) {
    for (x, y) in w.region_cells(ri) {
        let t = w.cell(x, y).terrain;
        if t.walkable() && !t.is_water() {
            return (x, y);
        }
    }
    panic!("region {ri} has no land");
}

/// Four wolves in region 0 and no prey anywhere: the predator trigger holds every day.
pub(super) fn hungry_pack() -> (World, CreatureStore) {
    let w = test_world();
    let mut store = CreatureStore::new();
    let (x, y) = land_cell_in(&w, 0);
    for _ in 0..4 {
        let mut c = test_creature(x, y);
        c.species = WOLF;
        c.genome = genome(WOLF);
        store.insert(c);
    }
    (w, store)
}

pub(super) fn run_migration_days(w: &World, store: &mut CreatureStore, events: &mut EventRing, time: &mut Time, cd: &mut [u64], db: &mut [u32], days: u32) {
    let pp = PredationParams::default();
    for _ in 0..days {
        time.tick += u64::from(time.ticks_per_day);
        migration_daily(store, w, events, time, roster(), &EcologyParams::default(), &pp, cd, db);
    }
}

/// A wolf at `(20, 5)` hungry enough to be a danger, plus one prey at
/// `(22, 5)` whose base senses can see it.
pub(super) fn wolf_and_sighted_prey() -> (World, CreatureStore) {
    let w = all_grass_world();
    let mut store = CreatureStore::new();
    let mut wolf = test_creature(20, 5);
    wolf.species = WOLF;
    wolf.genome = genome(WOLF);
    wolf.hunger = 0.9;
    store.insert(wolf);
    // A deer: base sense 0.55 detects the wolf's camo 0.25 at two cells.
    let mut deer = test_creature(22, 5);
    deer.species = DEER;
    deer.genome = genome(DEER);
    store.insert(deer);
    (w, store)
}

/// Two deer (the far one camouflaged) and two wolves (one already hunting
/// the far deer, one focal) in an all-grass world.
pub(super) fn pack_fixture() -> (World, CreatureStore, CreatureId, CreatureId, CreatureId, CreatureId) {
    let w = all_grass_world();
    let mut store = CreatureStore::new();
    let mut far = test_creature(33, 5);
    far.species = DEER;
    far.genome = genome(DEER);
    far.genome.0[5] = 0.98; // camouflage: undetectable
    let far_id = store.insert(far);
    let mut near = test_creature(29, 5);
    near.species = DEER;
    near.genome = genome(DEER);
    near.genome.0[5] = 0.02;
    let near_id = store.insert(near);
    // The packmate is already hunting the far deer; the focal wolf is not.
    let mut mate = test_creature(35, 5);
    mate.species = WOLF;
    mate.genome = genome(WOLF);
    mate.goal = Goal::Hunt;
    mate.hunt_phase = HuntPhase::Chase;
    mate.hunt_target = Some(far_id);
    let mate_id = store.insert(mate);
    let mut focal = test_creature(30, 5);
    focal.species = WOLF;
    focal.genome = genome(WOLF);
    let focal_id = store.insert(focal);
    (w, store, far_id, near_id, mate_id, focal_id)
}
