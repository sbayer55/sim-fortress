//! Tests for Diet breadth (genome slot 12): the graze score, the graze-in-place
//! rule and the bite.

use crate::sim::creatures::Goal;
use crate::sim::params::{CreaturesParams, DietParams, SocialParams};
use crate::sim::genetics::TickView;
use crate::sim::rng::Rng;
use crate::sim::species::IDX_DIET_BREADTH;
use crate::sim::world::{Terrain, World};
use super::perception::perceive;
use super::vitals::graze;
use super::tests::{all_grass_world, day_time, empty_index, plan, test_creature};

/// A grass strip with a forest block on the right half, every cell at 0.5
/// vegetation, so the only difference between the halves is the terrain.
fn grass_and_forest() -> World {
    let mut w = all_grass_world();
    let width = w.width;
    for (i, c) in w.cells.iter_mut().enumerate() {
        if i % width >= 30 {
            c.terrain = Terrain::Forest;
        }
    }
    w.refresh_shore();
    w
}

fn hungry_at(x: usize, breadth: f32) -> crate::sim::creatures::Creature {
    let mut c = test_creature(x, 5);
    c.hunger = 0.7;
    c.genome.0[IDX_DIET_BREADTH] = breadth;
    c
}

#[test]
fn specialist_ignores_forest_and_walks_to_grass() {
    let w = grass_and_forest();
    let idx = empty_index(&w);
    let cp = CreaturesParams::default();
    let diet = DietParams::default();
    // Standing in forest, four cells from the grass edge, with grass in sense range.
    let c = hungry_at(33, 0.2);
    let (p, _) = perceive(&c, &idx, &w, &cp, &TickView::empty(), &SocialParams::default(), &diet);
    let (cell, _) = p.best_graze.expect("grass is in range");
    assert!(cell.0 < 30, "a specialist's best graze cell must be grass, got {cell:?}");
    assert!(w.cell(cell.0, cell.1).terrain == Terrain::Grass);

    let mut c = c;
    plan(&mut c, &idx, &w, &day_time(12), &cp, &mut Rng::new(1));
    assert_eq!(c.goal, Goal::Graze);
    assert_eq!(c.target, Some(cell), "the forest under it is inedible, so it must not graze in place");
}

#[test]
fn generalist_grazes_forest() {
    let w = grass_and_forest();
    let idx = empty_index(&w);
    let cp = CreaturesParams::default();
    let mut c = hungry_at(33, 0.95);
    plan(&mut c, &idx, &w, &day_time(12), &cp, &mut Rng::new(1));
    assert_eq!(c.goal, Goal::Graze);
    assert_eq!(c.target, None, "forest is fully edible at breadth 0.95, so it grazes where it stands");
}

#[test]
fn specialist_bites_faster_on_meadow() {
    let cp = CreaturesParams::default();
    let diet = DietParams::default();
    let mut w = all_grass_world();
    for c in &mut w.cells {
        c.terrain = Terrain::GrassDense;
        c.vegetation = 1.0;
    }
    let mut specialist = hungry_at(5, 0.02);
    let mut generalist = hungry_at(5, 0.98);
    graze(&mut specialist, &mut w, &cp, &diet);
    let eaten_by_specialist = 1.0 - w.cell(5, 5).vegetation;
    w.cell_mut(5, 5).vegetation = 1.0;
    graze(&mut generalist, &mut w, &cp, &diet);
    let eaten_by_generalist = 1.0 - w.cell(5, 5).vegetation;
    assert!(eaten_by_specialist > eaten_by_generalist * 1.4, "{eaten_by_specialist} vs {eaten_by_generalist}");
    assert!(specialist.hunger < generalist.hunger, "the bigger bite relieves more hunger");
}

#[test]
fn inedible_terrain_feeds_nothing() {
    let cp = CreaturesParams::default();
    let diet = DietParams::default();
    let mut w = grass_and_forest();
    let mut c = hungry_at(40, 0.2);
    let before = c.hunger;
    graze(&mut c, &mut w, &cp, &diet);
    assert_eq!(c.hunger, before, "a grass specialist gets nothing from forest");
    assert!(w.cell(40, 5).vegetation < 0.5, "but the bite still removes the vegetation");
}

#[test]
fn neutral_diet_reproduces_the_old_score() {
    // The control overlay: every terrain edible, 1x bite. The graze score and
    // the bite must then match the pre-diet formula exactly.
    let cp = CreaturesParams::default();
    let mut diet = DietParams::default();
    diet.neutral();
    let mut w = grass_and_forest();
    let idx = empty_index(&w);
    let c = hungry_at(33, 0.2);
    let (p, _) = perceive(&c, &idx, &w, &cp, &TickView::empty(), &SocialParams::default(), &diet);
    let (cell, score) = p.best_graze.expect("everything is food");
    assert_eq!(cell, (33, 5), "the cell under it scores best at distance 0");
    assert_eq!(score, 0.5, "vegetation / (1 + 0/4)");
    let mut c = c;
    graze(&mut c, &mut w, &cp, &diet);
    assert_eq!(w.cell(33, 5).vegetation, 0.5 - cp.graze_per_hour);
    assert_eq!(c.hunger, 0.7 - cp.graze_nutrition * cp.graze_per_hour);
}
