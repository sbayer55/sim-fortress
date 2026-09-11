//! Predation (C5): the single source of truth for detection, the kill-chance
//! formula and the hunt-phase helpers shared with `behavior`.
//!
//! Detection is deterministic and distance-independent inside the sense ring:
//! a predator detects a prey within its sense range unless the prey is hidden
//! by cover × camouflage, or is resting on a den cell while `den_protects`.

use crate::sim::creatures::{Creature, Goal};
use crate::sim::geom;
use crate::sim::params::PredationParams;
use crate::sim::world::World;

/// The largest possible sense range. The sense trait is clamped to 0.02..0.98,
/// so `sense_cells = 2 + floor(sense × 10)` tops out at 11; one extra cell of
/// margin for the predator-first prey query (FR5).
pub const MAX_SENSE_CELLS: u16 = 12;

/// FR2 (predator rule): a predator detects a prey within its sense range unless
/// `prey.camouflage × cover_by_terrain[prey cell] ≥ pred.sense × detect_threshold`,
/// or the prey is resting on a den cell and `den_protects`.
pub fn can_detect(pred: &Creature, prey: &Creature, world: &World, p: &PredationParams) -> bool {
    if geom::dist(pred.x, pred.y, prey.x, prey.y) > pred.genome.sense_cells() as f32 {
        return false;
    }
    if p.den_protects && prey.goal == Goal::Rest && in_den(world, prey.x, prey.y) {
        return false;
    }
    let cover = p.cover_by_terrain.get(&world.cell(prey.x, prey.y).terrain).copied().unwrap_or(0.0);
    let hidden = prey.genome.camouflage() * cover >= pred.genome.sense() * p.detect_threshold;
    !hidden
}

/// FR2 (predator rule) over a peer snapshot: same detection rule as `can_detect`,
/// used by hunt-target selection where the prey is only available as a `Peer`.
pub fn can_detect_peer(pred: &Creature, px: usize, py: usize, cam: f32, resting: bool, world: &World, p: &PredationParams) -> bool {
    if geom::dist(pred.x, pred.y, px, py) > pred.genome.sense_cells() as f32 {
        return false;
    }
    if p.den_protects && resting && in_den(world, px, py) {
        return false;
    }
    let cover = p.cover_by_terrain.get(&world.cell(px, py).terrain).copied().unwrap_or(0.0);
    !(cam * cover >= pred.genome.sense() * p.detect_threshold)
}

/// FR2 (prey rule): a prey detects a predator within its own sense range
/// (halved while resting) when `pred.camouflage < prey.sense`.
pub fn prey_detects_pred(prey: &Creature, pred: &Creature, p: &PredationParams) -> bool {
    let range = if prey.goal == Goal::Rest {
        prey.genome.sense_cells() as f32 * p.rest_detect_factor
    } else {
        prey.genome.sense_cells() as f32
    };
    if geom::dist(prey.x, prey.y, pred.x, pred.y) > range {
        return false;
    }
    pred.genome.camouflage() < prey.genome.sense()
}

fn in_den(world: &World, x: usize, y: usize) -> bool {
    world.dens.iter().any(|&(dx, dy)| dx == x && dy == y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::creatures::{CreatureId, Sex};
    use crate::sim::params::{CreaturesParams, PredationParams, WorldParams};
    use crate::sim::species::SpeciesId;
    use crate::sim::world::Terrain;

    /// A minimal creature with the given species and genome at `(x, y)`.
    fn creature(id: u32, species: SpeciesId, x: usize, y: usize) -> Creature {
        let mut c = crate::sim::creatures::place_founders(
            &World::generate(7, &WorldParams::default()),
            &CreaturesParams::default(),
            &mut crate::sim::rng::Rng::new(1),
        )
        .remove(0);
        c.id = CreatureId(id);
        c.species = species;
        c.x = x;
        c.y = y;
        c.genome = species.base_genome();
        c.sex = Sex::Female;
        c.alive = true;
        c.goal = Goal::Wander;
        c
    }

    /// A flat all-`terrain` world with every cell `terrain`.
    fn flat_world(terrain: Terrain) -> World {
        let mut w = World::generate(7, &WorldParams { width: 30, height: 10, ..WorldParams::default() });
        for c in &mut w.cells {
            c.terrain = terrain;
        }
        w
    }

    #[test]
    fn can_detect_cover_table() {
        // Acceptance: hare camouflage 0.9 vs wolf sense 0.7 hidden in Forest
        // (0.90 ≥ 0.56) and visible on Sand (0.36 < 0.56).
        let p = PredationParams::default();
        let forest = flat_world(Terrain::Forest);
        let sand = flat_world(Terrain::Sand);

        let mut wolf = creature(1, SpeciesId::Wolf, 5, 5);
        wolf.genome.0[2] = 0.7; // sense
        let mut hare = creature(2, SpeciesId::Hare, 6, 5);
        hare.genome.0[5] = 0.9; // camouflage

        assert!(!can_detect(&wolf, &hare, &forest, &p), "0.90 ≥ 0.56 → hidden in Forest");
        assert!(can_detect(&wolf, &hare, &sand, &p), "0.36 < 0.56 → visible on Sand");
    }

    #[test]
    fn den_protects() {
        let p = PredationParams::default();
        let mut w = flat_world(Terrain::Grass);
        w.dens.push((6, 5));
        let mut wolf = creature(1, SpeciesId::Wolf, 5, 5);
        wolf.genome.0[2] = 1.0; // strong sense
        let mut hare = creature(2, SpeciesId::Hare, 6, 5);
        hare.genome.0[5] = 0.0; // no camouflage
        hare.goal = Goal::Rest;
        assert!(!can_detect(&wolf, &hare, &w, &p), "resting prey on a den is never targeted");

        // Off the den it is visible.
        hare.x = 7;
        hare.y = 5;
        assert!(can_detect(&wolf, &hare, &w, &p));
    }

    #[test]
    fn cheb_vs_ellipse_usage() {
        // Perception uses the ellipse metric: a cell at dx=2, dy=0 is only 1.0
        // cells away, so a sense-6 creature sees it; a cell at dx=0, dy=7 is 7 away.
        let p = PredationParams::default();
        let w = flat_world(Terrain::Grass);
        let mut wolf = creature(1, SpeciesId::Wolf, 5, 5);
        wolf.genome.0[2] = 0.4; // sense_cells = 2 + 4 = 6
        let mut hare = creature(2, SpeciesId::Hare, 5, 5);
        hare.genome.0[5] = 0.0;
        hare.x = 7; // dx 2 → ellipse 1.0
        hare.y = 5;
        assert!(can_detect(&wolf, &hare, &w, &p), "ellipse metric: dx2 = 1 cell");
        hare.x = 5;
        hare.y = 12; // dy 7 → ellipse 7
        assert!(!can_detect(&wolf, &hare, &w, &p), "dy7 = 7 cells > sense 6");
    }

    #[test]
    fn kill_chance_bounds() {
        let p = PredationParams::default();
        assert!((p.kill_chance(0.5, 0.5, 0.5, 0.5) - 0.40).abs() < 1e-6, "base 0.35 + 0.3×0.5 − 0.2×0.5");
        assert_eq!(p.kill_chance(1.0, 0.0, 1.0, 0.0), p.kill_max, "clamps at max");
        assert_eq!(p.kill_chance(0.0, 1.0, 0.0, 1.0), p.kill_min, "clamps at min");
    }
}
