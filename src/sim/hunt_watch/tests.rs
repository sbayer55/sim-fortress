use super::*;

fn key(hunter: u32, prey: u32) -> HuntKey {
    HuntKey { hunter: CreatureId(hunter), species: SpeciesId(4), prey: CreatureId(prey) }
}

fn sample(tick: u64, gap: u8) -> HuntSample {
    HuntSample {
        tick,
        gap,
        phase: HuntPhase::Chase,
        hunter_energy: 0.5,
        prey_energy: 0.5,
        hunter_hunger: 0.5,
        prey_goal: Goal::Flee,
        prey_terrain: Terrain::Grass,
    }
}

#[test]
fn ring_caps_at_36() {
    let mut w = HuntWatch::default();
    w.begin(key(1, 2), 0);
    let t = w.traces.get_mut(&CreatureId(1)).unwrap();
    for i in 0..50 {
        t.push_sample(sample(i, 5));
    }
    assert_eq!(t.samples().count(), RIBBON_LEN);
    assert_eq!(t.samples().next().unwrap().tick, 14, "the oldest samples fall off the front");
    assert_eq!(t.last_sample().unwrap().tick, 49);
}

#[test]
fn history_keeps_last_12_most_recent_first() {
    let mut w = HuntWatch::default();
    for i in 0..15u64 {
        w.begin(key(1, 2), i * 10);
        let outcome = if i % 2 == 0 { HuntOutcome::Kill { eat_until: i * 10 + 4 } } else { HuntOutcome::Miss };
        w.end(key(1, 2), outcome, i * 10 + 1, 3);
    }
    let t = w.trace(CreatureId(1)).unwrap();
    assert_eq!(t.history_len(), HISTORY_LEN);
    let bits: Vec<bool> = t.history().collect();
    assert_eq!(bits.len(), 12);
    assert!(bits[0], "the fifteenth attempt (even) was a kill and comes first");
    assert!(!bits[1], "the fourteenth was a miss");
    // A drop is not an attempt.
    w.begin(key(1, 2), 200);
    w.end(key(1, 2), HuntOutcome::Dropped, 201, 0);
    let t = w.trace(CreatureId(1)).unwrap();
    assert_eq!(t.history_len(), HISTORY_LEN);
    assert!(t.history().next().unwrap(), "the drop did not push a bit");
    assert_eq!(t.end().unwrap().outcome, HuntOutcome::Dropped);
}

#[test]
fn begin_is_idempotent_for_the_open_hunt_and_resets_after_end() {
    let mut w = HuntWatch::default();
    w.begin(key(1, 2), 0);
    w.traces.get_mut(&CreatureId(1)).unwrap().push_sample(sample(0, 6));
    w.begin(key(1, 2), 1);
    let t = w.trace(CreatureId(1)).unwrap();
    assert_eq!(t.samples().count(), 1, "the same open hunt is left alone");
    assert_eq!(t.start_tick(), 0);

    w.end(key(1, 2), HuntOutcome::Miss, 5, 2);
    w.begin(key(1, 2), 9);
    let t = w.trace(CreatureId(1)).unwrap();
    assert!(t.is_open());
    assert_eq!(t.samples().count(), 0, "a new hunt starts with an empty ribbon");
    assert_eq!(t.start_tick(), 9);
    assert_eq!(t.history_len(), 1, "the miss is remembered");

    // A switched target while open drops the old hunt without an attempt.
    w.begin(key(1, 3), 10);
    let t = w.trace(CreatureId(1)).unwrap();
    assert_eq!(t.prey(), CreatureId(3));
    assert!(t.is_open());
    assert_eq!(t.history_len(), 1);
}

#[test]
fn end_without_begin_creates_the_trace() {
    let mut w = HuntWatch::default();
    w.end(key(1, 2), HuntOutcome::Kill { eat_until: 9 }, 4, 0);
    let t = w.trace(CreatureId(1)).unwrap();
    assert!(!t.is_open());
    assert_eq!(t.end().unwrap(), HuntEnd { outcome: HuntOutcome::Kill { eat_until: 9 }, tick: 4, chase_ticks: 0 });
    assert_eq!(t.history().collect::<Vec<_>>(), vec![true]);
    assert_eq!(t.start_tick(), 4);
}

#[test]
fn prune_forgets_missing_hunters() {
    let mut w = HuntWatch::default();
    w.begin(key(1, 2), 0);
    w.begin(key(3, 2), 0);
    assert_eq!(w.len(), 2);
    w.prune(|id| id == CreatureId(1));
    assert_eq!(w.len(), 1);
    assert!(w.trace(CreatureId(3)).is_none());
    assert!(!w.is_empty());
}

#[test]
fn outcome_labels_and_attempt_rule() {
    assert_eq!(HuntOutcome::Kill { eat_until: 1 }.label(), "kill");
    assert_eq!(HuntOutcome::Timeout.label(), "timed out");
    assert!(HuntOutcome::Kill { eat_until: 1 }.is_kill());
    assert!(!HuntOutcome::Lost.is_kill());
    assert!(HuntOutcome::Miss.counts_attempt());
    assert!(!HuntOutcome::Dropped.counts_attempt());
}
