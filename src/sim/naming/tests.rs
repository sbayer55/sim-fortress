//! Player renames: normalisation, where a name shows, and that it never
//! touches the run.

use super::*;
use crate::sim::{Kind, Params};

/// A fresh default world and one living predator founder in it.
fn world_and_predator() -> (Sim, CreatureId) {
    let sim = Sim::new(7, Params::default());
    let id = sim.creatures.living().find(|c| sim.roster().kind(c.species) == Kind::Predator).map(|c| c.id);
    (sim, id.unwrap_or(CreatureId(u32::MAX)))
}

#[test]
fn clean_keeps_printable_ascii_collapses_spaces_and_caps_length() {
    assert_eq!(clean("  Old   Grey ", 12).as_deref(), Some("Old Grey"));
    assert_eq!(clean("Fang\u{2603}", 12).as_deref(), Some("Fang"), "non-ASCII is dropped");
    assert_eq!(clean("Tab\tbed", 12).as_deref(), Some("Tab bed"));
    assert_eq!(clean("Abcdefghijklmnop", 12).as_deref(), Some("Abcdefghijkl"));
    assert_eq!(clean("Abcdefghijk mnop", 12).as_deref(), Some("Abcdefghijk"), "a cut never leaves a trailing space");
    assert_eq!(clean("   ", 12), None);
    assert_eq!(clean("", 12), None);
}

#[test]
fn a_renamed_founder_shows_its_name_everywhere_and_clearing_restores_it() {
    let (mut sim, id) = world_and_predator();
    let before = sim.creatures.get(id).map(|c| c.label(sim.roster())).unwrap_or_default();
    let rev = sim.names_rev;
    assert!(sim.rename_creature(id, " Fang "));
    assert_eq!(sim.names_rev, rev + 1);
    let c = sim.creatures.get(id).expect("living");
    assert_eq!(c.name_str(sim.roster()), "Fang");
    assert!(c.label(sim.roster()).starts_with("Fang "));
    assert_eq!(sim.lineage.get(id).map(|n| n.name_str(sim.roster())), Some("Fang"));
    let line = sim.lineage.dynasties().get(id).expect("a predator founder roots a line");
    assert_eq!(line.founder, c.label(sim.roster()), "the founder label follows the rename");

    assert!(sim.rename_creature(id, "   "));
    assert_eq!(sim.creatures.get(id).map(|c| c.label(sim.roster())), Some(before.clone()));
    assert_eq!(sim.lineage.dynasties().get(id).map(|l| l.founder.clone()), Some(before));
    assert!(!sim.rename_creature(CreatureId(u32::MAX), "Nobody"));
}

#[test]
fn a_dynasty_takes_a_name_and_an_unknown_root_is_refused() {
    let (mut sim, id) = world_and_predator();
    assert!(sim.rename_dynasty(id, "The Grey Court of the North Wood"));
    assert_eq!(sim.lineage.dynasties().get(id).and_then(|l| l.name.as_deref()), Some("The Grey Court of th"));
    let view = crate::sim::lineage::dynasties::rank(&sim).into_iter().find(|v| v.root == id).expect("ranked");
    assert_eq!(view.name.as_deref(), Some("The Grey Court of th"));
    assert!(sim.rename_dynasty(id, ""));
    assert_eq!(sim.lineage.dynasties().get(id).and_then(|l| l.name.clone()), None);
    assert!(!sim.rename_dynasty(CreatureId(u32::MAX), "Nobody"));
}

#[test]
fn names_do_not_change_the_run() {
    let (mut named, id) = world_and_predator();
    let (mut plain, _) = world_and_predator();
    assert!(named.rename_creature(id, "Fang"));
    assert!(named.rename_dynasty(id, "Fang's Pack"));
    for _ in 0..480 {
        named.step();
        plain.step();
    }
    assert_eq!(named.checksum(), plain.checksum());
}

#[test]
fn names_survive_a_save_round_trip() {
    let (mut sim, id) = world_and_predator();
    assert!(sim.rename_creature(id, "Fang"));
    assert!(sim.rename_dynasty(id, "Fang's Pack"));
    let dir = std::env::temp_dir().join(format!("simf-naming-{}", std::process::id()));
    let loaded = crate::sim::save::save(&sim, "Named", &dir).and_then(|p| crate::sim::save::load(&p)).map(|l| l.sim);
    let _ = std::fs::remove_dir_all(&dir);
    let loaded = loaded.unwrap_or_else(|e| panic!("round trip: {e}"));
    assert_eq!(loaded.creatures.get(id).and_then(|c| c.nickname.as_deref()), Some("Fang"));
    assert_eq!(loaded.lineage.get(id).and_then(|n| n.nickname.as_deref()), Some("Fang"));
    assert_eq!(loaded.lineage.dynasties(), sim.lineage.dynasties());
}
