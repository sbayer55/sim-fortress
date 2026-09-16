//! Hunt and scavenge resolution, packs and kill outcomes.

use crate::sim::creatures::{
    Cause, Creature, CreatureId, CreatureStore, DeathTallies, Goal, HuntPhase,
};
use crate::sim::events::EventRing;
use crate::sim::genetics::{self, TickView};
use crate::sim::geom;
use crate::sim::lineage::Lineage;
use crate::sim::params::{PredationParams, Roster, SocialParams};
use crate::sim::predation::{self};
use crate::sim::rng::Rng;
use crate::sim::species::{Kind, SpeciesId};
use crate::sim::disease::{self, DiseaseState};
use crate::sim::params::DiseaseParams;
use crate::sim::time::Time;
use crate::sim::world::World;
use super::kill;
use super::threat::flee_target;

/// Facts about a predator/prey pair, snapshotted before either is mutated.
struct HuntSnap {
    pred_speed: f32,
    pred_aggr: f32,
    chase_start: Option<u64>,
    prey_species: SpeciesId,
    prey_size: f32,
    prey_speed: f32,
    px: usize,
    py: usize,
    cheb: usize,
    killer_label: String,
    pred_at: (usize, usize, SpeciesId),
    sick_bonus: f32,
}

/// Chase the current hunt target: keep it targeted, transition Stalk → Chase at
/// `chase_trigger_cheb`, and fail on timeout or when the prey leaves sense range.
pub(super) fn update_hunt_stalk(c: &mut Creature, view: &TickView, time: &Time, pp: &PredationParams, tallies: &mut DeathTallies) {
    let Some(prey_id) = c.hunt_target else { return };
    let Some(prey) = view.get(prey_id) else {
        fail_hunt(c, time, pp, tallies);
        return;
    };
    if geom::dist(c.x, c.y, prey.x, prey.y) > f32::from(c.genome.sense_cells()) {
        fail_hunt(c, time, pp, tallies);
        return;
    }
    c.target = Some((prey.x, prey.y));
    if c.hunt_phase == HuntPhase::Stalk && geom::cheb(c.x, c.y, prey.x, prey.y) <= pp.chase_trigger_cheb {
        c.hunt_phase = HuntPhase::Chase;
        c.chase_start_tick = Some(time.tick);
    }
    if c.hunt_phase == HuntPhase::Chase {
        if let Some(start) = c.chase_start_tick {
            if time.tick.saturating_sub(start) >= u64::from(pp.chase_max_ticks) {
                fail_hunt(c, time, pp, tallies);
            }
        }
    }
}

/// Record a failed hunt and re-arm for the next decision (FR4).
fn fail_hunt(c: &mut Creature, time: &Time, pp: &PredationParams, tallies: &mut DeathTallies) {
    c.attempts += 1;
    tallies.hunt_attempts[c.species.index()] += 1;
    c.hunt_cooldown_until = time.tick + u64::from(pp.hunt_cooldown_hours);
    c.hunt_phase = HuntPhase::Stalk;
    c.hunt_target = None;
    c.chase_start_tick = None;
    c.goal = Goal::Patrol;
    c.target = None;
    c.replan_at = time.tick + 1; // idle one tick
}

/// C8 FR4: a packmate that helped with a kill shares the meal. Deliberately *not*
/// `fail_hunt`: an attempt is not a failure, and the shared hunger is what makes
/// joining a pack worth the risk.
fn join_kill(c: &mut Creature, time: &Time, pp: &PredationParams, prey_size: f32, sp: &SocialParams) {
    c.hunger -= pp.hunger_per_kill(prey_size) * sp.pack_share;
    c.hunt_cooldown_until = time.tick + u64::from(pp.hunt_cooldown_hours);
    c.hunt_phase = HuntPhase::Stalk;
    c.hunt_target = None;
    c.chase_start_tick = None;
    c.goal = Goal::Patrol;
    c.target = None;
    c.replan_at = time.tick + 1;
}

/// Highest `preference × pack bonus / (1 + dist/4)` prey (preference 0 = never).
/// C8 FR4: a prey already hunted by a same-species packmate counts as detected and
/// scores a join bonus, which is what seeds pack hunting.
pub(super) fn pick_hunt_target(
    c: &Creature,
    candidates: &[CreatureId],
    view: &TickView,
    world: &World,
    roster: &Roster,
    pp: &PredationParams,
    sp: &SocialParams,
) -> Option<(CreatureId, (usize, usize))> {
    let mut ids: Vec<CreatureId> = candidates.to_vec();
    ids.sort_unstable();
    // Packmates seen this tick and the prey each is already hunting.
    let mut tally: Vec<(CreatureId, u32)> = Vec::new();
    for &id in &ids {
        if id == c.id {
            continue;
        }
        let Some(peer) = view.get(id) else { continue };
        if peer.species != c.species {
            continue;
        }
        let Some(t) = peer.hunt_target else { continue };
        match tally.binary_search_by_key(&t, |e| e.0) {
            Ok(i) => tally[i].1 += 1,
            Err(i) => tally.insert(i, (t, 1)),
        }
    }
    let sociality = c.genome.sociality();
    let mut best: Option<(CreatureId, (usize, usize), f32)> = None;
    for id in ids {
        let Some(peer) = view.get(id) else { continue };
        if roster.kind(peer.species) != Kind::Prey {
            continue;
        }
        let pref = roster.preference(c.species, peer.species);
        if pref <= 0.0 {
            continue;
        }
        let packed = tally.binary_search_by_key(&id, |e| e.0).map(|i| tally[i].1).unwrap_or(0);
        // A prey a packmate is chasing is known prey: no detection roll needed.
        if packed == 0 && !predation::can_detect_peer(c, peer.x, peer.y, peer.camouflage, peer.goal == Goal::Rest, world, pp) {
            continue;
        }
        let d = geom::dist(c.x, c.y, peer.x, peer.y);
        let join = 1.0 + sociality * sp.pack_join_bonus * crate::cast!(packed.min(3) => f32);
        let score = pref * join / (1.0 + d / 4.0);
        if best.is_none_or(|b| score > b.2) {
            best = Some((id, (peer.x, peer.y), score));
        }
    }
    best.map(|(id, pos, _)| (id, pos))
}

/// Nearest prey carcass within sense range (ties by id).
pub(super) fn pick_scavenge_target(c: &Creature, view: &TickView, roster: &Roster) -> Option<(CreatureId, (usize, usize))> {
    let r = f32::from(c.genome.sense_cells());
    let mut cs: Vec<&genetics::Carcass> = view.carcasses.iter().filter(|k| roster.kind(k.species) == Kind::Prey).collect();
    cs.sort_unstable_by_key(|k| k.id);
    let mut best: Option<(CreatureId, (usize, usize))> = None;
    let mut best_d = f32::INFINITY;
    for k in cs {
        let d = geom::dist(c.x, c.y, k.x, k.y);
        if d <= r && d < best_d {
            best_d = d;
            best = Some((k.id, (k.x, k.y)));
        }
    }
    best
}

/// Post-loop pass: resolve hunt contacts (single kill roll), the Eat phase and
/// the failure Flee (FR4).
#[allow(clippy::too_many_arguments)]
/// The prey enters Flee regardless of whether it had detected the predator: the
/// threat is forced in so the away-vector exists, and `mark_threats` retains it
/// until the flee timer expires.
fn force_flee(q: &mut Creature, world: &World, pp: &PredationParams, time: &Time, pred_at: (usize, usize, SpeciesId)) {
    if q.alive {
        if q.goal != Goal::Flee {
            q.chased += 1;
            q.threats_by_species[pred_at.2.index()] += 1;
        }
        q.goal = Goal::Flee;
        q.flee_until = time.tick + u64::from(pp.flee_ticks);
        q.threatened_by = Some(pred_at);
        q.target = flee_target(q, world, pp);
        q.replan_at = time.tick + 1;
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn hunt_contacts(
    store: &mut CreatureStore,
    world: &mut World,
    events: &mut EventRing,
    time: &Time,
    roster: &Roster,
    pp: &PredationParams,
    dp: &DiseaseParams,
    sp: &SocialParams,
    rng: &mut Rng,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
    dstate: &mut DiseaseState,
    drng: &mut Rng,
) {
    let mut hunters: Vec<(CreatureId, CreatureId)> = store
        .living()
        .filter(|c| c.goal == Goal::Hunt && c.hunt_phase != HuntPhase::Eat)
        .filter_map(|c| c.hunt_target.map(|t| (c.id, t)))
        .collect();
    hunters.sort_unstable();

    for (pred_id, prey_id) in hunters {
        let Some(snap) = hunt_snapshot(store, pred_id, prey_id, roster, dp) else { continue };
        if snap.cheb > pp.catch_distance_cheb {
            continue;
        }
        // C8 FR4: a pack converging on one prey kills more reliably, and the meal
        // is shared with the packmates within `pack_share_cheb` of the kill.
        let participants = pack_participants(store, pred_id, snap.pred_at.2, prey_id, snap.px, snap.py, sp);
        let extra = participants.len().min(3);
        // C7 FR6: a sick prey is easier to catch.
        let chance = (pp.kill_chance(snap.pred_speed, snap.prey_speed, snap.pred_aggr, snap.prey_size)
            + snap.sick_bonus
            + sp.pack_kill_bonus * crate::cast!(extra => f32))
        .clamp(pp.kill_min, pp.kill_max);
        let chase_ticks = snap.chase_start.map_or(0, |s| crate::cast!(time.tick.saturating_sub(s) => u16));
        if rng.chance(chance) {
            resolve_kill(store, world, events, time, roster, pp, dp, sp, tallies, lineage, dstate, drng, pred_id, prey_id, &snap, participants, extra, chase_ticks);
        } else {
            resolve_miss(store, world, time, pp, tallies, pred_id, prey_id, snap.pred_at);
        }
    }
}

/// Snapshot the pair, or `None` when the hunt is no longer valid.
fn hunt_snapshot(store: &CreatureStore, pred_id: CreatureId, prey_id: CreatureId, roster: &Roster, dp: &DiseaseParams) -> Option<HuntSnap> {
    // Snapshot the facts we need before mutating either creature.
    let (p, q) = (store.get(pred_id)?, store.get(prey_id)?);
    if !(p.alive && q.alive && p.goal == Goal::Hunt && p.hunt_target == Some(prey_id)) {
        return None;
    }
    Some(HuntSnap {
        pred_speed: p.genome.speed(),
        pred_aggr: p.genome.aggression(),
        chase_start: p.chase_start_tick,
        prey_species: q.species,
        prey_size: q.genome.size(),
        prey_speed: q.genome.speed(),
        px: q.x,
        py: q.y,
        cheb: geom::cheb(p.x, p.y, q.x, q.y),
        killer_label: p.label(roster),
        pred_at: (p.x, p.y, p.species),
        sick_bonus: disease::effects(q, dp).kill_bonus,
    })
}

/// Packmates hunting the same prey within sharing range of the kill.
fn pack_participants(
    store: &CreatureStore,
    pred_id: CreatureId,
    pred_species: SpeciesId,
    prey_id: CreatureId,
    px: usize,
    py: usize,
    sp: &SocialParams,
) -> Vec<CreatureId> {
    store
        .living()
        .filter(|o| {
            o.id != pred_id
                && o.species == pred_species
                && o.goal == Goal::Hunt
                && o.hunt_phase != HuntPhase::Eat
                && o.hunt_target == Some(prey_id)
                && geom::cheb(o.x, o.y, px, py) <= sp.pack_share_cheb
        })
        .map(|o| o.id)
        .collect()
}

/// A successful catch: kill, credit the hunter, share with the pack, feed.
#[allow(clippy::too_many_arguments)]
fn resolve_kill(
    store: &mut CreatureStore,
    world: &mut World,
    events: &mut EventRing,
    time: &Time,
    roster: &Roster,
    pp: &PredationParams,
    dp: &DiseaseParams,
    sp: &SocialParams,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
    dstate: &mut DiseaseState,
    drng: &mut Rng,
    pred_id: CreatureId,
    prey_id: CreatureId,
    snap: &HuntSnap,
    participants: Vec<CreatureId>,
    extra: usize,
    chase_ticks: u16,
) {
    let (px, py) = (snap.px, snap.py);
    let prey_species = snap.prey_species;
    let prey_size = snap.prey_size;
    if let Some(prey) = store.get_mut(prey_id) {
        if prey.alive {
            kill(prey, Cause::Predation, world, events, time, roster, tallies, lineage, Some(pred_id), chase_ticks, Some(&snap.killer_label));
        }
    }
    if let Some(p) = store.get_mut(pred_id) {
        p.kills += 1;
        p.kills_by_species[prey_species.index()] += 1;
        p.attempts += 1;
        tallies.hunt_attempts[p.species.index()] += 1;
        tallies.hunt_kills[p.species.index()] += 1;
        p.chase_stats.0 += u32::from(chase_ticks);
        if u32::from(chase_ticks) > p.chase_stats.1 {
            p.chase_stats.1 = u32::from(chase_ticks);
            p.chase_longest_year = time.year();
        }
        p.last_kill = Some((prey_id, crate::cast!(time.day_index() => u32), crate::cast!(world.region_index(px, py) => u8)));
        p.hunt_cooldown_until = time.tick + u64::from(pp.hunt_cooldown_hours);
        p.hunt_phase = HuntPhase::Eat;
        p.hunt_target = None;
        p.chase_start_tick = None;
        p.eat_until = Some(time.tick + u64::from(pp.eat_hours(prey_size)));
        p.target = Some((px, py));
        // FR4: `hunger −= hunger_per_kill` — allowed to go negative, so a
        // big kill extends the satiation period and bounds the hunt rate.
        p.hunger -= pp.hunger_per_kill(prey_size);
    }
    // C8 FR4: the pack shares the kill. Rewards are not attempts, so this
    // never goes through `fail_hunt`; the kill itself is counted once.
    for pid in participants {
        if let Some(o) = store.get_mut(pid) {
            join_kill(o, time, pp, prey_size, sp);
        }
    }
    if let Some(carcass) = store.get_mut(prey_id) {
        let eaten = pp.kill_consumes_decay * (1.0 + sp.pack_share * crate::cast!(extra => f32));
        carcass.decay = (carcass.decay + eaten).min(1.0);
    }
    // C7 FR7/FR8b: the meal carries parasites, infection or a spillover.
    disease::on_eat(store, pred_id, prey_id, world, events, time, roster, dp, dstate, drng);
}

/// A missed catch: the predator re-plans, the prey is forced to flee.
#[allow(clippy::too_many_arguments)]
fn resolve_miss(
    store: &mut CreatureStore,
    world: &World,
    time: &Time,
    pp: &PredationParams,
    tallies: &mut DeathTallies,
    pred_id: CreatureId,
    prey_id: CreatureId,
    pred_at: (usize, usize, SpeciesId),
) {
    if let Some(p) = store.get_mut(pred_id) {
        fail_hunt(p, time, pp, tallies);
    }
    // The prey enters Flee regardless of whether it had detected the
    // predator: the threat is forced in so the away-vector exists, and
    // `mark_threats` retains it until the flee timer expires.
    if let Some(q) = store.get_mut(prey_id) {
        force_flee(q, world, pp, time, pred_at);
    }
}

/// Post-loop pass: a predator adjacent to its scavenge target eats once (FR3).
#[allow(clippy::too_many_arguments)]
pub(super) fn scavenge_contacts(store: &mut CreatureStore, world: &World, events: &mut EventRing, time: &Time, roster: &Roster, pp: &PredationParams, dp: &DiseaseParams, dstate: &mut DiseaseState, drng: &mut Rng) {
    let scavengers: Vec<CreatureId> = store
        .living()
        .filter(|c| c.goal == Goal::Scavenge && c.scavenge_target.is_some())
        .map(|c| c.id)
        .collect();
    let mut scavengers = scavengers;
    scavengers.sort_unstable();

    for id in scavengers {
        // Snapshot: (carcass id, carcass x, carcass y, carcass decay, valid prey carcass).
        let snap = {
            let Some(c) = store.get(id) else { continue };
            let Some(t) = c.scavenge_target else { continue };
            match store.get(t) {
                Some(k) if !k.alive && roster.kind(k.species) == Kind::Prey => Some((t, k.x, k.y, k.decay)),
                _ => None,
            }
        };
        let Some((carcass_id, tx, ty, decay)) = snap else {
            if let Some(c) = store.get_mut(id) {
                c.scavenge_target = None;
            }
            continue;
        };
        let adjacent = store.get(id).is_some_and(|c| geom::cheb(c.x, c.y, tx, ty) <= 1);
        if !adjacent {
            continue;
        }
        let nutrition = pp.carcass_nutrition * (1.0 - decay);
        if let Some(c) = store.get_mut(id) {
            c.hunger = (c.hunger - nutrition).max(0.0);
            c.scavenge_target = None;
            c.goal = Goal::Patrol;
            c.target = None;
            c.replan_at = time.tick + 1;
        }
        if let Some(k) = store.get_mut(carcass_id) {
            k.decay = (k.decay + pp.scavenge_consumes_decay).min(1.0);
        }
        disease::on_eat(store, id, carcass_id, world, events, time, roster, dp, dstate, drng);
    }
}
