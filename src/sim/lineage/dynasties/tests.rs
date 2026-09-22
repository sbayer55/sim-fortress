//! Dynasty store: founding, joining, death folds, year rows and retention.

use std::collections::BTreeMap;

use super::*;
use crate::sim::creatures::{place_founders, Cause, CreatureStore, Mutation};
use crate::sim::lineage::Lineage;
use crate::sim::params::{CreaturesParams, GeneticsParams, WorldParams};
use crate::sim::rng::Rng;
use crate::sim::species::testing::*;
use crate::sim::world::World;

/// A creature of `species` with the given id, generation and parents.
fn creature(species: SpeciesId, id: u32, gen: u32, parents: Option<(u32, u32)>) -> Creature {
    let w = World::generate(7, &WorldParams::default());
    let founders = place_founders(&w, roster(), &CreaturesParams::default(), &GeneticsParams::default(), 0.20, &mut Rng::new(1));
    let mut c = founders.into_iter().find(|c| c.species == species).unwrap_or_else(|| panic!("no founder of {species:?}"));
    c.id = CreatureId(id);
    c.generation = gen;
    c.parents = parents.map(|(m, f)| (CreatureId(m), CreatureId(f)));
    c.born_day = crate::cast!(gen => i32) * 10;
    c
}

#[test]
fn founders_open_lines_children_join_them_and_prey_are_ignored() {
    let mut lin = Lineage::new();
    let fox = creature(FOX, 10, 1, None);
    let vixen = creature(FOX, 11, 1, None);
    let mut kit = creature(FOX, 12, 2, Some((10, 11)));
    kit.mutations.push(Mutation { trait_idx: 0, delta: 0.1, generation: 2 });
    let vole = creature(VOLE, 20, 1, None);
    for c in [&fox, &vixen, &kit, &vole] {
        lin.record(c, roster(), 0.5);
    }
    let d = lin.dynasties();
    assert_eq!(d.len(), 2, "two fox lines, no vole line");
    let line = d.get(CreatureId(10)).expect("the fox founded a line");
    assert_eq!((line.members_ever, line.members_living, line.max_generation, line.generations()), (2, 2, 2, 2));
    assert_eq!(line.founder, fox.label(roster()));
    assert_eq!(line.founded_day, fox.born_day);
    assert_eq!(d.get(CreatureId(11)).map(|l| l.members_ever), Some(1), "the vixen's line has only her: the kit follows its mother");
    assert!(d.get(CreatureId(20)).is_none());
}

#[test]
fn deaths_fold_into_the_line_and_pruning_does_not_shrink_it() {
    let mut lin = Lineage::new();
    let mut fox = creature(FOX, 10, 1, None);
    fox.kills = 5;
    fox.offspring = 3;
    fox.escaped = 2;
    let mut kit = creature(FOX, 12, 2, Some((10, 11)));
    kit.kills = 7;
    kit.mutations.push(Mutation { trait_idx: 1, delta: -0.05, generation: 2 });
    lin.record(&fox, roster(), 0.5);
    lin.record(&kit, roster(), 0.5);
    lin.record_death(fox.id, 40, Cause::Age, None, 0);
    lin.record_dynasty_death(&fox, roster(), 40);
    let line = lin.dynasties().get(CreatureId(10)).expect("line");
    assert_eq!(line.dead, Tally { kills: 5, terr: 0, young: 3, surv: 2, age: 0, muts: 0 });
    assert_eq!((line.members_living, line.died_out_day), (1, None));
    lin.record_death(kit.id, 60, Cause::Predation, None, 0);
    lin.record_dynasty_death(&kit, roster(), 60);
    let line = lin.dynasties().get(CreatureId(10)).expect("line");
    assert_eq!(line.dead, Tally { kills: 12, terr: 0, young: 3, surv: 2, age: 0, muts: 1 });
    assert_eq!((line.members_living, line.died_out_day), (0, Some(60)));
    // Prune every node: the record is untouched.
    let before = lin.dynasties().clone();
    lin.prune(&[20; 6], 0, &CreatureStore::new());
    assert!(lin.get(CreatureId(10)).is_none() && lin.get(CreatureId(12)).is_none());
    assert_eq!(lin.dynasties(), &before);
    // A prey death never reaches the store.
    let vole = creature(VOLE, 20, 1, None);
    lin.record(&vole, roster(), 0.5);
    lin.record_dynasty_death(&vole, roster(), 61);
    assert_eq!(lin.dynasties().len(), 1);
}

#[test]
fn year_rows_add_the_living_cap_at_forty_and_drop_extinct_lines_after_ten_years() {
    let mut d = Dynasties::default();
    let fox = creature(FOX, 10, 1, None);
    d.record_birth(CreatureId(10), &fox, roster());
    let mut living = BTreeMap::new();
    living.insert(CreatureId(10), Tally { kills: 4, terr: 9, young: 1, surv: 0, age: 360, muts: 0 });
    d.close_year(1, 360, 360, &living);
    let line = d.get(CreatureId(10)).expect("line");
    assert_eq!(line.years, vec![YearRow { year: 1, stats: Tally { kills: 4, terr: 9, young: 1, surv: 0, age: 360, muts: 0 } }]);
    for y in 2..=46u32 {
        d.close_year(y, y * 360, 360, &living);
    }
    let line = d.get(CreatureId(10)).expect("line");
    assert_eq!(line.years.len(), YEARS_KEPT);
    assert_eq!(line.years.first().map(|r| r.year), Some(7), "the oldest rows go first");
    // The line dies out on day 47·360; it is kept for ten years, then dropped.
    let mut dead_fox = fox;
    dead_fox.kills = 9;
    d.record_death(CreatureId(10), &dead_fox, 47 * 360);
    d.close_year(47, 47 * 360, 360, &BTreeMap::new());
    assert_eq!(d.get(CreatureId(10)).and_then(|l| l.years.last()).map(|r| r.stats.kills), Some(9), "dead totals carry the row");
    d.close_year(57, 57 * 360, 360, &BTreeMap::new());
    assert_eq!(d.len(), 1, "ten years on, still listed");
    d.close_year(58, 58 * 360, 360, &BTreeMap::new());
    assert!(d.is_empty(), "eleven years on, dropped");
}

#[test]
fn the_first_day_of_a_year_closes_a_row_on_every_line() {
    use crate::sim::params::Params;
    let mut p = Params::default();
    p.species.clear_initial_counts();
    p.species.set_initial_count("fox", 4);
    p.time.season_days = 2; // eight-day years
    let mut sim = Sim::new(3, p);
    assert_eq!(sim.lineage.dynasties().len(), 4, "four founders, four lines");
    assert!(sim.lineage.dynasties().iter().all(|l| l.years.is_empty()));
    let ticks_per_year = 8 * u64::from(sim.time.ticks_per_day);
    for _ in 0..ticks_per_year + 8 {
        sim.step();
    }
    assert_eq!(sim.time.year(), 2);
    let living = living_totals(&sim);
    let living_foxes = sim.creatures.living().count();
    let d = sim.lineage.dynasties();
    assert_eq!(d.iter().map(|l| l.members_living).sum::<u32>(), crate::cast!(living_foxes => u32), "incremental living count matches the store");
    for line in d.iter() {
        assert_eq!(line.years.len(), 1, "{}: one closed year", line.founder);
        let row = line.years.first().copied().expect("row");
        assert_eq!(row.year, 1);
        // A founder alive at the close was at least eight days old; a line that
        // starved out before the close carries the zero age of its dead.
        let died_before_close = line.died_out_day.is_some_and(|d| d <= 8);
        assert!(row.stats.age >= 8 || died_before_close, "{}: age {} at the close", line.founder, row.stats.age);
        // Age is a day-level quantity, so for a line still alive the row closed
        // today matches the living pass; a line that lost its last fox since
        // the close keeps the age it had.
        if let Some(live) = living.get(&line.root) {
            assert_eq!(row.stats.age, live.age, "{}: oldest living member", line.founder);
        }
        assert!(row.stats.kills >= line.dead.kills);
    }
}
