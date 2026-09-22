//! Goal selection with hysteresis: replanning for prey and predators.

use crate::sim::creatures::{
    Creature, DeathTallies, Goal, HuntPhase, RestReason,
};
use crate::sim::events::EventRing;
use crate::sim::genetics::{self, TickView};
use crate::sim::geom;
use crate::sim::lineage::Lineage;
use crate::sim::params::{CreaturesParams, DietParams, EcologyParams, GeneticsParams, PredationParams, Roster, SocialParams, TerritoryParams};
use crate::sim::rng::Rng;
use crate::sim::spatial::SpatialIndex;
use crate::sim::species::Kind;
use crate::sim::disease::{self};
use crate::sim::params::DiseaseParams;
use crate::sim::time::Time;
use crate::sim::world::World;
use super::needs;
use super::perception::{Kin, Perception, perceive};
use super::movement::{move_toward, wander};
use super::vitals::act;
use super::death::{maybe_die, pressure};
use super::threat::{flee_target, wary_target};
use super::hunt::{pick_hunt_target, pick_scavenge_target, update_hunt_stalk};
use super::Ledgers;
use super::territory::{self, Scent};

#[allow(clippy::too_many_arguments)]
pub(super) fn update_one(
    c: &mut Creature,
    spatial: &SpatialIndex,
    world: &mut World,
    events: &mut EventRing,
    time: &Time,
    roster: &Roster,
    cp: &CreaturesParams,
    ep: &EcologyParams,
    gp: &GeneticsParams,
    pp: &PredationParams,
    dp: &DiseaseParams,
    sp: &SocialParams,
    diet: &DietParams,
    tp: &TerritoryParams,
    view: &TickView,
    rng: &mut Rng,
    ledgers: &mut Ledgers<'_>,
    lineage: &mut Lineage,
) {
    let fx = disease::effects(c, dp);
    // FR5: Flee pre-empts every goal for prey; FR5b adds the wary tier below it.
    // C5 FR13: an evicted predator flees the contest winner the same way.
    if roster.kind(c.species) == Kind::Prey {
        preempt_prey(c, world, time, pp, ledgers.tallies);
    } else {
        territory::preempt_predator(c, world, time, pp);
    }

    // Eat phase: the predator walks onto the carcass cell (`target`, set at the
    // kill) and consumes it there until done.
    if c.goal == Goal::Hunt && c.hunt_phase == HuntPhase::Eat {
        if c.eat_until.is_none_or(|t| time.tick >= t) {
            c.hunt_phase = HuntPhase::Stalk;
            c.hunt_target = None;
            c.eat_until = None;
            c.target = None;
            c.goal = Goal::Patrol;
            c.replan_at = time.tick;
        } else {
            c.replan_at = c.eat_until.unwrap_or(time.tick) + 1;
        }
    }

    // Goal satisfied → replan now.
    if goal_satisfied(c, time, roster, pp) {
        c.replan_at = time.tick;
    }
    // Replan when due.
    if time.tick >= c.replan_at {
        replan(c, spatial, world, time, roster, cp, gp, pp, view, rng, dp, fx.rest_energy, sp, diet, tp);
    }
    // Track the current prey target while hunting (Stalk → Chase).
    if c.goal == Goal::Hunt && c.hunt_phase != HuntPhase::Eat {
        update_hunt_stalk(c, view, time, pp, ledgers);
    }
    // C5 FR13: track the intruder while challenging.
    if c.goal == Goal::Challenge {
        territory::update_challenge(c, view, world, time, tp);
    }
    // Move toward the target.
    move_toward(c, world, time, cp, pp, fx.speed_factor);
    // Act on the goal at the current location.
    act(c, world, events, time, roster, cp, dp, diet, rng);
    // Needs and hp.
    needs(c, world, time, cp, ep, gp, dp, fx.hunger_factor);
    // Death.
    maybe_die(c, world, events, time, roster, ledgers.tallies, lineage, dp);
    // Pressure and parasite shedding.
    pressure(c, world, roster, cp, tp);
    disease::parasite_shed(c, world, dp);
}

/// C5 FR4/FR5/FR5b: the prey's two avoidance tiers, both running before
/// `replan` so they pre-empt the ordinary goal. Flee wins outright; Wary, the
/// low-exertion tier, yields only to a forced rest. Each keeps its away-vector
/// alive for the length of its own timer.
fn preempt_prey(c: &mut Creature, world: &World, time: &Time, pp: &PredationParams, tallies: &mut DeathTallies) {
    let threatened = c.threatened_by.is_some();
    let flee_continues = threatened
        && time.tick < c.flee_until
        && c.threatened_by.is_some_and(|(px, py, _)| geom::dist(c.x, c.y, px, py) < pp.flee_distance);
    if c.goal == Goal::Flee && !flee_continues {
        // The threat is gone, the prey fled far enough, or the timer ran out.
        c.escaped += 1;
        c.goal = Goal::Wander;
        c.flee_until = 0;
        c.target = None;
        c.replan_at = time.tick;
    } else if threatened {
        if c.goal != Goal::Flee {
            c.goal = Goal::Flee;
            c.flee_until = time.tick + u64::from(pp.flee_ticks);
            c.chased += 1;
            if let Some((_, _, sp)) = c.threatened_by {
                c.threats_by_species[sp.index()] += 1;
            }
        }
        c.target = flee_target(c, world, pp);
        c.replan_at = time.tick + 1;
    }
    if c.goal == Goal::Flee {
        return;
    }
    // C5 FR5b: Wary is the second tier — the prey gives a predator that is not
    // hunting it some room, at a fraction of the flee exertion. It pre-empts
    // every goal except a forced rest (energy ≤ 0).
    let forced_rest = c.goal == Goal::Rest && c.rest_reason == Some(RestReason::Forced);
    let wary_continues = c.goal == Goal::Wary
        && time.tick < c.wary_until
        && c.wary_by.is_some_and(|(px, py, _)| geom::dist(c.x, c.y, px, py) <= pp.wary_distance * pp.wary_release_factor);
    if c.goal == Goal::Wary && !wary_continues {
        c.goal = Goal::Wander;
        c.wary_until = 0;
        c.target = None;
        c.replan_at = time.tick;
    } else if c.wary_by.is_some() && !forced_rest {
        if c.goal != Goal::Wary {
            enter_wary(c, world, time.tick, pp, tallies);
        }
        c.target = wary_target(c, world, pp);
        c.replan_at = time.tick + 1;
    }
}

/// Enter the wary tier: set the goal, arm the timer and count the encounter.
/// The per-region-per-day tally is flushed into one Wary event by `flush_wary`.
fn enter_wary(c: &mut Creature, world: &World, tick: u64, pp: &PredationParams, tallies: &mut DeathTallies) {
    c.goal = Goal::Wary;
    c.wary_until = tick + u64::from(pp.wary_ticks);
    c.wary_count += 1;
    if let Some((_, _, pred)) = c.wary_by {
        let ri = crate::cast!(world.region_index(c.x, c.y) => u8);
        *tallies.wary_today.entry((ri, c.species, pred)).or_default() += 1;
    }
}

fn goal_satisfied(c: &Creature, time: &Time, roster: &Roster, pp: &PredationParams) -> bool {
    match c.goal {
        Goal::Drink => c.thirst <= 0.1,
        Goal::Graze => c.hunger <= 0.2,
        Goal::Rest => match c.rest_reason {
            Some(RestReason::Forced) => c.energy >= 0.3,
            Some(RestReason::Energy) => c.energy >= 0.9,
            // Diurnal wake at sunrise; nocturnal wake at nightfall.
            Some(RestReason::Night) => {
                if roster.is_nocturnal(c.species) { time.is_night() } else { !time.is_night() }
            }
            None => true,
        },
        Goal::Wander => c.target.is_none(),
        // Mated (cooldown just set) or the partner is gone.
        Goal::Mate => c.mate_id.is_none() || c.cooldown_until > time.tick,
        Goal::Flee => c.threatened_by.is_none()
            || time.tick >= c.flee_until
            || c.threatened_by.is_none_or(|(px, py, _)| geom::dist(c.x, c.y, px, py) >= pp.flee_distance),
        Goal::Wary => c.wary_by.is_none()
            || time.tick >= c.wary_until
            || c.wary_by.is_none_or(|(px, py, _)| geom::dist(c.x, c.y, px, py) > pp.wary_distance * pp.wary_release_factor),
        Goal::Migrate => time.tick >= c.migrate_until || c.migrate_target.is_none(),
        // C5 FR13: the timer here; `update_challenge` ends it on the intruder.
        Goal::Challenge => c.challenge_target.is_none() || time.tick >= c.challenge_until,
        // Hunt / Scavenge / Patrol are ended by their dedicated passes.
        Goal::Hunt | Goal::Scavenge | Goal::Patrol => false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replan(
    c: &mut Creature,
    spatial: &SpatialIndex,
    world: &World,
    time: &Time,
    roster: &Roster,
    cp: &CreaturesParams,
    gp: &GeneticsParams,
    pp: &PredationParams,
    view: &TickView,
    rng: &mut Rng,
    dp: &DiseaseParams,
    rest_energy: f32,
    sp: &SocialParams,
    diet: &DietParams,
    tp: &TerritoryParams,
) {
    let tick = time.tick;
    let next = tick + cp.replan_ticks;
    // A mate target only survives while the goal is Mate; a pregnant female
    // keeps the father's id until delivery.
    if c.pregnant_due.is_none() {
        c.mate_id = None;
    }

    // Forced rest overrides every other need (FR5).
    if c.energy <= 0.0 {
        c.goal = Goal::Rest;
        c.rest_reason = Some(RestReason::Forced);
        c.target = None;
        c.replan_at = next;
        return;
    }

    // An active migration overrides Wander/Graze/Patrol (FR7).
    if c.goal == Goal::Migrate && time.tick < c.migrate_until && c.migrate_target.is_some() {
        c.target = c.migrate_target;
        c.replan_at = next;
        return;
    }

    if roster.kind(c.species) == Kind::Predator {
        replan_predator(c, spatial, world, time, roster, cp, gp, pp, view, rng, dp, rest_energy, sp, diet, tp);
    } else {
        replan_prey(c, spatial, world, time, roster, cp, gp, pp, view, rng, dp, rest_energy, sp, diet);
    }
}

#[allow(clippy::too_many_arguments)]
fn replan_prey(
    c: &mut Creature,
    spatial: &SpatialIndex,
    world: &World,
    time: &Time,
    roster: &Roster,
    cp: &CreaturesParams,
    gp: &GeneticsParams,
    pp: &PredationParams,
    view: &TickView,
    rng: &mut Rng,
    dp: &DiseaseParams,
    rest_energy: f32,
    sp: &SocialParams,
    diet: &DietParams,
) {
    let next = time.tick + cp.replan_ticks;
    let (p, kin) = perceive(c, spatial, world, cp, view, sp, diet, &Scent::NONE);
    c.kin_nearby = kin.count;

    // 1. Drink (enter thirst > 0.6, stay while thirst > 0.1).
    if c.thirst > 0.6 || (c.goal == Goal::Drink && c.thirst > 0.1) {
        if let Some(water) = p.nearest_water.or(c.last_water) {
            c.goal = Goal::Drink;
            c.rest_reason = None;
            c.target = Some(water);
        } else {
            wander(c, world, time, view, gp, sp, rng, Some(kin));
        }
        c.replan_at = next;
        return;
    }

    // 2. Graze (enter hunger > 0.5, stay while hunger > 0.2 — hysteresis).
    if c.hunger > 0.5 || (c.goal == Goal::Graze && c.hunger > 0.2) {
        c.goal = Goal::Graze;
        c.rest_reason = None;
        // Diet breadth: the current cell counts for what this animal can
        // digest of it, the same rule `perceive` scored with.
        let cur = world.cell(c.x, c.y);
        let cur_edible = cur.vegetation * diet.edibility(cur.terrain, c.genome.diet_breadth());
        if let Some((cell, score)) = p.best_graze {
            let cur_score = if cur_edible >= cp.graze_min_vegetation { cur_edible } else { 0.0 };
            c.target = if cur_score >= score * 0.9 && cur_edible >= cp.graze_min_vegetation {
                None // graze in place
            } else {
                Some(cell)
            };
        } else if cur_edible >= cp.graze_min_vegetation {
            c.target = None;
        } else {
            wander(c, world, time, view, gp, sp, rng, Some(kin));
            c.goal = Goal::Graze;
        }
        c.replan_at = next;
        return;
    }

    // 3. Rest (enter energy < rest threshold or night; stay until satisfied).
    // The threshold is 0.25, raised for the sick (C7 FR6).
    let rest_enter = c.energy < rest_energy || time.is_night();
    let rest_stay = c.goal == Goal::Rest && !goal_satisfied(c, time, roster, pp);
    if rest_enter || rest_stay {
        if c.goal != Goal::Rest {
            c.rest_reason = Some(if c.energy < rest_energy { RestReason::Energy } else { RestReason::Night });
        }
        c.goal = Goal::Rest;
        c.target = p.nearest_den;
        c.replan_at = next;
        return;
    }

    // 4. Mate (C4 FR2): eligible adults seek the nearest eligible partner.
    if view.cap_ok && genetics::eligible(c, time, world, roster, gp, dp) {
        if let Some((mate, pos)) = genetics::pick_mate(c, &p.creatures, view) {
            c.goal = Goal::Mate;
            c.rest_reason = None;
            c.mate_id = Some(mate);
            c.target = Some(pos);
            c.replan_at = next;
            return;
        }
    }

    // 5. Wander.
    wander(c, world, time, view, gp, sp, rng, Some(kin));
    c.replan_at = next;
}

#[allow(clippy::too_many_arguments)]
fn replan_predator(
    c: &mut Creature,
    spatial: &SpatialIndex,
    world: &World,
    time: &Time,
    roster: &Roster,
    cp: &CreaturesParams,
    gp: &GeneticsParams,
    pp: &PredationParams,
    view: &TickView,
    rng: &mut Rng,
    dp: &DiseaseParams,
    rest_energy: f32,
    sp: &SocialParams,
    diet: &DietParams,
    tp: &TerritoryParams,
) {
    let next = time.tick + cp.replan_ticks;
    // C5 FR13: what this predator makes of its species' scent this replan.
    let scent = territory::scent_view(c, world, tp, sp);
    let (p, kin) = perceive(c, spatial, world, cp, view, sp, diet, &scent);
    c.kin_nearby = kin.count;

    // 1. Drink (thirst > 0.6).
    if c.thirst > 0.6 {
        if let Some(water) = p.nearest_water.or(c.last_water) {
            c.goal = Goal::Drink;
            c.rest_reason = None;
            c.target = Some(water);
        } else {
            patrol(c, world, time, view, gp, sp, rng, &p, Some(kin));
        }
        c.replan_at = next;
        return;
    }

    // 2. Hunt (hunger > hunt_hunger_min and a detectable prey in range).
    if c.hunger > pp.hunt_hunger_min && time.tick >= c.hunt_cooldown_until {
        if let Some((prey, pos)) = pick_hunt_target(c, &p.creatures, view, world, roster, pp, sp, &scent) {
            c.goal = Goal::Hunt;
            c.rest_reason = None;
            c.hunt_phase = HuntPhase::Stalk;
            c.hunt_target = Some(prey);
            c.chase_start_tick = None;
            c.target = Some(pos);
            c.replan_at = next;
            return;
        }
    }

    // 3. Scavenge (hunger > scavenge_hunger_min and a prey carcass in range).
    if c.hunger > pp.scavenge_hunger_min {
        if let Some((carcass, pos)) = pick_scavenge_target(c, view, roster) {
            c.goal = Goal::Scavenge;
            c.rest_reason = None;
            c.scavenge_target = Some(carcass);
            c.target = Some(pos);
            c.replan_at = next;
            return;
        }
    }

    // 3b. Challenge (C5 FR13): a solitary resident walks at a same-species
    // adult standing on its ground. Hunger has had its say above.
    if let Some((intruder, pos)) = territory::pick_intruder(c, &p.creatures, view, world, time, tp, sp) {
        if c.goal != Goal::Challenge || c.challenge_target != Some(intruder) {
            c.challenge_until = time.tick + u64::from(tp.challenge_ticks);
        }
        c.goal = Goal::Challenge;
        c.rest_reason = None;
        c.challenge_target = Some(intruder);
        c.target = Some(pos);
        c.replan_at = next;
        return;
    }

    // 4. Mate (C4 rules; the vegetation gate does not apply to predators).
    if view.cap_ok && genetics::eligible(c, time, world, roster, gp, dp) {
        if let Some((mate, pos)) = genetics::pick_mate(c, &p.creatures, view) {
            c.goal = Goal::Mate;
            c.rest_reason = None;
            c.mate_id = Some(mate);
            c.target = Some(pos);
            c.replan_at = next;
            return;
        }
    }

    // 5. Rest (energy < rest threshold, or night for diurnal / day for nocturnal).
    let rest_enter = c.energy < rest_energy
        || if roster.is_nocturnal(c.species) { !time.is_night() } else { time.is_night() };
    let rest_stay = c.goal == Goal::Rest && !goal_satisfied(c, time, roster, pp);
    if rest_enter || rest_stay {
        if c.goal != Goal::Rest {
            c.rest_reason = Some(if c.energy < rest_energy { RestReason::Energy } else { RestReason::Night });
        }
        c.goal = Goal::Rest;
        c.target = None; // predators rest in place
        c.replan_at = next;
        return;
    }

    // 6. Patrol (wander biased toward the highest prey_pressure cell seen).
    patrol(c, world, time, view, gp, sp, rng, &p, Some(kin));
    c.replan_at = next;
}

/// Patrol (FR3): a wander *biased* toward the highest `prey_pressure` cell seen —
/// half of the replans head there when it is more than two cells away; the rest
/// wander, so a satiated predator never camps on a hotspot (a water hole or den)
/// and denies the prey their drink.
#[allow(clippy::too_many_arguments)]
fn patrol(
    c: &mut Creature,
    world: &World,
    time: &Time,
    view: &TickView,
    gp: &GeneticsParams,
    sp: &SocialParams,
    rng: &mut Rng,
    p: &Perception,
    kin: Option<Kin>,
) {
    c.goal = Goal::Patrol;
    c.rest_reason = None;
    if let Some((px, py)) = p.best_patrol {
        if geom::cheb(c.x, c.y, px, py) > 2 && rng.chance(0.5) {
            c.target = Some((px, py));
            return;
        }
    }
    wander(c, world, time, view, gp, sp, rng, kin);
    c.goal = Goal::Patrol;
}
