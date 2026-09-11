//! C3 herbivore acceptance tests.

use std::time::Instant;

use sim_fortress::sim::{EventKind, Params, Sim};

fn run_days(sim: &mut Sim, days: u64) {
    for _ in 0..days * 24 {
        sim.step();
    }
}

/// Run until the population reaches zero or `cap_days` elapse.
fn run_to_extinction(sim: &mut Sim, cap_days: u64) -> u64 {
    for day in 1..=cap_days {
        run_days(sim, 1);
        if sim.creatures.len_living() == 0 {
            return day;
        }
    }
    cap_days
}

#[test]
fn all_die_without_reproduction() {
    let mut sim = Sim::new(42, Params::default());
    let start = sim.creatures.len_living();
    assert!(start > 0, "expected founders to be placed");
    let mut prev = start;
    let mut extinct_day = None;
    for day in 1..=1200u64 {
        run_days(&mut sim, 1);
        let now = sim.creatures.len_living();
        assert!(now <= prev, "population increased on day {day}: {prev} → {now}");
        prev = now;
        if now == 0 {
            extinct_day = Some(day);
            break;
        }
    }
    let ed = extinct_day.expect("population did not reach zero by day 1200");
    assert!(ed <= 1100, "extinct on day {ed}, expected ≤ 1100");
}

#[test]
fn death_causes_all_present() {
    let mut sim = Sim::new(42, Params::default());
    run_to_extinction(&mut sim, 1200);
    let starved = sim.events.iter().filter(|e| e.kind == EventKind::DeathStarved).count();
    let thirst = sim.events.iter().filter(|e| e.kind == EventKind::DeathThirst).count();
    let age = sim.events.iter().filter(|e| e.kind == EventKind::DeathAge).count();
    assert!(starved > 0, "no starvation deaths");
    assert!(thirst > 0, "no thirst deaths");
    assert!(age > 0, "no old-age deaths");
}

#[test]
fn food_is_findable() {
    // Food is reachable: creatures survive the opening days and graze vegetation
    // down rather than all starving immediately.
    let mut sim = Sim::new(42, Params::default());
    let start = sim.creatures.len_living();
    run_days(&mut sim, 30);
    let alive = sim.creatures.len_living();
    assert!(alive as f32 >= start as f32 * 0.5, "only {alive}/{start} alive after 30 days — food unreachable?");
}

#[test]
fn deer_deaths_not_all_starvation() {
    // Acceptance: among deer deaths in the first 90 days, at least 60% are not
    // DeathStarved (vacuously true when there are no deer deaths).
    let mut sim = Sim::new(42, Params::default());
    run_days(&mut sim, 90);
    let deer_deaths: Vec<_> = sim.events.iter().filter(|e| e.species == Some(sim_fortress::sim::SpeciesId::Deer) && e.kind.is_death()).collect();
    if deer_deaths.is_empty() {
        return;
    }
    let starved = deer_deaths.iter().filter(|e| e.kind == EventKind::DeathStarved).count();
    let not_starved = deer_deaths.len() - starved;
    assert!(not_starved as f32 >= deer_deaths.len() as f32 * 0.6, "too many deer starved early: {starved}/{}", deer_deaths.len());
}

#[test]
fn performance_budget() {
    // Headless 1200 days must stay well under the 15 s budget.
    let mut sim = Sim::new(42, Params::default());
    let t0 = Instant::now();
    run_days(&mut sim, 1200);
    let elapsed = t0.elapsed();
    assert!(elapsed.as_secs_f64() < 15.0, "1200 days took {elapsed:?}");
}
