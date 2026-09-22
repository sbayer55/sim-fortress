//! C5 FR13 territory: the scent grid, scent avoidance, the challenge, the
//! contest and the neutral-overlay tripwire.

use super::tests::*;
use super::territory::{self, Scent};
use super::perception::perceive;
use crate::sim::creatures::{CreatureId, CreatureStore, DeathTallies, Goal};
use crate::sim::events::{EventKind, EventRing};
use crate::sim::genetics::TickView;
use crate::sim::params::{CreaturesParams, DietParams, GeneticsParams, PredationParams, SocialParams, TerritoryParams};
use crate::sim::params::DiseaseParams;
use crate::sim::rng::Rng;
use crate::sim::spatial::SpatialIndex;
use crate::sim::species::testing::*;
use crate::sim::world::{Mark, World};
use crate::sim::{Params, Sim};

/// The all-grass fixture with a sized scent grid.
fn scented_world() -> World {
    let mut w = all_grass_world();
    w.init_scent(N_SPECIES);
    w
}

fn fox(x: usize, y: usize) -> crate::sim::creatures::Creature {
    let mut c = test_creature(x, y);
    c.species = FOX;
    c.genome = genome(FOX);
    c
}

fn set_mark(w: &mut World, x: usize, y: usize, holder: CreatureId, strength: f32) {
    *w.mark_mut(FOX, x, y).unwrap() = Mark { strength, holder };
}

#[test]
fn adult_marks_and_juvenile_does_not() {
    let mut w = scented_world();
    let tp = TerritoryParams::default();
    let mut adult = fox(20, 5);
    adult.id = CreatureId(7);
    territory::deposit(&adult, &mut w, &tp);
    let m = w.mark(FOX, 20, 5);
    assert_eq!(m.strength, tp.mark_per_tick);
    assert_eq!(m.holder, CreatureId(7), "a faint mark takes the depositor as holder");
    let mut pup = fox(21, 5);
    pup.adult = false;
    territory::deposit(&pup, &mut w, &tp);
    assert_eq!(w.mark(FOX, 21, 5), Mark::NONE, "juveniles do not mark");
    assert_eq!(w.mark(WOLF, 20, 5), Mark::NONE, "another species' block is untouched");
    // Twenty deposits saturate at 1.
    for _ in 0..30 {
        territory::deposit(&adult, &mut w, &tp);
    }
    assert_eq!(w.mark(FOX, 20, 5).strength, 1.0);
}

#[test]
fn holder_changes_only_below_hold_min() {
    let mut w = scented_world();
    let tp = TerritoryParams::default();
    let mut resident = fox(20, 5);
    resident.id = CreatureId(1);
    let mut floater = fox(20, 5);
    floater.id = CreatureId(2);
    set_mark(&mut w, 20, 5, CreatureId(1), 0.5);
    territory::deposit(&floater, &mut w, &tp);
    assert_eq!(w.mark(FOX, 20, 5).holder, CreatureId(1), "a strong mark keeps its holder");
    set_mark(&mut w, 20, 5, CreatureId(1), tp.hold_min - 0.01);
    territory::deposit(&floater, &mut w, &tp);
    assert_eq!(w.mark(FOX, 20, 5).holder, CreatureId(2), "a faded mark changes hands");
    // The neutral overlay never touches the grid.
    let mut off = TerritoryParams::default();
    off.neutral();
    set_mark(&mut w, 22, 5, CreatureId(0), 0.0);
    territory::deposit(&resident, &mut w, &off);
    assert_eq!(w.mark(FOX, 20, 5).holder, CreatureId(2));
    assert_eq!(w.mark(FOX, 22, 5), Mark::NONE);
}

#[test]
fn decay_frees_ground_after_nineteen_days() {
    let mut w = scented_world();
    let tp = TerritoryParams::default();
    set_mark(&mut w, 20, 5, CreatureId(1), 1.0);
    for _ in 0..18 {
        territory::decay(&mut w, &tp);
    }
    assert!(w.mark(FOX, 20, 5).strength >= tp.hold_min, "still held on day 18: {}", w.mark(FOX, 20, 5).strength);
    territory::decay(&mut w, &tp);
    assert!(w.mark(FOX, 20, 5).strength < tp.hold_min, "faded on day 19");
    assert_eq!(w.mark(FOX, 20, 5).holder, CreatureId(1), "the holder id stays until re-marked");
}

#[test]
fn kill_adds_kill_mark() {
    let mut w = scented_world();
    let tp = TerritoryParams::default();
    territory::deposit_kill(FOX, CreatureId(3), 10, 4, &mut w, &tp);
    assert_eq!(w.mark(FOX, 10, 4), Mark { strength: tp.kill_mark, holder: CreatureId(3) });
    // A kill on a rival's strong ground marks it but does not take it.
    set_mark(&mut w, 11, 4, CreatureId(9), 0.6);
    territory::deposit_kill(FOX, CreatureId(3), 11, 4, &mut w, &tp);
    assert_eq!(w.mark(FOX, 11, 4), Mark { strength: 1.0, holder: CreatureId(9) });
}

#[test]
fn foreign_is_zero_for_own_holder_and_for_social_readers() {
    let mut w = scented_world();
    let (tp, sp) = (TerritoryParams::default(), SocialParams::default());
    let mut me = fox(20, 5);
    me.id = CreatureId(1);
    set_mark(&mut w, 20, 5, CreatureId(1), 0.8);
    set_mark(&mut w, 21, 5, CreatureId(2), 0.8);
    set_mark(&mut w, 22, 5, CreatureId(2), tp.notice_min / 2.0);
    let view = territory::scent_view(&me, &w, &tp, &sp);
    assert!(view.avoid > 0.0);
    assert_eq!(view.foreign(&w, 20, 5), 0.0, "own ground");
    assert_eq!(view.foreign(&w, 21, 5), 0.8, "a rival's ground");
    assert_eq!(view.foreign(&w, 22, 5), 0.0, "below notice");
    assert_eq!(view.foreign(&w, 30, 5), 0.0, "unmarked");
    // A wolf (sociality 0.70) reads nothing.
    let mut wolf = test_creature(20, 5);
    wolf.species = WOLF;
    wolf.genome = genome(WOLF);
    let mut w2 = scented_world();
    *w2.mark_mut(WOLF, 21, 5).unwrap() = Mark { strength: 0.9, holder: CreatureId(2) };
    let view = territory::scent_view(&wolf, &w2, &tp, &sp);
    assert_eq!(view.avoid, 0.0);
    assert_eq!(view.foreign(&w2, 21, 5), 0.0, "a social reader shares ground");
    assert_eq!(Scent::NONE.patrol_factor(1.0), 1.0);
    assert_eq!(Scent::NONE.hunt_divisor(1.0), 1.0);
}

#[test]
fn hungry_reader_ignores_scent() {
    let mut w = scented_world();
    let (tp, sp) = (TerritoryParams::default(), SocialParams::default());
    set_mark(&mut w, 21, 5, CreatureId(2), 1.0);
    let mut me = fox(20, 5);
    me.id = CreatureId(1);
    me.hunger = 1.0;
    let view = territory::scent_view(&me, &w, &tp, &sp);
    assert_eq!(view.avoid, 0.0);
    assert_eq!(view.foreign(&w, 21, 5), 0.0);
    me.hunger = 0.0;
    let view = territory::scent_view(&me, &w, &tp, &sp);
    let want = tp.avoid_w * genome(FOX).aggression() * (1.0 - genome(FOX).sociality());
    assert!((view.avoid - want).abs() < 1e-6);
    assert!((view.patrol_factor(1.0) - (1.0 - want)).abs() < 1e-6);
    assert!((view.hunt_divisor(1.0) - (1.0 + want)).abs() < 1e-6);
}

#[test]
fn patrol_prefers_held_ground() {
    let mut w = scented_world();
    let (tp, sp, cp, diet) = (TerritoryParams::default(), SocialParams::default(), CreaturesParams::default(), DietParams::default());
    let mut me = fox(20, 5);
    me.id = CreatureId(1);
    me.hunger = 0.0;
    // Two equal prey hotspots; the nearer-in-scan-order one is a rival's ground.
    w.cell_mut(16, 5).prey_pressure = 0.5;
    w.cell_mut(24, 5).prey_pressure = 0.5;
    set_mark(&mut w, 16, 5, CreatureId(2), 1.0);
    let idx = empty_index(&w);
    let (plain, _) = perceive(&me, &idx, &w, &cp, &TickView::empty(), &sp, &diet, &Scent::NONE);
    assert_eq!(plain.best_patrol, Some((16, 5)), "without scent the first equal hotspot wins");
    let scent = territory::scent_view(&me, &w, &tp, &sp);
    let (scented, _) = perceive(&me, &idx, &w, &cp, &TickView::empty(), &sp, &diet, &scent);
    assert_eq!(scented.best_patrol, Some((24, 5)), "foreign scent discounts the rival's hotspot");
}

/// Two adult foxes in one store; the first holds row 5 from x = 20 to 26.
fn resident_and_intruder(intruder_at: (usize, usize)) -> (World, CreatureStore, CreatureId, CreatureId) {
    let mut w = scented_world();
    let mut store = CreatureStore::new();
    let a = store.insert(fox(20, 5));
    let b = store.insert(fox(intruder_at.0, intruder_at.1));
    for x in 20..=26 {
        set_mark(&mut w, x, 5, a, 0.6);
    }
    (w, store, a, b)
}

fn view_and_peers(store: &CreatureStore, w: &World, time: &crate::sim::time::Time, at: (usize, usize)) -> (TickView, Vec<CreatureId>) {
    let view = TickView::build(store, time, w, roster(), &GeneticsParams::default(), &DiseaseParams::default());
    let mut idx = SpatialIndex::new(w);
    idx.rebuild(store, w);
    let peers = idx.within(at.0, at.1, 12);
    (view, peers)
}

#[test]
fn resident_challenges_an_intruder_on_held_ground() {
    let (mut w, mut store, a, b) = resident_and_intruder((26, 5));
    let (tp, sp) = (TerritoryParams::default(), SocialParams::default());
    let t = day_time(12);
    let (view, peers) = view_and_peers(&store, &w, &t, (20, 5));
    let picked = territory::pick_intruder(store.get(a).unwrap(), &peers, &view, &w, &t, &tp, &sp);
    assert_eq!(picked, Some((b, (26, 5))));
    // The intruder holds nothing here, so it has no one to challenge.
    assert_eq!(territory::pick_intruder(store.get(b).unwrap(), &peers, &view, &w, &t, &tp, &sp), None);
    // A full tick at noon: the resident's replan picks Challenge and it
    // starts walking; six cells apart, no contest is rolled yet.
    let events = tick_once(&mut store, &mut w, &t, &PredationParams::default());
    let ra = store.get(a).unwrap();
    assert_eq!(ra.goal, Goal::Challenge);
    assert_eq!(ra.challenge_target, Some(b));
    assert!(ra.x > 20, "walking at the intruder");
    assert_eq!(ra.challenge_until, t.tick + u64::from(tp.challenge_ticks));
    assert!(events.iter().all(|e| e.kind != EventKind::Contest));
}

#[test]
fn nobody_challenges_off_held_ground() {
    let (tp, sp) = (TerritoryParams::default(), SocialParams::default());
    let t = day_time(12);
    // The intruder stands on unmarked ground.
    let (w, store, a, _b) = resident_and_intruder((30, 5));
    let (view, peers) = view_and_peers(&store, &w, &t, (20, 5));
    assert_eq!(territory::pick_intruder(store.get(a).unwrap(), &peers, &view, &w, &t, &tp, &sp), None);
    // On cooldown, or a juvenile, or a mate: no challenge either.
    let (mut w, mut store, a, b) = resident_and_intruder((22, 5));
    let (view, peers) = view_and_peers(&store, &w, &t, (20, 5));
    store.get_mut(a).unwrap().contest_cooldown_until = t.tick + 1;
    assert_eq!(territory::pick_intruder(store.get(a).unwrap(), &peers, &view, &w, &t, &tp, &sp), None);
    store.get_mut(a).unwrap().contest_cooldown_until = 0;
    store.get_mut(a).unwrap().mate_id = Some(b);
    assert_eq!(territory::pick_intruder(store.get(a).unwrap(), &peers, &view, &w, &t, &tp, &sp), None);
    store.get_mut(a).unwrap().mate_id = None;
    store.get_mut(b).unwrap().adult = false;
    let (view, peers) = view_and_peers(&store, &w, &t, (20, 5));
    assert_eq!(territory::pick_intruder(store.get(a).unwrap(), &peers, &view, &w, &t, &tp, &sp), None);
    // Social species share ground: two wolves on wolf-held cells never challenge.
    for id in [a, b] {
        let c = store.get_mut(id).unwrap();
        c.species = WOLF;
        c.genome = genome(WOLF);
        c.adult = true;
    }
    *w.mark_mut(WOLF, 20, 5).unwrap() = Mark { strength: 0.6, holder: a };
    *w.mark_mut(WOLF, 22, 5).unwrap() = Mark { strength: 0.6, holder: a };
    let (view, peers) = view_and_peers(&store, &w, &t, (20, 5));
    assert_eq!(territory::pick_intruder(store.get(a).unwrap(), &peers, &view, &w, &t, &tp, &sp), None);
}

/// A resident (id `a`) adjacent to its challenge target (id `b`), the
/// resident bonus large enough that the resident always wins.
fn adjacent_contest() -> (World, CreatureStore, CreatureId, CreatureId, TerritoryParams) {
    let (w, mut store, a, b) = resident_and_intruder((21, 5));
    let ra = store.get_mut(a).unwrap();
    ra.goal = Goal::Challenge;
    ra.challenge_target = Some(b);
    ra.challenge_until = 100;
    ra.genome.0[1] = 0.5; // size: the loser's injury is 0.1 × 0.5
    let tp = TerritoryParams { resident_bonus: 100.0, ..TerritoryParams::default() };
    (w, store, a, b, tp)
}

#[test]
fn contest_evicts_loser_and_overmarks_cell() {
    let (mut w, mut store, a, b, tp) = adjacent_contest();
    let t = day_time(12);
    let pp = PredationParams::default();
    let mut events = EventRing::new(16);
    territory::contest_contacts(&mut store, &mut w, &mut events, &t, roster(), &pp, &tp, &mut Rng::new(1), &mut DeathTallies::new(N_SPECIES));
    let (ra, rb) = (store.get(a).unwrap(), store.get(b).unwrap());
    assert_eq!(ra.goal, Goal::Patrol, "the winner replans");
    assert_eq!(ra.challenge_target, None);
    assert_eq!((ra.contests_won, ra.contests_lost), (1, 0));
    assert!((ra.energy - (0.8 - tp.contest_energy)).abs() < 1e-6);
    assert_eq!(rb.goal, Goal::Flee, "the loser is evicted");
    assert_eq!(rb.threatened_by, Some((20, 5, FOX)));
    assert_eq!(rb.flee_until, t.tick + u64::from(tp.evict_ticks));
    assert_eq!(rb.target, Some((21 + crate::cast!(pp.flee_distance.ceil() => usize), 5)), "away from the winner");
    assert!((rb.hp - (1.0 - tp.contest_injury * 0.5)).abs() < 1e-6);
    assert!((rb.energy - (0.8 - tp.contest_energy)).abs() < 1e-6);
    assert_eq!((rb.contests_won, rb.contests_lost), (0, 1));
    let cooldown = t.tick + u64::from(tp.contest_cooldown_days) * u64::from(t.ticks_per_day);
    assert_eq!((ra.contest_cooldown_until, rb.contest_cooldown_until), (cooldown, cooldown));
}

#[test]
fn contest_marks_ground_logs_event_and_clamps_injury() {
    let (mut w, mut store, a, b, tp) = adjacent_contest();
    let t = day_time(12);
    let pp = PredationParams::default();
    let mut events = EventRing::new(16);
    territory::contest_contacts(&mut store, &mut w, &mut events, &t, roster(), &pp, &tp, &mut Rng::new(1), &mut DeathTallies::new(N_SPECIES));
    assert_eq!(w.mark(FOX, 21, 5), Mark { strength: 1.0, holder: a }, "the winner over-marks the cell");
    let e = events.last().unwrap();
    assert_eq!(e.kind, EventKind::Contest);
    assert_eq!(e.subject, Some(a));
    assert_eq!(e.detail, format!("{}:{}:{}", a.0, b.0, w.region_index(21, 5)));
    assert!(e.text.contains("drove"), "{}", e.text);
    // Injury never kills: hp is clamped at the floor.
    let (mut w, mut store, _a, b, tp) = adjacent_contest();
    store.get_mut(b).unwrap().hp = 0.06;
    territory::contest_contacts(&mut store, &mut w, &mut EventRing::new(4), &t, roster(), &pp, &tp, &mut Rng::new(1), &mut DeathTallies::new(N_SPECIES));
    assert_eq!(store.get(b).unwrap().hp, tp.hp_floor);
}

#[test]
fn eviction_ends_after_evict_ticks_and_resumes_patrol() {
    let w = scented_world();
    let pp = PredationParams::default();
    let mut c = fox(21, 5);
    c.goal = Goal::Flee;
    c.threatened_by = Some((20, 5, FOX));
    c.flee_until = 10;
    let mut t = day_time(12);
    t.tick = 5;
    territory::preempt_predator(&mut c, &w, &t, &pp);
    assert_eq!(c.goal, Goal::Flee);
    assert_eq!(c.target, Some((21 + crate::cast!(pp.flee_distance.ceil() => usize), 5)));
    assert_eq!(c.replan_at, 6, "re-evaluated every tick, never replanned mid-flight");
    t.tick = 10;
    territory::preempt_predator(&mut c, &w, &t, &pp);
    assert_eq!(c.goal, Goal::Patrol);
    assert_eq!(c.threatened_by, None);
    assert_eq!(c.replan_at, 10, "replans at once");
    // A stale threat on a predator that is no longer fleeing is dropped.
    c.threatened_by = Some((20, 5, FOX));
    c.flee_until = 50;
    territory::preempt_predator(&mut c, &w, &t, &pp);
    assert_eq!((c.goal, c.threatened_by, c.flee_until), (Goal::Patrol, None, 0));
}

#[test]
fn mutual_border_challenge_resolves_once() {
    let (mut w, mut store, a, b, tp) = adjacent_contest();
    // The intruder is a resident of its own cell and challenges back.
    set_mark(&mut w, 21, 5, b, 0.6);
    let rb = store.get_mut(b).unwrap();
    rb.goal = Goal::Challenge;
    rb.challenge_target = Some(a);
    rb.challenge_until = 100;
    let t = day_time(12);
    let mut events = EventRing::new(16);
    territory::contest_contacts(&mut store, &mut w, &mut events, &t, roster(), &PredationParams::default(), &tp, &mut Rng::new(1), &mut DeathTallies::new(N_SPECIES));
    assert_eq!(events.iter().filter(|e| e.kind == EventKind::Contest).count(), 1, "one contest for the pair");
    let (ra, rb) = (store.get(a).unwrap(), store.get(b).unwrap());
    assert_eq!(ra.contests_won + rb.contests_won, 1);
    assert_eq!(ra.contests_lost + rb.contests_lost, 1);
    assert!(ra.challenge_target.is_none() && rb.challenge_target.is_none());
    // Neither can challenge again until the cooldown lapses.
    let (view, peers) = view_and_peers(&store, &w, &t, (20, 5));
    let sp = SocialParams::default();
    assert_eq!(territory::pick_intruder(ra, &peers, &view, &w, &t, &tp, &sp), None);
    assert_eq!(territory::pick_intruder(rb, &peers, &view, &w, &t, &tp, &sp), None);
}

/// The neutral overlay switches every part of the mechanic off, so a run
/// under it is bit for bit the pre-territory run: the checksum that
/// `sim::tests::checksum_is_fnv_stable` pinned before FR13 landed.
#[test]
fn neutral_territory_reproduces_the_old_checksum() {
    let mut p = Params::default();
    p.territory.neutral();
    let mut sim = Sim::new(7, p);
    for _ in 0..8640 {
        sim.step();
    }
    assert_eq!(sim.checksum(), 0xf883_9b51_57e3_c3f8);
    assert!(sim.world.scent.iter().all(|m| *m == Mark::NONE), "no scent was ever laid");
    assert!(sim.events.iter().all(|e| e.kind != EventKind::Contest));
}

