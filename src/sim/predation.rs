//! Predation (C5): the single source of truth for detection, the kill-chance
//! formula and the hunt-phase helpers shared with `behavior`.
//!
//! Detection is deterministic and distance-independent inside the sense ring:
//! a predator detects a prey within its sense range unless the prey is hidden
//! by cover × camouflage, or is resting on a den cell while `den_protects`.

use crate::sim::creatures::{Creature, CreatureId, CreatureStore, Goal, HuntPhase};
use crate::sim::disease;
use crate::sim::geom;
use crate::sim::params::{PredationParams, SocialParams};
use crate::sim::species::SpeciesId;
use crate::sim::world::World;
use crate::sim::Sim;

/// The largest possible sense range. The sense trait is clamped to 0.02..0.98,
/// so `sense_cells = 2 + floor(sense × 10)` tops out at 11; one extra cell of
/// margin for the predator-first prey query (FR5).
pub const MAX_SENSE_CELLS: u16 = 12;

/// FR2 (predator rule): a predator detects a prey within its sense range unless
/// `prey.camouflage × cover_by_terrain[prey cell] ≥ pred.sense × detect_threshold`,
///
/// or the prey is resting on a den cell and `den_protects`.
pub fn can_detect(pred: &Creature, prey: &Creature, world: &World, p: &PredationParams) -> bool {
    if geom::dist(pred.x, pred.y, prey.x, prey.y) > f32::from(pred.genome.sense_cells()) {
        return false;
    }
    if p.den_protects && prey.goal == Goal::Rest && in_den(world, prey.x, prey.y) {
        return false;
    }
    let cover = p.cover_by_terrain.get(&world.cell(prey.x, prey.y).terrain).copied().unwrap_or(0.0);
    let hidden = prey.genome.camouflage() * cover >= pred.genome.sense() * p.effective_detect_threshold();
    !hidden
}

/// FR2 (predator rule) over a peer snapshot: same detection rule as `can_detect`,
/// used by hunt-target selection where the prey is only available as a `Peer`.
pub fn can_detect_peer(pred: &Creature, px: usize, py: usize, cam: f32, resting: bool, world: &World, p: &PredationParams) -> bool {
    if geom::dist(pred.x, pred.y, px, py) > f32::from(pred.genome.sense_cells()) {
        return false;
    }
    if p.den_protects && resting && in_den(world, px, py) {
        return false;
    }
    let cover = p.cover_by_terrain.get(&world.cell(px, py).terrain).copied().unwrap_or(0.0);
    cam * cover < pred.genome.sense() * p.effective_detect_threshold()
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
        f32::from(prey.genome.sense_cells()) * p.rest_detect_factor
    } else {
        f32::from(prey.genome.sense_cells())
    };
    if geom::dist(prey.x, prey.y, px, py) > range {
        return false;
    }
    pred_camouflage < prey.genome.sense()
}

/// The parts of the contact roll at `behavior::hunt::hunt_contacts`, in the
/// order they are summed, so S17 can show the same number the sim rolls under.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OddsParts {
    /// `effective_kill_base()`.
    pub base: f32,
    /// `kill_speed_w × (pred.speed − prey.speed)`.
    pub speed: f32,
    /// `kill_aggression_w × pred.aggression`.
    pub aggression: f32,
    /// `−kill_size_w × prey.size`.
    pub size: f32,
    /// `kill_chance(..)`: the four parts, clamped.
    pub formula: f32,
    /// `pack_kill_bonus × min(participants, 3)`.
    pub pack: f32,
    /// The prey's disease `kill_bonus`.
    pub sick: f32,
    /// `(formula + sick + pack).clamp(kill_min, kill_max)`: what the roll uses.
    pub total: f32,
}

/// The raw facts of one contest, as `hunt_contacts` snapshots them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contest {
    pub pred_speed: f32,
    pub pred_aggression: f32,
    pub prey_speed: f32,
    pub prey_size: f32,
    /// Packmates at the kill, already capped at three.
    pub extra: usize,
    pub sick: f32,
}

/// The contact roll's parts. `total` is computed with the exact expression
/// `hunt_contacts` used before this function existed, so the checksum holds.
pub fn odds(pp: &PredationParams, sp: &SocialParams, c: Contest) -> OddsParts {
    let formula = pp.kill_chance(c.pred_speed, c.prey_speed, c.pred_aggression, c.prey_size);
    let pack = sp.pack_kill_bonus * crate::cast!(c.extra => f32);
    OddsParts {
        base: pp.effective_kill_base(),
        speed: pp.kill_speed_w * (c.pred_speed - c.prey_speed),
        aggression: pp.kill_aggression_w * c.pred_aggression,
        size: -pp.kill_size_w * c.prey_size,
        formula,
        pack,
        sick: c.sick,
        total: (formula + c.sick + pack).clamp(pp.kill_min, pp.kill_max),
    }
}

/// Who is hunting whom, and where the prey stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HuntPair {
    pub pred: CreatureId,
    pub pred_species: SpeciesId,
    pub prey: CreatureId,
    pub prey_at: (usize, usize),
}

/// Packmates hunting the same prey within sharing range of it (C8 FR4).
pub fn pack_participants(store: &CreatureStore, pair: HuntPair, sp: &SocialParams) -> Vec<CreatureId> {
    store
        .living()
        .filter(|o| {
            o.id != pair.pred
                && o.species == pair.pred_species
                && o.goal == Goal::Hunt
                && o.hunt_phase != HuntPhase::Eat
                && o.hunt_target == Some(pair.prey)
                && geom::cheb(o.x, o.y, pair.prey_at.0, pair.prey_at.1) <= sp.pack_share_cheb
        })
        .map(|o| o.id)
        .collect()
}

/// The odds the hunter would roll under if it made contact now: the S17 entry.
/// `None` when either creature is gone.
pub fn kill_odds(sim: &Sim, pred: CreatureId, prey: CreatureId) -> Option<OddsParts> {
    let (p, q) = (sim.creatures.get(pred)?, sim.creatures.get(prey)?);
    let pair = HuntPair { pred, pred_species: p.species, prey, prey_at: (q.x, q.y) };
    let extra = pack_participants(&sim.creatures, pair, &sim.params.social).len().min(3);
    let contest = Contest {
        pred_speed: p.genome.speed(),
        pred_aggression: p.genome.aggression(),
        prey_speed: q.genome.speed(),
        prey_size: q.genome.size(),
        extra,
        sick: disease::effects(q, &sim.params.disease).kill_bonus,
    };
    Some(odds(&sim.params.predation, &sim.params.social, contest))
}

fn in_den(world: &World, x: usize, y: usize) -> bool {
    world.dens.iter().any(|&(dx, dy)| dx == x && dy == y)
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {

    use super::*;
    use crate::sim::species::testing::*;
    use crate::sim::creatures::{CreatureId, Sex};
    use crate::sim::params::{CreaturesParams, GeneticsParams, PredationParams, WorldParams};
    use crate::sim::species::SpeciesId;
    use crate::sim::world::Terrain;

    /// A minimal creature with the given species and genome at `(x, y)`.
    fn creature(id: u32, species: SpeciesId, x: usize, y: usize) -> Creature {
        let mut c = crate::sim::creatures::place_founders(
            &World::generate(7, &WorldParams::default()),
            roster(),
            &CreaturesParams::default(),
            &GeneticsParams::default(),
            0.20,
            &mut crate::sim::rng::Rng::new(1),
        )
        .remove(0);
        c.id = CreatureId(id);
        c.species = species;
        c.x = x;
        c.y = y;
        c.genome = genome(species);
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

        let mut wolf = creature(1, WOLF, 5, 5);
        wolf.genome.0[2] = 0.7; // sense
        let mut hare = creature(2, HARE, 6, 5);
        hare.genome.0[5] = 0.9; // camouflage

        assert!(!can_detect(&wolf, &hare, &forest, &p), "0.90 ≥ 0.56 → hidden in Forest");
        assert!(can_detect(&wolf, &hare, &sand, &p), "0.36 < 0.56 → visible on Sand");
    }

    #[test]
    fn den_protects() {
        let p = PredationParams::default();
        let mut w = flat_world(Terrain::Grass);
        w.dens.push((6, 5));
        let mut wolf = creature(1, WOLF, 5, 5);
        wolf.genome.0[2] = 1.0; // strong sense
        let mut hare = creature(2, HARE, 6, 5);
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
        let mut wolf = creature(1, WOLF, 5, 5);
        wolf.genome.0[2] = 0.4; // sense_cells = 2 + 4 = 6
        let mut hare = creature(2, HARE, 5, 5);
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
        p.species.clear_initial_counts();
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
        let mut w = creature(0, WOLF, 10, 5);
        w.genome.0[5] = 0.99; // camouflage: the hare never detects it (prey rule)
        w.genome.0[2] = 0.7; // sense_cells 9
        w
    }

    #[test]
    fn hunt_contact_kills_and_eats() {
        // Guaranteed kill: Stalk → Chase (trigger at cheb ≤ 4) → contact → one roll.
        let mut sim = arena(|p| {
            p.kill_min = 1.0;
            p.kill_max = 1.0;
        });
        let wolf = put(&mut sim, stealthy_wolf(), 0.9);
        let mut hare = creature(0, HARE, 11, 5);
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
        assert_eq!(w.kills_by_species[HARE.index()], 1);
        assert!(w.last_kill.map(|k| k.0) == Some(hare));
        assert!(sim.events.iter().any(|e| e.kind == EventKind::DeathPredation && e.subject == Some(hare)));
        assert_eq!(sim.deaths.hunt_kills[WOLF.index()], 1);
        sim.step();
        let w = sim.creatures.get(wolf).unwrap();
        assert_eq!(w.attempts, 1, "no second roll while eating");
        assert_eq!(w.hunt_phase, HuntPhase::Eat, "eating lasts eat_hours");
    }

    /// A forced-failure arena with a stealthy wolf and a hidden hare, after one
    /// tick: the hunt has failed once and the prey is fleeing.
    fn failed_hunt_fixture() -> (Sim, CreatureId, CreatureId, u64) {
        let mut sim = arena(|p| {
            p.kill_min = 0.0;
            p.kill_max = 0.0;
            p.hunt_cooldown_hours = 100; // no second attempt during the flee
        });
        let wolf = put(&mut sim, stealthy_wolf(), 0.9);
        let mut hare = creature(0, HARE, 11, 5);
        hare.genome.0[5] = 0.0;
        let hare = put(&mut sim, hare, 0.3);
        still(&mut sim, hare);
        sim.step();
        let t = sim.time.tick;
        (sim, wolf, hare, t)
    }

    #[test]
    fn failed_hunt_forces_flee() {
        let (sim, wolf, hare, t) = failed_hunt_fixture();
        let w = sim.creatures.get(wolf).unwrap();
        let h = sim.creatures.get(hare).unwrap();
        assert!(h.alive);
        assert_eq!((w.kills, w.attempts), (0, 1));
        assert_eq!(w.goal, Goal::Patrol, "failed hunt idles one tick");
        assert_eq!(w.hunt_target, None);
        assert_eq!(w.hunt_cooldown_until, t + u64::from(sim.params.predation.hunt_cooldown_hours));
        assert_eq!(h.goal, Goal::Flee, "prey flees regardless of detection");
        assert_eq!(h.flee_until, t + u64::from(sim.params.predation.flee_ticks));
        assert!(h.threatened_by.is_some(), "the forced threat gives the away-vector");
        assert_eq!(h.chased, 1);
        assert_eq!(sim.deaths.hunt_attempts[WOLF.index()], 1);
    }

    #[test]
    fn forced_flee_lasts_and_escapes() {
        let (mut sim, wolf, hare, _t) = failed_hunt_fixture();
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
        let mut hare = creature(0, HARE, 10, 12);
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
        let chase_ticks = u64::from(sim.creatures.get(hare).unwrap().death.unwrap().chase_ticks);
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
        let mut hare = creature(0, HARE, 11, 5);
        hare.genome.0[5] = 0.0;
        let size = hare.genome.size();
        let hare = put(&mut sim, hare, 0.3);
        still(&mut sim, hare);
        sim.step();
        let pp = &sim.params.predation;
        let w = sim.creatures.get(wolf).unwrap();
        let h = sim.creatures.get(hare).unwrap();
        assert!(w.hunger < 0.9 - pp.hunger_per_kill(size) + 0.05, "hunger −= hunger_per_kill: {}", w.hunger);
        assert_eq!(w.eat_until, Some(sim.time.tick + u64::from(pp.eat_hours(size))));
        assert!((h.decay - pp.kill_consumes_decay).abs() < 1e-6, "carcass decay advanced by kill_consumes_decay");
        // The wolf eats on the carcass cell, then patrols.
        for _ in 0..=pp.eat_hours(size) {
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
        let k = sim.creatures.insert(carcass(HARE, 11, 5, 0.2));
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
        let k = sim.creatures.insert(carcass(FOX, 11, 5, 0.2));
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
