use super::*;
use crate::sim::creatures::Cause;
use crate::sim::species::{Genome, SpeciesId};

fn rec(died_day: u32) -> LifeRecord {
    LifeRecord {
        species: SpeciesId::default(),
        genome: Genome([0.5; crate::sim::species::N_TRAITS]),
        born_day: -3,
        died_day,
        cause: Cause::Starved,
        offspring: 2,
        kills: 1,
        attempts: 4,
        chased: 5,
        escaped: 3,
    }
}

#[test]
fn age_counts_from_birth_and_founders_are_negative() {
    assert_eq!(rec(10).age_days(), 13);
}

#[test]
fn records_older_than_the_window_are_dropped() {
    let mut log = LifeLog::default();
    log.push(rec(0));
    log.push(rec(LIFE_WINDOW_DAYS));
    assert_eq!(log.len(), 2, "day 0 is exactly one window old and stays");
    log.push(rec(LIFE_WINDOW_DAYS + 1));
    assert_eq!(log.len(), 2);
    assert!(log.records().all(|r| r.died_day >= LIFE_WINDOW_DAYS));
}

#[test]
fn the_cap_drops_the_oldest() {
    let mut log = LifeLog::default();
    for _ in 0..=LIFE_LOG_MAX {
        log.push(rec(5));
    }
    assert_eq!(log.len(), LIFE_LOG_MAX);
}

/// Every death `kill` records in the lineage is also in the life log, with the
/// same cause and day (120 days: too short for generation pruning to bite).
#[test]
fn kill_logs_every_death() {
    let mut sim = crate::sim::Sim::new(7, crate::sim::Params::default());
    for _ in 0..24 * 120 {
        sim.step();
    }
    let dead: Vec<_> = sim.lineage.nodes().filter(|n| n.died_day.is_some()).collect();
    assert!(!dead.is_empty(), "no deaths in 120 days");
    assert_eq!(sim.lineage.lives().len(), dead.len());
    for n in dead {
        let hit = sim.lineage.lives().records().find(|r| r.species == n.species && r.born_day == n.born_day && r.genome == n.genome);
        let Some(r) = hit else { panic!("no life record for {:?}", n.id) };
        assert_eq!(Some(r.died_day), n.died_day);
        assert_eq!(Some(r.cause), n.cause);
    }
}
