//! S17 Hunt Watch: the hunt traces through the real step, on the arena harness.

use super::*;
use crate::sim::hunt_watch::{HuntOutcome, HuntTrace};

/// A stealthy wolf at (10,5) and a pinned, visible hare at (10,12): stalk, then chase.
fn stalking_pair(sim: &mut Sim) -> (CreatureId, CreatureId) {
    let wolf = put(sim, stealthy_wolf(), 0.9);
    let mut hare = creature(0, HARE, 10, 12);
    hare.genome.0[5] = 0.0;
    let hare = put(sim, hare, 0.3);
    still(sim, hare);
    (wolf, hare)
}

#[test]
fn trace_opens_on_first_stalk_tick() {
    let mut sim = arena(|_| {});
    let (wolf, hare) = stalking_pair(&mut sim);
    sim.step();
    let t = sim.hunts.trace(wolf).expect("the hunter has a trace");
    assert_eq!(t.prey(), hare);
    assert_eq!(t.start_tick(), sim.time.tick);
    assert!(t.is_open());
    assert!(t.chase().is_none(), "detection does not start the clock");
    let s = t.last_sample().expect("one sample after the first tick");
    assert_eq!(t.samples().count(), 1);
    assert_eq!(s.phase, HuntPhase::Stalk);
    assert!(s.gap >= 4 && s.gap <= 7, "gap {} after one tick of walking in", s.gap);
    assert!(sim.hunts.trace(hare).is_none(), "prey never get a trace");
}

#[test]
fn kill_records_samples_chase_start_and_a_hit() {
    let mut sim = arena(|p| {
        p.kill_min = 1.0;
        p.kill_max = 1.0;
    });
    let (wolf, hare) = stalking_pair(&mut sim);
    let mut steps = 0;
    for _ in 0..12 {
        sim.step();
        steps += 1;
        if !sim.creatures.get(hare).unwrap().alive {
            break;
        }
    }
    let w = sim.creatures.get(wolf).unwrap();
    let h = sim.creatures.get(hare).unwrap();
    assert!(!h.alive);
    let t = sim.hunts.trace(wolf).unwrap();
    let end = t.end().expect("the kill closed the trace");
    assert_eq!(end.outcome, HuntOutcome::Kill { eat_until: w.eat_until.unwrap() });
    assert_eq!(end.tick, sim.time.tick);
    assert_eq!(end.chase_ticks, h.death.unwrap().chase_ticks, "the same chase length the death record carries");
    // Chase and contact can share a tick; observe runs between them, so the clock start is recorded.
    let chase = t.chase().expect("the clock start was recorded before the kill cleared it");
    assert_eq!(end.tick - chase.tick, u64::from(end.chase_ticks));
    assert_eq!(t.samples().count(), steps, "one sample per tick up to and including the contact tick");
    assert_eq!(t.last_sample().unwrap().gap, 1, "the contact sample is the last one");
    assert_eq!(t.history().collect::<Vec<_>>(), vec![true]);
    assert!(!t.is_open());
    sim.step();
    assert_eq!(sim.hunts.trace(wolf).unwrap().samples().count(), steps, "no samples while eating");
}

#[test]
fn miss_records_outcome_and_a_miss() {
    let (sim, wolf, _hare, t) = failed_hunt_fixture();
    let trace = sim.hunts.trace(wolf).unwrap();
    let end = trace.end().unwrap();
    assert_eq!(end.outcome, HuntOutcome::Miss);
    assert_eq!(end.tick, t);
    assert_eq!(trace.history().collect::<Vec<_>>(), vec![false]);
    assert!(trace.samples().count() >= 1, "the contact tick was sampled before the roll");
    assert!(!trace.is_open());
}

#[test]
fn timeout_records_outcome() {
    // The clock starts at detection (trigger 8 ≥ the 7-cell gap) and runs out in
    // three ticks, before the wolf can close seven cells.
    let mut sim = arena(|p| {
        p.chase_trigger_cheb = 8;
        p.chase_max_ticks = 3;
        p.chase_speed_bonus = 0.0;
        p.kill_min = 1.0;
        p.kill_max = 1.0;
    });
    // A slow wolf: a third of a cell per tick, so seven cells take far longer than the clock.
    sim.params.creatures.move_speed_base = 0.3;
    sim.params.creatures.move_speed_per_trait = 0.0;
    let (wolf, hare) = stalking_pair(&mut sim);
    let mut end = None;
    for _ in 0..8 {
        sim.step();
        if let Some(e) = sim.hunts.trace(wolf).and_then(HuntTrace::end) {
            end = Some(e);
            break;
        }
    }
    let end = end.expect("the chase clock ran out");
    assert_eq!(end.outcome, HuntOutcome::Timeout);
    assert_eq!(end.chase_ticks, 3);
    assert!(sim.creatures.get(hare).unwrap().alive);
    assert_eq!(sim.creatures.get(wolf).unwrap().attempts, 1, "a timeout is an attempt");
    assert_eq!(sim.hunts.trace(wolf).unwrap().history().collect::<Vec<_>>(), vec![false]);
}

#[test]
fn lost_when_prey_leaves_sense_range() {
    let mut sim = arena(|_| {});
    let (wolf, hare) = stalking_pair(&mut sim);
    sim.step();
    assert!(sim.hunts.trace(wolf).unwrap().is_open());
    {
        let h = sim.creatures.get_mut(hare).unwrap();
        h.x = 100;
        h.migrate_target = Some((100, h.y));
    }
    sim.spatial.rebuild(&sim.creatures, &sim.world);
    sim.step();
    let t = sim.hunts.trace(wolf).unwrap();
    assert_eq!(t.end().unwrap().outcome, HuntOutcome::Lost);
    assert_eq!(t.history().collect::<Vec<_>>(), vec![false], "losing the prey counts as an attempt, as fail_hunt does");
    assert_eq!(sim.creatures.get(wolf).unwrap().attempts, 1);
}

#[test]
fn dropped_when_the_hunter_turns_to_another_goal() {
    let mut sim = arena(|_| {});
    let (wolf, hare) = stalking_pair(&mut sim);
    sim.step();
    // The hunter is pulled off the hunt without fail_hunt: its target stays set
    // but its goal is no longer Hunt (here pinned the way `still` pins a prey).
    still(&mut sim, wolf);
    assert_eq!(sim.creatures.get(wolf).unwrap().hunt_target, Some(hare), "the stale target is still held");
    sim.step();
    let t = sim.hunts.trace(wolf).unwrap();
    assert_eq!(t.end().unwrap().outcome, HuntOutcome::Dropped);
    assert_eq!(t.history_len(), 0, "a drop is not an attempt");
    assert_eq!(sim.creatures.get(wolf).unwrap().attempts, 0);
}

#[test]
fn dead_hunter_is_marked_and_its_trace_lingers_with_the_slot() {
    let mut sim = arena(|_| {});
    let (wolf, _hare) = stalking_pair(&mut sim);
    sim.step();
    {
        let w = sim.creatures.get_mut(wolf).unwrap();
        w.hp = 0.0;
        w.hunger = 1.5;
    }
    sim.step();
    let w = sim.creatures.get(wolf).unwrap();
    assert!(!w.alive, "hp 0 kills the wolf this tick");
    let t = sim.hunts.trace(wolf).unwrap();
    let died = t.died().expect("the trace notes the death");
    assert_eq!(died.cause, w.death.unwrap().cause);
    assert_eq!(died.tick, sim.time.tick);
    assert!(!t.is_open());
    for _ in 0..5 {
        sim.step();
    }
    assert!(sim.hunts.trace(wolf).is_some(), "the trace stays while the carcass slot exists");
}

#[test]
fn kill_odds_matches_the_contact_formula() {
    let mut sim = arena(|_| {});
    let (wolf, hare) = stalking_pair(&mut sim);
    let pp = &sim.params.predation;
    let (w, h) = (sim.creatures.get(wolf).unwrap(), sim.creatures.get(hare).unwrap());
    let want = (pp.kill_chance(w.genome.speed(), h.genome.speed(), w.genome.aggression(), h.genome.size()) + 0.0 + sim.params.social.pack_kill_bonus * 0.0)
        .clamp(pp.kill_min, pp.kill_max);
    let parts = kill_odds(&sim, wolf, hare).expect("both alive");
    assert_eq!(parts.total, want);
    assert_eq!(parts.pack, 0.0, "a lone hunter has no pack bonus");
    assert_eq!(parts.sick, 0.0);
    assert!((parts.base + parts.speed + parts.aggression + parts.size - parts.formula).abs() < 1e-6 || parts.formula == pp.kill_min || parts.formula == pp.kill_max);
    assert!(kill_odds(&sim, wolf, CreatureId(9999)).is_none());
}

#[test]
fn hunt_watch_is_deterministic() {
    let run = || {
        let mut sim = Sim::new(7, Params::default());
        for _ in 0..2000 {
            sim.step();
        }
        sim
    };
    let (a, b) = (run(), run());
    assert!(!a.hunts.is_empty(), "the default world hunts within 2000 ticks");
    assert_eq!(a.hunts, b.hunts);
    assert_eq!(a.checksum(), b.checksum());
}
