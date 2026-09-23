//! Quirks acceptance: named birth oddities are off by default and change nothing
//! then; switched on they are deterministic, inherited, saved, and they move the
//! simulation they claim to move.

// Test crates are separate compilation roots, so they do not inherit the allow
// list in `src/lib.rs`: indices come from checked loops, and a panic is the failure.
#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim_fortress::sim::params::{QuirkParams, QuirkTier};
use sim_fortress::sim::{save, EventKind, Params, Sim};

const DAY: u32 = 24;

fn with_quirks(q: QuirkParams) -> Params {
    Params { quirks: q, ..Params::default() }
}

fn on() -> QuirkParams {
    QuirkParams { enabled: true, ..QuirkParams::default() }
}

/// Step `ticks` times and count the events of `kind` pushed along the way.
fn run_counting(sim: &mut Sim, ticks: u32, kind: EventKind) -> usize {
    let mut n = 0;
    for _ in 0..ticks {
        let before = sim.events.total();
        sim.step();
        let fresh = usize::try_from(sim.events.total() - before).unwrap();
        n += sim.events.iter().rev().take(fresh).filter(|e| e.kind == kind).count();
    }
    n
}

#[test]
fn off_is_the_run_without_the_feature() {
    // A disabled section with a different catalogue and chances is inert.
    let mut q = QuirkParams { birth_chance: 1.0, inherit_chance: 1.0, ..QuirkParams::default() };
    q.catalog.truncate(3);
    let mut a = Sim::new(42, Params::default());
    let mut b = Sim::new(42, with_quirks(q));
    for _ in 0..DAY * 120 {
        a.step();
        b.step();
    }
    assert_eq!(a.checksum(), b.checksum());
    assert!(b.creatures.living().all(|c| c.quirks.is_empty()));
    assert!(b.events.iter().all(|e| e.kind != EventKind::Quirk));
}

#[test]
fn on_is_deterministic_and_announces_rare_births() {
    let p = with_quirks(QuirkParams { birth_chance: 0.6, rare_weight: 20.0, ..on() });
    let mut a = Sim::new(9, p.clone());
    let mut b = Sim::new(9, p);
    let quirk_events = run_counting(&mut a, DAY * 180, EventKind::Quirk);
    for _ in 0..DAY * 180 {
        b.step();
    }
    assert_eq!(a.checksum(), b.checksum(), "same seed, same quirky run");
    assert!(quirk_events > 0, "rare births are announced");
    let qp = &a.params.quirks;
    for c in a.creatures.living() {
        for i in c.quirks.iter() {
            let d = &qp.catalog[i];
            for j in c.quirks.iter().filter(|j| *j != i) {
                assert!(!d.excludes.contains(&qp.catalog[j].name), "{} with {}", d.name, qp.catalog[j].name);
            }
        }
    }
}

#[test]
fn common_quirks_run_in_families() {
    // Fresh rolls are rare, inheritance is certain: a pup's quirks mostly come
    // from its parents, so carriers cluster in lineages.
    let p = with_quirks(QuirkParams { birth_chance: 0.05, inherit_chance: 1.0, ..on() });
    let mut sim = Sim::new(11, p);
    for _ in 0..DAY * 300 {
        sim.step();
    }
    let qp = sim.params.quirks.clone();
    let (mut inherited, mut pups) = (0, 0);
    for c in sim.creatures.living().filter(|c| c.parents.is_some()) {
        let (m, f) = c.parents.unwrap();
        let parent = |id| sim.creatures.get(id).map(|p| p.quirks).or_else(|| sim.lineage.get(id).map(|n| n.quirks)).unwrap_or_default();
        let from_parents = parent(m).0 | parent(f).0;
        if from_parents == 0 {
            continue;
        }
        pups += 1;
        let heritable = c.quirks.iter().filter(|i| qp.catalog[*i].tier != QuirkTier::Legendary).any(|i| from_parents & (1 << i) != 0);
        if heritable {
            inherited += 1;
        }
    }
    assert!(pups > 20, "enough pups of quirky parents: {pups}");
    assert!(inherited * 10 >= pups * 8, "{inherited} of {pups} pups carry a parent's quirk");
}

#[test]
fn undying_founders_do_not_die_of_age() {
    // Every founder is Undying (lifespan x4); the baseline world sees age deaths.
    let undying = QuirkParams { birth_chance: 1.0, extra_chance: 0.0, max_per_creature: 1, common_weight: 0.0, rare_weight: 0.0, legendary_weight: 1.0, ..on() };
    let mut q = undying;
    q.catalog.retain(|d| d.name == "Undying");
    let mut base = Sim::new(42, Params::default());
    let mut quirky = Sim::new(42, with_quirks(q));
    assert!(quirky.creatures.living().all(|c| c.quirks.len() == 1));
    let ticks = DAY * 360;
    let base_age = run_counting(&mut base, ticks, EventKind::DeathAge);
    let quirky_age = run_counting(&mut quirky, ticks, EventKind::DeathAge);
    assert!(base_age > 0, "the baseline has age deaths");
    assert!(quirky_age * 4 < base_age, "Undying cuts age deaths: {quirky_age} vs {base_age}");
}

#[test]
fn saves_keep_quirks() {
    let mut sim = Sim::new(5, with_quirks(QuirkParams { birth_chance: 0.5, ..on() }));
    for _ in 0..DAY * 30 {
        sim.step();
    }
    let dir = std::env::temp_dir().join(format!("simf-quirks-{}", std::process::id()));
    let path = save::save(&sim, "Quirky", &dir).unwrap();
    let loaded = save::load(&path).unwrap().sim;
    let _ = std::fs::remove_dir_all(&dir);
    assert!(loaded.params.quirks.enabled);
    assert_eq!(loaded.checksum(), sim.checksum());
    let masks = |s: &Sim| s.creatures.living().map(|c| (c.id, c.quirks, c.qm)).collect::<Vec<_>>();
    assert_eq!(masks(&loaded), masks(&sim));
    assert!(masks(&sim).iter().any(|(_, q, _)| !q.is_empty()));
}
