//! The predation threat scan and the flee response.

use crate::sim::creatures::{
    Creature, CreatureId, CreatureStore, Goal, HuntPhase,
};
use crate::sim::geom;
use crate::sim::params::{PredationParams, SocialParams};
use crate::sim::predation::MAX_SENSE_CELLS;
use crate::sim::spatial::SpatialIndex;
use crate::sim::species::{Kind, SpeciesId};
use crate::sim::world::World;

/// FR5: mark every prey threatened by a living predator, predator-first (ids
/// ascending), using a bucket-bounded `spatial.within(pred, MAX_SENSE_CELLS)`
/// query. Also computes `predation_risk` for the S03 Condition bar.
///
/// Decision: a prey is "threatened" when *it* detects the predator (the FR2
/// prey rule), matching FR4 ("the prey detecting the predator starts its Flee")
/// and the FR12 danger line — and the predator is a danger: it is hunting this
/// prey, or it is hungry enough to hunt and within `chase_trigger_cheb`. A
/// satiated predator ambling past is seen (S02d, the danger line) but not fled
/// from; without that bound every prey near any predator flees on every tick,
/// exhausts itself (flee steps cost double) and dies of thirst in forced rest. A failed kill roll forces a Flee regardless (FR4); that
/// forced threat is retained here until the flee timer expires so the
/// away-vector survives ticks on which the prey cannot sense the predator.
// `camouflage < sense` is intentional: two different 0..=1 traits.
#[allow(clippy::suspicious_operation_groupings)]
/// Per-prey facts accumulated while scanning predators.
struct PreySnap {
    id: CreatureId,
    rest: bool,
    sense: f32,
    sense_cells: u16,
    count: u32,
    dist: f32,
    pos: (usize, usize),
    /// The predator species that threatens this prey (set with `dist`).
    species: SpeciesId,
    /// The prey's own species and sociality, for the alarm pass (C8 FR3).
    own: SpeciesId,
    sociality: f32,
}

/// The predator facts the prey rule needs (no `Creature` clones per tick).
struct Pred {
    id: CreatureId,
    x: usize,
    y: usize,
    species: SpeciesId,
    camouflage: f32,
    hunting: Option<CreatureId>,
    /// Hungry enough to hunt: a satiated predator at four cells is no danger.
    hungry: bool,
}

pub(super) fn mark_threats(store: &mut CreatureStore, spatial: &SpatialIndex, world: &World, tick: u64, pp: &PredationParams, sp: &SocialParams) {
    let mut preds = build_preds(store, pp);
    preds.sort_unstable_by_key(|p| p.id);
    let mut prey = build_prey(store);
    prey.sort_unstable_by_key(|p| p.id);

    scan_prey(&preds, &mut prey, spatial, pp);
    propagate_alarms(&mut prey, spatial, sp);
    apply_threats(store, &prey, world, tick);
}

/// Snapshot every predator's threat-relevant facts.
fn build_preds(store: &CreatureStore, pp: &PredationParams) -> Vec<Pred> {
    store
        .living()
        .filter(|c| c.species.kind() == Kind::Predator)
        .map(|c| Pred {
            id: c.id,
            x: c.x,
            y: c.y,
            species: c.species,
            camouflage: c.genome.camouflage(),
            hunting: if c.goal == Goal::Hunt && c.hunt_phase != HuntPhase::Eat { c.hunt_target } else { None },
            hungry: c.hunger > pp.hunt_hunger_min,
        })
        .collect()
}

/// Snapshot every prey's detection-relevant facts.
fn build_prey(store: &CreatureStore) -> Vec<PreySnap> {
    store
        .living()
        .filter(|c| c.species.kind() == Kind::Prey)
        .map(|c| PreySnap {
            id: c.id,
            rest: c.goal == Goal::Rest,
            sense: c.genome.sense(),
            sense_cells: c.genome.sense_cells(),
            count: 0,
            dist: f32::INFINITY,
            pos: (0, 0),
            species: SpeciesId::Vole,
            own: c.species,
            sociality: c.genome.sociality(),
        })
        .collect()
}

/// Detection pass: for each predator, mark the prey that see it as threatened.
// `camouflage < sense` is intentional: two different 0..=1 traits.
#[allow(clippy::suspicious_operation_groupings)]
fn scan_prey(preds: &[Pred], prey: &mut [PreySnap], spatial: &SpatialIndex, pp: &PredationParams) {
    for pred in preds {
        spatial.for_each_within(pred.x, pred.y, MAX_SENSE_CELLS, |prey_id, qx, qy| {
            let Ok(i) = prey.binary_search_by_key(&prey_id, |p| p.id) else { return };
            let p = &mut prey[i];
            let d = geom::dist(pred.x, pred.y, qx, qy);
            let range = if p.rest { f32::from(p.sense_cells) * pp.rest_detect_factor } else { f32::from(p.sense_cells) };
            let in_range = d <= f32::from(p.sense_cells);
            let detects = d <= range && pred.camouflage < p.sense;
            if in_range {
                p.count += 1;
            }
            let danger = pred.hunting == Some(prey_id)
                || (pred.hungry && geom::cheb(pred.x, pred.y, qx, qy) <= pp.chase_trigger_cheb);
            if detects && danger && d < pp.flee_distance && d < p.dist {
                p.dist = d;
                p.pos = (pred.x, pred.y);
                p.species = pred.species;
            }
        });
    }
}

/// C8 FR3: alarm propagation from already-threatened prey to nearby kin.
fn propagate_alarms(prey: &mut [PreySnap], spatial: &SpatialIndex, sp: &SocialParams) {
    let reach = crate::cast!(sp.alarm_cells.max(0.0) => u16);
    if reach == 0 {
        return;
    }
    let mut alarms: Vec<(CreatureId, (usize, usize), SpeciesId)> = Vec::new();
    for p in prey.iter() {
        if p.dist.is_infinite() {
            continue;
        }
        let (sx, sy) = p.pos;
        let (sender, threat_species) = (p.own, p.species);
        spatial.for_each_within(sx, sy, reach, |other_id, qx, qy| {
            let Ok(i) = prey.binary_search_by_key(&other_id, |q| q.id) else { return };
            let q = &prey[i];
            if !q.dist.is_infinite() || q.own != sender {
                return;
            }
            // The alarm reaches `q` only if `q` is social enough to heed it.
            if geom::dist(sx, sy, qx, qy) <= q.sociality * sp.alarm_cells {
                alarms.push((other_id, (sx, sy), threat_species));
            }
        });
    }
    for (id, pos, threat_species) in alarms {
        if let Ok(i) = prey.binary_search_by_key(&id, |q| q.id) {
            let q = &mut prey[i];
            if q.dist.is_infinite() {
                q.pos = pos;
                q.species = threat_species;
                q.dist = 0.0; // alerted, distance unknown: not a detection
            }
        }
    }
}

/// Write the scan results (and predation risk) back onto the prey.
fn apply_threats(store: &mut CreatureStore, prey: &[PreySnap], world: &World, tick: u64) {
    for c in store.living_mut() {
        if c.species.kind() != Kind::Prey {
            continue;
        }
        // A forced flee (FR4) keeps its last known threat until the timer ends.
        let keep_forced = c.goal == Goal::Flee && tick < c.flee_until && c.threatened_by.is_some();
        if let Ok(i) = prey.binary_search_by_key(&c.id, |p| p.id) {
            let p = &prey[i];
            if p.dist < f32::INFINITY {
                c.threatened_by = Some((p.pos.0, p.pos.1, p.species));
            } else if !keep_forced {
                c.threatened_by = None;
            }
            let pressure = world.cell(c.x, c.y).pred_pressure;
            c.predation_risk = (0.5 * pressure + 0.5 * (crate::cast!(p.count => f32)) / 3.0).min(1.0);
        } else {
            if !keep_forced {
                c.threatened_by = None;
            }
            c.predation_risk = (0.5 * world.cell(c.x, c.y).pred_pressure).min(1.0);
        }
    }
}

/// The cell `flee_distance` away from the threatening predator (FR5).
pub(super) fn flee_target(c: &Creature, world: &World, pp: &PredationParams) -> Option<(usize, usize)> {
    let (px, py, _) = c.threatened_by?;
    let dx = crate::cast!(c.x => i32) - crate::cast!(px => i32);
    let dy = crate::cast!(c.y => i32) - crate::cast!(py => i32);
    if dx == 0 && dy == 0 {
        return Some((c.x, c.y));
    }
    let k = crate::cast!(pp.flee_distance.ceil() => i32);
    let nx = crate::cast!((crate::cast!(c.x => i32) + dx.signum() * k).clamp(0, crate::cast!(world.width => i32) - 1) => usize);
    let ny = crate::cast!((crate::cast!(c.y => i32) + dy.signum() * k).clamp(0, crate::cast!(world.height => i32) - 1) => usize);
    Some((nx, ny))
}
