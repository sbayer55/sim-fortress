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
    cam * cover < pred.genome.sense() * p.detect_threshold
}

/// FR2 (prey rule): a prey detects a predator within its own sense range
/// (halved while resting) when `pred.camouflage < prey.sense`.
pub fn prey_detects_pred(prey: &Creature, pred: &Creature, p: &PredationParams) -> bool {
    prey_detects_pred_at(prey, pred.x, pred.y, pred.genome.camouflage(), p)
}

/// FR2 (prey rule) against a predator snapshot `(x, y, camouflage)`; used by the
/// per-tick predator-first threat query, which avoids cloning predators.
pub fn prey_detects_pred_at(prey: &Creature, px: usize, py: usize, pred_camouflage: f32, p: &PredationParams) -> bool {
    let range = if prey.goal == Goal::Rest {
        prey.genome.sense_cells() as f32 * p.rest_detect_factor
    } else {
        prey.genome.sense_cells() as f32
    };
    if geom::dist(prey.x, prey.y, px, py) > range {
        return false;
    }
    pred_camouflage < prey.genome.sense()
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

    // ---- Hunt / scavenge harness: a flat Grass world with hand-placed creatures ----

    use crate::sim::creatures::{Cause, HuntPhase};
    use crate::sim::{EventKind, Params, Sim};

    /// A `Sim` with no founders, every cell Grass, no dens, and the given
    /// predation overrides; creatures are inserted by the test.
    fn arena(tweak: impl Fn(&mut PredationParams)) -> Sim {
        let mut p = Params::default();
        p.creatures.initial_counts.clear();
        tweak(&mut p.predation);
        let mut sim = Sim::new(7, p);
        for c in &mut sim.world.cells {
            c.terrain = Terrain::Grass;
            c.vegetation = 0.5;
        }
        sim.world.dens.clear();
        sim.world.refresh_shore();
        sim
    }

    /// Insert `c` (hunger/thirst/energy set to sensible test values) and rebuild the index.
    fn put(sim: &mut Sim, mut c: Creature, hunger: f32) -> CreatureId {
        c.hunger = hunger;
        c.thirst = 0.1;
        c.energy = 0.9;
        c.replan_at = 0;
        let id = sim.creatures.insert(c);
        sim.spatial.rebuild(&sim.creatures, &sim.world);
        id
    }

    /// Pin a creature in place: no replans, no target (hunt tests need a static prey).
    fn still(sim: &mut Sim, id: CreatureId) {
        let c = sim.creatures.get_mut(id).unwrap();
        // A never-ending migration to its own cell: `goal_satisfied` stays false,
        // so the creature neither replans nor moves (Wander would replan at once).
        c.replan_at = u64::MAX;
        c.target = None;
        c.goal = Goal::Migrate;
        c.migrate_until = u64::MAX;
        c.migrate_target = Some((c.x, c.y));
    }

    /// A hungry diurnal wolf at (10,5) that a hare cannot detect (camouflage 0.99).
    fn stealthy_wolf() -> Creature {
        let mut w = creature(0, SpeciesId::Wolf, 10, 5);
        w.genome.0[5] = 0.99; // camouflage: the hare never detects it (prey rule)
        w.genome.0[2] = 0.7; // sense_cells 9
        w
    }

    #[test]
    fn hunt_phases_and_single_roll() {
        // Guaranteed kill: Stalk → Chase (trigger at cheb ≤ 4) → contact → one roll.
        let mut sim = arena(|p| {
            p.kill_min = 1.0;
            p.kill_max = 1.0;
        });
        let wolf = put(&mut sim, stealthy_wolf(), 0.9);
        let mut hare = creature(0, SpeciesId::Hare, 11, 5);
        hare.genome.0[5] = 0.0; // no camouflage: always visible
        let hare = put(&mut sim, hare, 0.3);
        still(&mut sim, hare);
        sim.step();
        let w = sim.creatures.get(wolf).unwrap();
        let h = sim.creatures.get(hare).unwrap();
        assert!(!h.alive, "contact at cheb ≤ 1 with kill chance 1.0 kills the hare");
        assert_eq!(h.death.unwrap().cause, Cause::Predation);
        assert_eq!(h.death.unwrap().killer, Some(wolf));
        assert_eq!((w.kills, w.attempts), (1, 1), "one roll per contact");
        assert_eq!(w.hunt_phase, HuntPhase::Eat);
        assert_eq!(w.kills_by_species[SpeciesId::Hare.index()], 1);
        assert!(w.last_kill.map(|k| k.0) == Some(hare));
        assert!(sim.events.iter().any(|e| e.kind == EventKind::DeathPredation && e.subject == Some(hare)));
        assert_eq!(sim.deaths.hunt_kills[SpeciesId::Wolf.index()], 1);
        sim.step();
        let w = sim.creatures.get(wolf).unwrap();
        assert_eq!(w.attempts, 1, "no second roll while eating");
        assert_eq!(w.hunt_phase, HuntPhase::Eat, "eating lasts eat_hours");

        // Guaranteed failure: the predator idles, the hunt fails once, and the
        // prey flees for flee_ticks even though it cannot detect the wolf.
        let mut sim = arena(|p| {
            p.kill_min = 0.0;
            p.kill_max = 0.0;
            p.hunt_cooldown_hours = 100; // no second attempt during the flee
        });
        let wolf = put(&mut sim, stealthy_wolf(), 0.9);
        let mut hare = creature(0, SpeciesId::Hare, 11, 5);
        hare.genome.0[5] = 0.0;
        let hare = put(&mut sim, hare, 0.3);
        still(&mut sim, hare);
        sim.step();
        let t = sim.time.tick;
        let w = sim.creatures.get(wolf).unwrap();
        let h = sim.creatures.get(hare).unwrap();
        assert!(h.alive);
        assert_eq!((w.kills, w.attempts), (0, 1));
        assert_eq!(w.goal, Goal::Patrol, "failed hunt idles one tick");
        assert_eq!(w.hunt_target, None);
        assert_eq!(w.hunt_cooldown_until, t + sim.params.predation.hunt_cooldown_hours as u64);
        assert_eq!(h.goal, Goal::Flee, "prey flees regardless of detection");
        assert_eq!(h.flee_until, t + sim.params.predation.flee_ticks as u64);
        assert!(h.threatened_by.is_some(), "the forced threat gives the away-vector");
        assert_eq!(h.chased, 1);
        assert_eq!(sim.deaths.hunt_attempts[SpeciesId::Wolf.index()], 1);
        // The forced flee is retained for the whole timer, then ends as an escape.
        for _ in 0..(sim.params.predation.flee_ticks - 1) {
            sim.step();
            assert_eq!(sim.creatures.get(hare).unwrap().goal, Goal::Flee, "forced flee lasts flee_ticks");
        }
        sim.step();
        let h = sim.creatures.get(hare).unwrap();
        assert_ne!(h.goal, Goal::Flee);
        assert_eq!(h.escaped, 1);
        assert_eq!(sim.creatures.get(wolf).unwrap().attempts, 1, "cooldown blocks a re-roll");
    }

    #[test]
    fn chase_clock_starts_at_trigger() {
        let mut sim = arena(|p| {
            p.kill_min = 1.0;
            p.kill_max = 1.0;
        });
        let wolf = put(&mut sim, stealthy_wolf(), 0.9);
        // dy = 7 → ellipse distance 7 (inside sense 9), cheb 7 > chase_trigger 4.
        let mut hare = creature(0, SpeciesId::Hare, 10, 12);
        hare.genome.0[5] = 0.0;
        let hare = put(&mut sim, hare, 0.3);
        still(&mut sim, hare);
        sim.step();
        let detected_at = sim.time.tick;
        let w = sim.creatures.get(wolf).unwrap();
        assert_eq!(w.goal, Goal::Hunt);
        assert_eq!(w.hunt_target, Some(hare));
        assert_eq!(w.hunt_phase, HuntPhase::Stalk, "detection does not start the clock");
        assert_eq!(w.chase_start_tick, None);
        // Walk in: the clock starts on the tick cheb first drops to ≤ 4, which is
        // at least one tick after detection, and the kill record carries only
        // the ticks since then.
        let mut killed_at = None;
        for _ in 0..10 {
            sim.step();
            let w = sim.creatures.get(wolf).unwrap();
            if w.hunt_phase == HuntPhase::Chase {
                assert_eq!(w.chase_start_tick, Some(sim.time.tick), "clock starts on the trigger tick");
            }
            if !sim.creatures.get(hare).unwrap().alive {
                killed_at = Some(sim.time.tick);
                break;
            }
        }
        let killed_at = killed_at.expect("the wolf should reach and kill the pinned hare");
        let chase_ticks = sim.creatures.get(hare).unwrap().death.unwrap().chase_ticks as u64;
        assert!(killed_at - detected_at >= 2, "needs at least two ticks to close 7 cells");
        assert!(chase_ticks < killed_at - detected_at, "chase_ticks {chase_ticks} counts from the trigger, not from detection");
        assert!(chase_ticks <= 1);
    }

    #[test]
    fn eat_reduces_hunger_and_decay() {
        let mut sim = arena(|p| {
            p.kill_min = 1.0;
            p.kill_max = 1.0;
        });
        let wolf = put(&mut sim, stealthy_wolf(), 0.9);
        let mut hare = creature(0, SpeciesId::Hare, 11, 5);
        hare.genome.0[5] = 0.0;
        let size = hare.genome.size();
        let hare = put(&mut sim, hare, 0.3);
        still(&mut sim, hare);
        sim.step();
        let pp = &sim.params.predation;
        let w = sim.creatures.get(wolf).unwrap();
        let h = sim.creatures.get(hare).unwrap();
        assert!(w.hunger < 0.9 - pp.hunger_per_kill(size) + 0.05, "hunger −= hunger_per_kill: {}", w.hunger);
        assert_eq!(w.eat_until, Some(sim.time.tick + pp.eat_hours(size) as u64));
        assert!((h.decay - pp.kill_consumes_decay).abs() < 1e-6, "carcass decay advanced by kill_consumes_decay");
        // The wolf eats on the carcass cell, then patrols.
        for _ in 0..pp.eat_hours(size) + 1 {
            sim.step();
        }
        let w = sim.creatures.get(wolf).unwrap();
        assert_ne!(w.hunt_phase, HuntPhase::Eat);
        assert_eq!(w.hunt_target, None);
    }

    /// A dead prey carcass at `(x, y)` with the given decay.
    fn carcass(species: SpeciesId, x: usize, y: usize, decay: f32) -> Creature {
        let mut c = creature(0, species, x, y);
        c.alive = false;
        c.decay = decay;
        c.death = Some(crate::sim::creatures::Death { cause: Cause::Age, day: 0, killer: None, chase_ticks: 0 });
        c
    }

    #[test]
    fn scavenge_consumes_decay() {
        let mut sim = arena(|_| {});
        let wolf = put(&mut sim, stealthy_wolf(), 0.8); // > scavenge_hunger_min
        let k = sim.creatures.insert(carcass(SpeciesId::Hare, 11, 5, 0.2));
        sim.world.carcasses.push((11, 5));
        sim.spatial.rebuild(&sim.creatures, &sim.world);
        sim.step();
        let pp = &sim.params.predation;
        let w = sim.creatures.get(wolf).unwrap();
        let c = sim.creatures.get(k).unwrap();
        let expect = pp.carcass_nutrition * (1.0 - 0.2);
        assert!(w.hunger < 0.8 - expect + 0.05, "hunger −= nutrition × (1 − decay): {}", w.hunger);
        assert!((c.decay - (0.2 + pp.scavenge_consumes_decay)).abs() < 1e-6);
        assert_ne!(w.goal, Goal::Scavenge, "one visit, then move on");
    }

    #[test]
    fn scavenge_prey_carcass_only() {
        let mut sim = arena(|_| {});
        let wolf = put(&mut sim, stealthy_wolf(), 0.8);
        let k = sim.creatures.insert(carcass(SpeciesId::Fox, 11, 5, 0.2));
        sim.world.carcasses.push((11, 5));
        sim.spatial.rebuild(&sim.creatures, &sim.world);
        sim.step();
        let w = sim.creatures.get(wolf).unwrap();
        let c = sim.creatures.get(k).unwrap();
        assert_ne!(w.goal, Goal::Scavenge, "predator carcasses are never scavenged");
        assert!(w.hunger > 0.8, "no nutrition taken: {}", w.hunger);
        assert!((c.decay - 0.2).abs() < 1e-6);
    }
}
