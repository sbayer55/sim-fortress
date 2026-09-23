use super::*;
use crate::sim::params::Params;
use crate::sim::Sim;

fn on() -> QuirkParams {
    QuirkParams { enabled: true, ..QuirkParams::default() }
}

fn idx(qp: &QuirkParams, name: &str) -> usize {
    qp.position(name).unwrap()
}

#[test]
fn off_by_default_and_catalog_valid() {
    let qp = QuirkParams::default();
    assert!(!qp.enabled);
    assert_eq!(qp.catalog.len(), 44);
    qp.validate().unwrap();
}

#[test]
fn empty_set_folds_to_identity() {
    assert_eq!(fold(QuirkSet(0), &on()), QuirkMods::IDENTITY);
    assert_eq!(scaled(0.37, 1.0), 0.37);
    assert_eq!(scale_ticks(12, 1.0), 12);
}

#[test]
fn fold_multiplies_and_scaled_clamps() {
    let qp = on();
    let mut s = QuirkSet::default();
    s.insert(idx(&qp, "Swift"));
    s.insert(idx(&qp, "Glutton"));
    let m = fold(s, &qp);
    assert!((m.speed - 1.3).abs() < 1e-6);
    assert!((m.hunger - 1.1 * 1.35).abs() < 1e-6);
    assert_eq!(scaled(0.9, 3.0), 0.98);
    let mut c = QuirkSet::default();
    c.insert(idx(&qp, "Cannibal"));
    assert_eq!(fold(c, &qp).cannibal_hunger, qp.cannibal_hunger);
}

#[test]
fn rolls_are_deterministic_and_respect_rules() {
    let qp = QuirkParams { birth_chance: 1.0, extra_chance: 1.0, ..on() };
    for id in 0..2_000u32 {
        let kind = if id % 2 == 0 { Kind::Prey } else { Kind::Predator };
        let set = roll(&qp, 42, CreatureId(id), kind, &[]);
        assert_eq!(set, roll(&qp, 42, CreatureId(id), kind, &[]));
        assert_eq!(set.len(), qp.max_per_creature);
        for i in set.iter() {
            let d = &qp.catalog[i];
            match d.kinds {
                QuirkKinds::Prey => assert_eq!(kind, Kind::Prey, "{}", d.name),
                QuirkKinds::Predator => assert_eq!(kind, Kind::Predator, "{}", d.name),
                QuirkKinds::Any => {}
            }
            for j in set.iter().filter(|j| *j != i) {
                assert!(!d.excludes.contains(&qp.catalog[j].name), "{} with {}", d.name, qp.catalog[j].name);
            }
        }
    }
}

#[test]
fn inheritance_rate_tracks_inherit_chance() {
    let qp = QuirkParams { birth_chance: 0.0, ..on() };
    let mut parent = QuirkSet::default();
    parent.insert(idx(&qp, "Swift"));
    let n = 4_000u32;
    let got = (0..n).filter(|i| roll(&qp, 7, CreatureId(*i), Kind::Prey, &[parent]).has(idx(&qp, "Swift"))).count();
    let rate = crate::cast!(got => f32) / crate::cast!(n => f32);
    assert!((rate - qp.inherit_chance).abs() < 0.04, "rate {rate}");
}

#[test]
fn legendary_quirks_are_not_inherited() {
    let qp = QuirkParams { birth_chance: 0.0, inherit_chance: 1.0, ..on() };
    let mut parent = QuirkSet::default();
    parent.insert(idx(&qp, "Undying"));
    parent.insert(idx(&qp, "Robust"));
    let set = roll(&qp, 1, CreatureId(9), Kind::Prey, &[parent]);
    assert!(set.has(idx(&qp, "Robust")));
    assert!(!set.has(idx(&qp, "Undying")));
}

#[test]
fn sim_off_rolls_nothing() {
    let sim = Sim::new(3, Params::default());
    assert!(sim.creatures.living().all(|c| c.quirks.is_empty() && c.qm == QuirkMods::IDENTITY));
}

#[test]
fn sim_on_rolls_founders_and_newborns_deterministically() {
    let p = Params { quirks: QuirkParams { birth_chance: 0.5, ..on() }, ..Params::default() };
    let run = || {
        let mut sim = Sim::new(5, p.clone());
        let founders = sim.creatures.living().filter(|c| !c.quirks.is_empty()).count();
        for _ in 0..24 * 60 {
            sim.step();
        }
        (founders, sim.checksum(), sim)
    };
    let (founders, sum, sim) = run();
    assert!(founders > 0, "some founders carry quirks");
    assert_eq!(sum, run().1, "quirky runs are deterministic");
    let born = sim.creatures.living().filter(|c| c.parents.is_some()).collect::<Vec<_>>();
    assert!(born.iter().any(|c| !c.quirks.is_empty()), "some newborns carry quirks");
    for c in sim.creatures.living() {
        assert_eq!(c.qm, fold(c.quirks, &sim.params.quirks));
        if let Some(n) = sim.lineage.get(c.id) {
            assert_eq!(n.quirks, c.quirks, "lineage keeps the mask");
        }
    }
}
