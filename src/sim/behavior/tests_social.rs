//! Tests for alarms, cohesion and packs.

use crate::sim::creatures::{
    CreatureStore, DeathTallies, Goal, HuntPhase, 
};
use crate::sim::events::{EventRing};
use crate::sim::genetics::{TickView };
use crate::sim::geom;
use crate::sim::lineage::Lineage;
use crate::sim::params::{GeneticsParams, PredationParams, SocialParams};
use crate::sim::rng::Rng;
use crate::sim::spatial::SpatialIndex;
use crate::sim::species::{SpeciesId};
use crate::sim::disease::{DiseaseState};
use crate::sim::params::DiseaseParams;
use super::perception::kin_summary;
use super::movement::wander;
use super::threat::mark_threats;
use super::hunt::{hunt_contacts, pick_hunt_target};
use super::tests::{all_grass_world, day_time, pack_fixture, test_creature, wolf_and_sighted_prey};

use crate::sim::species::{IDX_SOCIALITY};

#[test]
fn alarm_spreads_to_social_kin_only() {
    let sp = SocialParams::default();
    let (w, mut store) = wolf_and_sighted_prey();
    // A second deer out of sight of the wolf (weak sense) but near the first.
    let mut blind = test_creature(26, 5);
    blind.species = SpeciesId::Deer;
    blind.genome = SpeciesId::Deer.base_genome();
    blind.genome.0[2] = 0.02; // sense range 2 cells: cannot see the wolf at 6
    blind.genome.0[IDX_SOCIALITY] = 0.70; // social enough to heed an alarm
    let blind_id = store.insert(blind);
    let mut idx = SpatialIndex::new(&w);
    idx.rebuild(&store, &w);
    mark_threats(&mut store, &idx, &w, 0, &PredationParams::default(), &sp);
    let sighted = store.living().find(|c| c.x == 22).unwrap();
    assert!(sighted.threatened_by.is_some(), "the prey that sees the wolf is threatened");
    let blind = store.get(blind_id).unwrap();
    assert_eq!(
        blind.threatened_by.map(|t| (t.0, t.1, t.2)),
        Some((20, 5, SpeciesId::Wolf)),
        "the alarm reaches the blind kin"
    );

    // An asocial neighbour (0.02) is out of earshot: 0.02 x 6 cells < 4.
    let (w, mut store) = wolf_and_sighted_prey();
    let mut asocial = test_creature(26, 5);
    asocial.species = SpeciesId::Deer;
    asocial.genome = SpeciesId::Deer.base_genome();
    asocial.genome.0[2] = 0.02;
    asocial.genome.0[IDX_SOCIALITY] = 0.02;
    let asocial_id = store.insert(asocial);
    let mut idx = SpatialIndex::new(&w);
    idx.rebuild(&store, &w);
    mark_threats(&mut store, &idx, &w, 0, &PredationParams::default(), &sp);
    assert!(store.get(asocial_id).unwrap().threatened_by.is_none(), "an asocial kin is not alerted");
}

#[test]
fn cohesion_pulls_the_social_and_leaves_the_asocial_alone() {
    let w = all_grass_world();
    let gp = GeneticsParams::default();
    let sp = SocialParams::default();
    let t = day_time(12);
    // Two deer clustered far to the west; the focal animal stands east.
    let mut store = CreatureStore::new();
    let mut kin_ids = Vec::new();
    for (x, y) in [(12usize, 5usize), (8, 6)] {
        let mut k = test_creature(x, y);
        k.species = SpeciesId::Deer;
        k.genome = SpeciesId::Deer.base_genome();
        kin_ids.push(store.insert(k));
    }
    let mut focal = test_creature(50, 5);
    focal.species = SpeciesId::Deer;
    focal.genome = SpeciesId::Deer.base_genome();
    focal.genome.0[IDX_SOCIALITY] = 0.98;
    let view = TickView::build(&store, &t, &w, &gp, &DiseaseParams::default());
    let kin = kin_summary(&focal, &kin_ids, &view);
    assert_eq!(kin.count, 2, "both kin are visible");
    let (kx, ky) = kin.centroid();
    let start_d = geom::dist(focal.x, focal.y, kx, ky);
    wander(&mut focal, &w, &t, &view, &gp, &sp, &mut Rng::new(4), Some(kin));
    let target = focal.target.expect("a social wanderer herds");
    assert!(geom::dist(target.0, target.1, kx, ky) < start_d, "target {target:?} should close on the herd");

    // The same animal with no sociality herds not: kin makes no difference.
    let mut asocial = test_creature(50, 5);
    asocial.species = SpeciesId::Deer;
    asocial.genome = SpeciesId::Deer.base_genome();
    asocial.genome.0[IDX_SOCIALITY] = 0.02;
    let mut a = asocial.clone();
    wander(&mut a, &w, &t, &view, &gp, &sp, &mut Rng::new(9), Some(kin));
    let mut b = asocial;
    wander(&mut b, &w, &t, &view, &gp, &sp, &mut Rng::new(9), None);
    assert_eq!(a.target, b.target, "an asocial wanderer ignores the herd");
    assert!(!sp.herding(0.02, 2) && sp.herding(0.98, 2), "the herding rule is the gate");
}

#[test]
fn pack_joins_the_shared_target() {
    let (w, store, far_id, near_id, mate_id, focal_id) = pack_fixture();
    let pp = PredationParams { kill_max: 1.0, ..PredationParams::default() };
    let sp = SocialParams::default();
    let t = day_time(12);
    let mut idx = SpatialIndex::new(&w);
    idx.rebuild(&store, &w);
    let view = TickView::build(&store, &t, &w, &GeneticsParams::default(), &DiseaseParams::default());
    // The perception id list holds prey *and* packmates; the packmate is what
    // marks the shared target.
    let candidates = vec![far_id, near_id, mate_id];
    let (picked, _) = pick_hunt_target(store.get(focal_id).unwrap(), &candidates, &view, &w, &pp, &sp).unwrap();
    assert_eq!(picked, far_id, "the pack's target beats the nearer open prey");
    // Without the pack bonus the nearer, visible deer would win.
    let mut alone = store.get(focal_id).unwrap().clone();
    alone.genome.0[IDX_SOCIALITY] = 0.0;
    let (solo, _) = pick_hunt_target(&alone, &candidates, &view, &w, &pp, &sp).unwrap();
    assert_eq!(solo, near_id, "no pack bonus, no join: the visible prey wins");
}

#[test]
fn pack_shares_the_kill() {
    let (mut w, mut store, far_id, _near_id, mate_id, focal_id) = pack_fixture();
    let pp = PredationParams { kill_max: 1.0, ..PredationParams::default() };
    let sp = SocialParams::default();
    let t = day_time(12);
    // Let the focal wolf kill the far deer, with the packmate in support range.
    {
        let f = store.get_mut(focal_id).unwrap();
        f.goal = Goal::Hunt;
        f.hunt_phase = HuntPhase::Chase;
        f.hunt_target = Some(far_id);
        f.x = 32;
        f.y = 5;
        f.hunger = 0.9;
    }
    {
        let m = store.get_mut(mate_id).unwrap();
        // Close enough to share the kill, too far to make the roll itself.
        m.x = 37;
        m.y = 5;
        m.hunger = 0.9;
    }
    let mate_hunger = store.get(mate_id).unwrap().hunger;
    let prey_size = store.get(far_id).unwrap().genome.size();
    let mut events = EventRing::new(64);
    let mut tallies = DeathTallies::default();
    let mut lineage = Lineage::new();
    let mut dstate = DiseaseState::new(&DiseaseParams::default());
    hunt_contacts(
        &mut store,
        &mut w,
        &mut events,
        &t,
        &pp,
        &DiseaseParams::default(),
        &sp,
        &mut Rng::new(1),
        &mut tallies,
        &mut lineage,
        &mut dstate,
        &mut Rng::new(2),
    );
    assert!(!store.get(far_id).unwrap().alive, "the pack brings the prey down");
    assert_eq!(store.get(focal_id).unwrap().kills, 1, "the kill is counted once");
    assert_eq!(store.get(mate_id).unwrap().kills, 0, "a participant does not score a kill");
    let want = mate_hunger - pp.hunger_per_kill(prey_size) * sp.pack_share;
    assert!((store.get(mate_id).unwrap().hunger - want).abs() < 1e-5, "the packmate shares the meal");
    let mate = store.get(mate_id).unwrap();
    assert_eq!(mate.hunt_target, None);
    assert_eq!(mate.goal, Goal::Patrol);
    assert_eq!(mate.attempts, 0, "sharing is not a failed attempt");
    assert_eq!(tallies.hunt_kills[SpeciesId::Wolf.index()], 1);
}
