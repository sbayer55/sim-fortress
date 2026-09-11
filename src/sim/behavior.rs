//! Creature behaviour: perception, goal selection (with hysteresis), fractional
//! movement, grazing/drinking/resting, mating (C4), den creation, death and the
//! day boundary.

use std::collections::BTreeMap;

use crate::sim::creatures::{Cause, Creature, CreatureId, CreatureStore, Death, DeathTallies, Goal, HuntPhase, RestReason};
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::genetics::{self, TickView, OFF8};
use crate::sim::geom;
use crate::sim::lineage::Lineage;
use crate::sim::params::{CreaturesParams, EcologyParams, GeneticsParams, PredationParams};
use crate::sim::predation::{self, MAX_SENSE_CELLS};
use crate::sim::rng::Rng;
use crate::sim::spatial::SpatialIndex;
use crate::sim::species::{Kind, SpeciesId};
use crate::sim::time::Time;
use crate::sim::world::{Terrain, World};

/// What a creature can see at a replan. Creature ids feed mate selection (C4)
/// and predation (C5).
pub struct Perception {
    pub nearest_water: Option<(usize, usize)>,
    pub best_graze: Option<((usize, usize), f32)>,
    pub nearest_den: Option<(usize, usize)>,
    /// Highest `prey_pressure` cell within sense range (predator Patrol, C5).
    pub best_patrol: Option<(usize, usize)>,
    pub creatures: Vec<CreatureId>,
}

/// Advance every living creature one tick, in slot order (FR9), then run the
/// C4/C5 passes: hunt contacts (kill/eat), scavenging, consummation and delivery.
#[allow(clippy::too_many_arguments)]
pub fn tick_creatures(
    store: &mut CreatureStore,
    spatial: &SpatialIndex,
    world: &mut World,
    events: &mut EventRing,
    time: &Time,
    cp: &CreaturesParams,
    ep: &EcologyParams,
    gp: &GeneticsParams,
    pp: &PredationParams,
    rng: &mut Rng,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
    soft_cap_noted: &mut bool,
) {
    let view = TickView::build(store, time, world, gp);
    // FR5: predator-first threat marking (bucket-bounded; never per-prey scans).
    mark_threats(store, spatial, world, time.tick, pp);
    for c in store.living_mut() {
        update_one(c, spatial, world, events, time, cp, ep, gp, pp, &view, rng, tallies, lineage);
    }
    genetics::consummate(store, time, gp, events, &view, soft_cap_noted);
    hunt_contacts(store, world, events, time, pp, rng, tallies, lineage);
    scavenge_contacts(store, time, pp);
    genetics::deliver(store, world, events, time, gp, cp, rng, tallies, lineage);
}

/// The day-boundary step: age death, adult re-evaluation, carcass decay/free and
/// pressure decay. Runs at midnight, before the census.
pub fn day_boundary(
    store: &mut CreatureStore,
    world: &mut World,
    events: &mut EventRing,
    time: &Time,
    cp: &CreaturesParams,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
) {
    let day_index = time.day_index();

    // 1. Age death + adult re-evaluation (FR3, FR7).
    for c in store.living_mut() {
        let age = c.age_days(day_index);
        c.adult = age >= cp.adult_age(c.species);
        if age >= c.max_age_days(cp) {
            kill(c, Cause::Age, world, events, time, tallies, lineage, None, 0, None);
        }
    }

    // 2. Carcass decay and slot freeing (FR7).
    let decay_step = 1.0 / cp.carcass_decay_days.max(1) as f32;
    let mut to_free: Vec<CreatureId> = Vec::new();
    for c in store.carcasses_mut() {
        c.decay += decay_step;
        if c.decay >= 1.0 {
            to_free.push(c.id);
        }
    }
    for id in to_free {
        if let Some(c) = store.remove(id) {
            world.carcasses.retain(|&(x, y)| !(x == c.x && y == c.y));
        }
    }

    // 3. Pressure decay (FR8): both traffic maps decay the same way.
    for cell in &mut world.cells {
        cell.prey_pressure *= cp.pressure_decay_per_day;
        cell.pred_pressure *= cp.pressure_decay_per_day;
    }
}

/// C5 FR7: daily per-(species, region) migration evaluation. A group migrates
/// when its trigger holds for `migrate_days` consecutive days and its
/// (species, region) cooldown has passed.
pub fn migration_daily(
    store: &mut CreatureStore,
    world: &World,
    events: &mut EventRing,
    time: &Time,
    ep: &EcologyParams,
    pp: &PredationParams,
    cooldown_until: &mut [u64; 48],
    days_below: &mut [u32; 48],
) {
    let n_regions = world.regions.len().min(8);
    let mut counts = [[0u32; 8]; 6];
    let mut prey_per_region = [0u32; 8];
    // Sum of `pred_pressure` over the cells the group occupies (FR7: the prey
    // trigger uses the mean over occupied cells, not the region mean).
    let mut occupied_pressure = [[0.0f32; 8]; 6];
    for c in store.living() {
        let ri = world.region_index(c.x, c.y).min(7);
        counts[c.species.index()][ri] += 1;
        occupied_pressure[c.species.index()][ri] += world.cell(c.x, c.y).pred_pressure;
        if c.species.kind() == Kind::Prey {
            prey_per_region[ri] += 1;
        }
    }
    let season_cap = ep.season_cap.get(&time.season()).copied().unwrap_or(1.0);

    for (si, id) in SpeciesId::ALL.iter().enumerate() {
        for ri in 0..n_regions {
            let key = si * 8 + ri;
            if counts[si][ri] == 0 || time.tick < cooldown_until[key] {
                days_below[key] = 0;
                continue;
            }
            let trigger = if id.kind() == Kind::Prey {
                let r = &world.regions[ri];
                let veg_mean = crate::sim::ecology::region_land_veg_mean(world, r);
                let shortfall = season_cap > 0.0 && veg_mean / season_cap < pp.migrate_veg;
                let group_pressure = occupied_pressure[si][ri] / counts[si][ri] as f32;
                let pressure = group_pressure > pp.migrate_pressure;
                shortfall || pressure
            } else {
                prey_per_region[ri] < pp.migrate_prey_min
            };
            if trigger {
                days_below[key] += 1;
                if days_below[key] >= pp.migrate_days {
                    days_below[key] = 0;
                    cooldown_until[key] = time.tick + pp.migrate_cooldown_days as u64 * time.ticks_per_day as u64;
                    migrate_group(store, world, events, time, *id, ri);
                }
            } else {
                days_below[key] = 0;
            }
        }
    }
}

fn mean_pred_pressure(world: &World, r: &crate::sim::world::RegionRect) -> f32 {
    let (mut sum, mut n) = (0.0f32, 0usize);
    for y in r.2..r.4 {
        for x in r.1..r.3 {
            let c = &world.cells[y * world.width + x];
            if !c.terrain.is_water() {
                sum += c.pred_pressure;
                n += 1;
            }
        }
    }
    if n > 0 { sum / n as f32 } else { 0.0 }
}

fn regions_adjacent(a: &crate::sim::world::RegionRect, b: &crate::sim::world::RegionRect) -> bool {
    let (ax0, ay0, ax1, ay1) = (a.1, a.2, a.3, a.4);
    let (bx0, by0, bx1, by1) = (b.1, b.2, b.3, b.4);
    let overlap_y = ay0 < by1 && by0 < ay1;
    let overlap_x = ax0 < bx1 && bx0 < ax1;
    (ax1 == bx0 || bx1 == ax0) && overlap_y || (ay1 == by0 || by1 == ay0) && overlap_x
}

fn migrate_group(store: &mut CreatureStore, world: &World, events: &mut EventRing, time: &Time, id: SpeciesId, origin_ri: usize) {
    let members: Vec<CreatureId> = store
        .living()
        .filter(|c| c.species == id && world.region_index(c.x, c.y) == origin_ri)
        .map(|c| c.id)
        .collect();
    if members.is_empty() {
        return;
    }
    let n = members.len() as u32;

    // Destination = adjacent region maximising the species' score.
    let mut best_dest: Option<usize> = None;
    let mut best_score = f32::NEG_INFINITY;
    for (di, r) in world.regions.iter().enumerate() {
        if di == origin_ri || !regions_adjacent(&world.regions[origin_ri], r) {
            continue;
        }
        let score = if id.kind() == Kind::Prey {
            crate::sim::ecology::region_land_veg_mean(world, r) * (1.0 - mean_pred_pressure(world, r))
        } else {
            store.living().filter(|c| c.species.kind() == Kind::Prey && world.region_index(c.x, c.y) == di).count() as f32
        };
        if score > best_score {
            best_score = score;
            best_dest = Some(di);
        }
    }
    let Some(dest) = best_dest else { return };
    let Some((tx, ty)) = migration_target_cell(world, id, dest) else { return };

    let migrate_until = time.tick + 2 * time.ticks_per_day as u64;
    for mid in &members {
        if let Some(c) = store.get_mut(*mid) {
            c.goal = Goal::Migrate;
            c.migrate_target = Some((tx, ty));
            c.migrate_until = migrate_until;
            c.target = Some((tx, ty));
            c.replan_at = time.tick;
        }
    }

    let group_word = if n <= 3 { "family" } else if id.kind() == Kind::Prey { "herd" } else { "pack" };
    let origin = &world.regions[origin_ri];
    let dest_name = world.regions[dest].0.clone();
    let pos = ((origin.1 + origin.3) / 2, (origin.2 + origin.4) / 2);
    events.push(Event {
        year: time.year(),
        day: time.day_of_year(),
        hour: time.hour(),
        kind: EventKind::Migration,
        species: Some(id),
        subject: None,
        text: format!("A {} of {} {} migrated from {} to {}", group_word, n, id.plural(), origin.0, dest_name),
        pos: Some(pos),
        // `detail` carries "origin→dest" region indices for tests and S06.
        detail: format!("{origin_ri}>{dest}"),
    });
}

/// The walkable destination cell with the highest vegetation (prey) or
/// `prey_pressure` (predators) — the group's `target_cell` (FR7).
fn migration_target_cell(world: &World, id: SpeciesId, dest: usize) -> Option<(usize, usize)> {
    let r = &world.regions[dest];
    let mut best: Option<(usize, usize)> = None;
    let mut best_score = f32::NEG_INFINITY;
    for y in r.2..r.4 {
        for x in r.1..r.3 {
            let c = &world.cells[y * world.width + x];
            if !c.terrain.walkable() || c.terrain.is_water() {
                continue;
            }
            let score = if id.kind() == Kind::Prey { c.vegetation } else { c.prey_pressure };
            if score > best_score {
                best_score = score;
                best = Some((x, y));
            }
        }
    }
    best
}

#[allow(clippy::too_many_arguments)]
fn update_one(
    c: &mut Creature,
    spatial: &SpatialIndex,
    world: &mut World,
    events: &mut EventRing,
    time: &Time,
    cp: &CreaturesParams,
    ep: &EcologyParams,
    gp: &GeneticsParams,
    pp: &PredationParams,
    view: &TickView,
    rng: &mut Rng,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
) {
    // FR5: Flee pre-empts every goal for prey.
    if c.species.kind() == Kind::Prey {
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
                c.flee_until = time.tick + pp.flee_ticks as u64;
                c.chased += 1;
                if let Some((_, _, sp)) = c.threatened_by {
                    c.threats_by_species[sp.index()] += 1;
                }
            }
            c.target = flee_target(c, world, pp);
            c.replan_at = time.tick + 1;
        }
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
    if goal_satisfied(c, time, pp) {
        c.replan_at = time.tick;
    }
    // Replan when due.
    if time.tick >= c.replan_at {
        replan(c, spatial, world, time, cp, gp, pp, view, rng);
    }
    // Track the current prey target while hunting (Stalk → Chase).
    if c.goal == Goal::Hunt && c.hunt_phase != HuntPhase::Eat {
        update_hunt_stalk(c, view, time, pp, tallies);
    }
    // Move toward the target.
    move_toward(c, world, time, cp, pp);
    // Act on the goal at the current location.
    act(c, world, events, time, cp, rng);
    // Needs and hp.
    needs(c, world, time, cp, ep, gp);
    // Death.
    maybe_die(c, world, events, time, tallies, lineage);
    // Pressure.
    pressure(c, world, cp);
}

fn goal_satisfied(c: &Creature, time: &Time, pp: &PredationParams) -> bool {
    match c.goal {
        Goal::Drink => c.thirst <= 0.1,
        Goal::Graze => c.hunger <= 0.2,
        Goal::Rest => match c.rest_reason {
            Some(RestReason::Forced) => c.energy >= 0.3,
            Some(RestReason::Energy) => c.energy >= 0.9,
            // Diurnal wake at sunrise; nocturnal wake at nightfall.
            Some(RestReason::Night) => {
                if pp.is_nocturnal(c.species) { time.is_night() } else { !time.is_night() }
            }
            None => true,
        },
        Goal::Wander => c.target.is_none(),
        // Mated (cooldown just set) or the partner is gone.
        Goal::Mate => c.mate_id.is_none() || c.cooldown_until > time.tick,
        Goal::Flee => c.threatened_by.is_none()
            || time.tick >= c.flee_until
            || c.threatened_by.is_none_or(|(px, py, _)| geom::dist(c.x, c.y, px, py) >= pp.flee_distance),
        Goal::Migrate => time.tick >= c.migrate_until || c.migrate_target.is_none(),
        // Hunt / Scavenge / Patrol are ended by their dedicated passes.
        Goal::Hunt | Goal::Scavenge | Goal::Patrol => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn replan(
    c: &mut Creature,
    spatial: &SpatialIndex,
    world: &World,
    time: &Time,
    cp: &CreaturesParams,
    gp: &GeneticsParams,
    pp: &PredationParams,
    view: &TickView,
    rng: &mut Rng,
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

    if c.species.kind() == Kind::Predator {
        replan_predator(c, spatial, world, time, cp, gp, pp, view, rng);
    } else {
        replan_prey(c, spatial, world, time, cp, gp, pp, view, rng);
    }
}

#[allow(clippy::too_many_arguments)]
fn replan_prey(
    c: &mut Creature,
    spatial: &SpatialIndex,
    world: &World,
    time: &Time,
    cp: &CreaturesParams,
    gp: &GeneticsParams,
    pp: &PredationParams,
    view: &TickView,
    rng: &mut Rng,
) {
    let next = time.tick + cp.replan_ticks;
    let p = perceive(c, spatial, world, cp);

    // 1. Drink (enter thirst > 0.6, stay while thirst > 0.1).
    if c.thirst > 0.6 || (c.goal == Goal::Drink && c.thirst > 0.1) {
        if let Some(water) = p.nearest_water.or(c.last_water) {
            c.goal = Goal::Drink;
            c.rest_reason = None;
            c.target = Some(water);
        } else {
            wander(c, world, time, view, gp, rng);
        }
        c.replan_at = next;
        return;
    }

    // 2. Graze (enter hunger > 0.5, stay while hunger > 0.2 — hysteresis).
    if c.hunger > 0.5 || (c.goal == Goal::Graze && c.hunger > 0.2) {
        c.goal = Goal::Graze;
        c.rest_reason = None;
        match p.best_graze {
            Some((cell, score)) => {
                let cur = world.cell(c.x, c.y);
                let cur_score = if cur.vegetation >= cp.graze_min_vegetation {
                    cur.vegetation
                } else {
                    0.0
                };
                c.target = if cur_score >= score * 0.9 && cur.vegetation >= cp.graze_min_vegetation {
                    None // graze in place
                } else {
                    Some(cell)
                };
            }
            None => {
                if world.cell(c.x, c.y).vegetation >= cp.graze_min_vegetation {
                    c.target = None;
                } else {
                    wander(c, world, time, view, gp, rng);
                    c.goal = Goal::Graze;
                }
            }
        }
        c.replan_at = next;
        return;
    }

    // 3. Rest (enter energy < 0.25 or night; stay until satisfied).
    let rest_enter = c.energy < 0.25 || time.is_night();
    let rest_stay = c.goal == Goal::Rest && !goal_satisfied(c, time, pp);
    if rest_enter || rest_stay {
        if c.goal != Goal::Rest {
            c.rest_reason = Some(if c.energy < 0.25 { RestReason::Energy } else { RestReason::Night });
        }
        c.goal = Goal::Rest;
        c.target = p.nearest_den;
        c.replan_at = next;
        return;
    }

    // 4. Mate (C4 FR2): eligible adults seek the nearest eligible partner.
    if view.cap_ok && genetics::eligible(c, time, world, gp) {
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
    wander(c, world, time, view, gp, rng);
    c.replan_at = next;
}

#[allow(clippy::too_many_arguments)]
fn replan_predator(
    c: &mut Creature,
    spatial: &SpatialIndex,
    world: &World,
    time: &Time,
    cp: &CreaturesParams,
    gp: &GeneticsParams,
    pp: &PredationParams,
    view: &TickView,
    rng: &mut Rng,
) {
    let next = time.tick + cp.replan_ticks;
    let p = perceive(c, spatial, world, cp);

    // 1. Drink (thirst > 0.6).
    if c.thirst > 0.6 {
        if let Some(water) = p.nearest_water.or(c.last_water) {
            c.goal = Goal::Drink;
            c.rest_reason = None;
            c.target = Some(water);
        } else {
            patrol(c, world, time, view, gp, rng, &p);
        }
        c.replan_at = next;
        return;
    }

    // 2. Hunt (hunger > hunt_hunger_min and a detectable prey in range).
    if c.hunger > pp.hunt_hunger_min && time.tick >= c.hunt_cooldown_until {
        if let Some((prey, pos)) = pick_hunt_target(c, &p.creatures, view, world, pp) {
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
        if let Some((carcass, pos)) = pick_scavenge_target(c, view) {
            c.goal = Goal::Scavenge;
            c.rest_reason = None;
            c.scavenge_target = Some(carcass);
            c.target = Some(pos);
            c.replan_at = next;
            return;
        }
    }

    // 4. Mate (C4 rules; the vegetation gate does not apply to predators).
    if view.cap_ok && genetics::eligible(c, time, world, gp) {
        if let Some((mate, pos)) = genetics::pick_mate(c, &p.creatures, view) {
            c.goal = Goal::Mate;
            c.rest_reason = None;
            c.mate_id = Some(mate);
            c.target = Some(pos);
            c.replan_at = next;
            return;
        }
    }

    // 5. Rest (energy < 0.25, or night for diurnal / day for nocturnal).
    let rest_enter = c.energy < 0.25
        || if pp.is_nocturnal(c.species) { !time.is_night() } else { time.is_night() };
    let rest_stay = c.goal == Goal::Rest && !goal_satisfied(c, time, pp);
    if rest_enter || rest_stay {
        if c.goal != Goal::Rest {
            c.rest_reason = Some(if c.energy < 0.25 { RestReason::Energy } else { RestReason::Night });
        }
        c.goal = Goal::Rest;
        c.target = None; // predators rest in place
        c.replan_at = next;
        return;
    }

    // 6. Patrol (wander biased toward the highest prey_pressure cell seen).
    patrol(c, world, time, view, gp, rng, &p);
    c.replan_at = next;
}

/// Patrol (FR3): a wander *biased* toward the highest `prey_pressure` cell seen —
/// half of the replans head there when it is more than two cells away; the rest
/// wander, so a satiated predator never camps on a hotspot (a water hole or den)
/// and denies the prey their drink.
fn patrol(c: &mut Creature, world: &World, time: &Time, view: &TickView, gp: &GeneticsParams, rng: &mut Rng, p: &Perception) {
    c.goal = Goal::Patrol;
    c.rest_reason = None;
    if let Some((px, py)) = p.best_patrol {
        if geom::cheb(c.x, c.y, px, py) > 2 && rng.chance(0.5) {
            c.target = Some((px, py));
            return;
        }
    }
    wander(c, world, time, view, gp, rng);
    c.goal = Goal::Patrol;
}

fn wander(c: &mut Creature, world: &World, time: &Time, view: &TickView, gp: &GeneticsParams, rng: &mut Rng) {
    c.goal = Goal::Wander;
    c.rest_reason = None;
    // C4 FR4: juveniles stay near their living mother.
    if !c.adult {
        if let Some(t) = genetics::follow_target(c, view, world, time, gp, rng) {
            c.target = Some(t);
            return;
        }
    }
    // Keep the current heading with p = 0.7, else choose a new direction.
    let mut dir = random_dir(rng);
    if let Some((tx, ty)) = c.target {
        if rng.chance(0.7) {
            let dx = (tx as i32 - c.x as i32).signum();
            let dy = (ty as i32 - c.y as i32).signum();
            if dx != 0 || dy != 0 {
                dir = (dx, dy);
            }
        }
    }
    let steps = 4 + rng.below(5) as i32; // 4..=8
    let nx = c.x as i32 + dir.0 * steps;
    let ny = c.y as i32 + dir.1 * steps;
    if world.in_bounds(nx, ny) && world.cell(nx as usize, ny as usize).terrain.walkable() {
        c.target = Some((nx as usize, ny as usize));
    } else {
        c.target = find_walkable_near(c.x, c.y, world);
    }
}

fn random_dir(rng: &mut Rng) -> (i32, i32) {
    let i = rng.below(8);
    OFF8[i]
}

/// The nearest walkable cell within a small radius (wander fallback).
pub(crate) fn find_walkable_near(x: usize, y: usize, world: &World) -> Option<(usize, usize)> {
    for r in 1i32..=8 {
        for dy in -r..=r {
            for dx in -r..=r {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if world.in_bounds(nx, ny) && world.cell(nx as usize, ny as usize).terrain.walkable() {
                    return Some((nx as usize, ny as usize));
                }
            }
        }
    }
    None
}

fn perceive(c: &Creature, spatial: &SpatialIndex, world: &World, cp: &CreaturesParams) -> Perception {
    let r = c.genome.sense_cells(); // u16
    let r_i = r as i32;
    let r_f = r as f32;
    let (cx, cy) = (c.x, c.y);
    let y0 = (cy as i32 - r_i).max(0) as usize;
    let y1 = ((cy as i32 + r_i + 1).min(world.height as i32)).max(0) as usize;

    let mut nearest_water: Option<(usize, usize)> = None;
    let mut nearest_water_d = f32::INFINITY;
    let mut best_graze: Option<((usize, usize), f32)> = None;
    let mut best_score = f32::NEG_INFINITY;
    let mut best_patrol: Option<(usize, usize)> = None;
    let mut best_patrol_score = f32::NEG_INFINITY;
    let use_shore = world.shore.len() == world.cells.len();

    // Visit only the rows of the ellipse; each row's cell span is contiguous.
    for y in y0..y1 {
        let dy = y as f32 - cy as f32;
        let half = 2.0 * (r_f * r_f - dy * dy).max(0.0).sqrt();
        let x0 = ((cx as f32 - half).ceil() as i32).max(0) as usize;
        let x1 = (((cx as f32 + half).floor() as i32) + 1).min(world.width as i32).max(0) as usize;
        if x0 >= x1 {
            continue;
        }
        let row = y * world.width;
        let cells = &world.cells[row + x0..row + x1];
        for (i, cell) in cells.iter().enumerate() {
            let x = x0 + i;
            if cell.terrain.is_water() {
                continue;
            }
            let dx = (x as f32 - cx as f32) / 2.0;
            let d = (dx * dx + dy * dy).sqrt();
            if d > r_f {
                continue;
            }
            let shore = if use_shore { world.shore[row + x] } else { world.is_shore(x, y) };
            if shore && d < nearest_water_d {
                nearest_water_d = d;
                nearest_water = Some((x, y));
            }
            if cell.vegetation >= cp.graze_min_vegetation {
                let score = cell.vegetation / (1.0 + d / 4.0);
                if score > best_score {
                    best_score = score;
                    best_graze = Some(((x, y), score));
                }
            }
            if cell.prey_pressure > best_patrol_score {
                best_patrol_score = cell.prey_pressure;
                best_patrol = Some((x, y));
            }
        }
    }

    let mut nearest_den: Option<(usize, usize)> = None;
    let mut nearest_den_d = f32::INFINITY;
    for &(dx, dy) in &world.dens {
        let d = geom::dist(cx, cy, dx, dy);
        if d <= 2.0 * r_f && d < nearest_den_d {
            nearest_den_d = d;
            nearest_den = Some((dx, dy));
        }
    }

    let creatures = spatial.within(cx, cy, r);
    Perception { nearest_water, best_graze, nearest_den, best_patrol, creatures }
}

fn move_toward(c: &mut Creature, world: &World, time: &Time, cp: &CreaturesParams, pp: &PredationParams) {
    if c.target.is_none() {
        c.path.clear();
        return;
    }
    let mut speed = cp.move_speed_base + cp.move_speed_per_trait * c.genome.speed();
    speed = if c.adult { speed } else { speed * 0.75 };
    // FR4: a hunting predator moves with the chase speed bonus.
    if c.goal == Goal::Hunt && c.hunt_phase != HuntPhase::Eat {
        speed += pp.chase_speed_bonus;
    }
    c.move_budget = (c.move_budget + speed).min(2.0);
    // A path searched for an earlier target (a flee vector, a fled prey) must
    // not be followed toward the new one.
    if !c.path.is_empty() && c.path_for != c.target {
        c.path.clear();
    }
    while c.move_budget >= 1.0 {
        let Some(target) = c.target else { break };
        if (c.x, c.y) == target {
            c.path.clear();
            if c.goal == Goal::Wander {
                c.target = None; // waypoint reached
            }
            break;
        }
        // A planned step (from a path search) takes precedence when still valid.
        let planned = match c.path.last() {
            Some(&(nx, ny)) if geom::cheb(c.x, c.y, nx, ny) == 1 && world.cell(nx, ny).terrain.walkable() => {
                c.path.pop();
                Some((nx, ny))
            }
            Some(_) => {
                c.path.clear();
                None
            }
            None => None,
        };
        let next = match planned.or_else(|| step_toward(c.x, c.y, target, world)) {
            Some(n) => Some(n),
            None => {
                // Greedy stalled at an obstacle (FR6 fallback): plan around it.
                match bfs_path((c.x, c.y), target, world, PATH_KEEP) {
                    Some(mut path) => {
                        path.reverse();
                        let first = path.pop();
                        c.path = path;
                        c.path_for = Some(target);
                        first
                    }
                    None => None,
                }
            }
        };
        match next {
            Some((nx, ny)) => {
                c.x = nx;
                c.y = ny;
                c.energy -= if c.goal == Goal::Flee { cp.move_cost_energy * pp.flee_energy_factor } else { cp.move_cost_energy };
                c.move_budget -= 1.0;
                c.trail.push((nx, ny));
                if c.trail.len() > cp.trail_len {
                    c.trail.remove(0);
                }
                if (c.x, c.y) == target && c.goal == Goal::Wander {
                    c.target = None;
                    c.path.clear();
                    break;
                }
            }
            None => {
                // Unreachable: drop the target and replan. A remembered water
                // spot that cannot be reached is forgotten.
                c.target = None;
                c.path.clear();
                if c.goal == Goal::Drink {
                    c.last_water = None;
                }
                c.replan_at = time.tick;
                break;
            }
        }
    }
    // FR6: a creature must never occupy an impassable cell.
    debug_assert!(world.cell(c.x, c.y).terrain.walkable(), "creature {} on impassable {:?}", c.id.0, world.cell(c.x, c.y).terrain);
}

/// One 8-neighbour step toward `target` (FR6). Returns `None` when already there
/// or when no walkable neighbour strictly reduces the distance (a local minimum
/// such as a rock face or lake shore), which hands over to `bfs_path`.
fn step_toward(x: usize, y: usize, target: (usize, usize), world: &World) -> Option<(usize, usize)> {
    let (tx, ty) = target;
    let cur_d = geom::dist(x, y, tx, ty);

    let mut best: Option<(usize, usize)> = None;
    let mut best_d = f32::INFINITY;
    for &(dx, dy) in &OFF8 {
        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
        if world.in_bounds(nx, ny) && world.cell(nx as usize, ny as usize).terrain.walkable() {
            let d = geom::dist(nx as usize, ny as usize, tx, ty);
            if d < best_d {
                best_d = d;
                best = Some((nx as usize, ny as usize));
            }
        }
    }
    if best_d < cur_d {
        return best;
    }
    None
}

/// Planned steps kept after a path search; the greedy step resumes afterwards.
const PATH_KEEP: usize = 24;
/// Padding (cells) around the start/target bounding box searched by `bfs_path`.
const PATH_PAD_X: i32 = 16;
const PATH_PAD_Y: i32 = 8;

/// Bounded breadth-first search over walkable cells inside the padded bounding
/// box of `from` and `target`. Returns the path (excluding `from`, including
/// `target`) truncated to `keep` steps, or `None` when unreachable in the box.
fn bfs_path(from: (usize, usize), target: (usize, usize), world: &World, keep: usize) -> Option<Vec<(usize, usize)>> {
    let (fx, fy) = (from.0 as i32, from.1 as i32);
    let (tx, ty) = (target.0 as i32, target.1 as i32);
    if !world.in_bounds(tx, ty) {
        return None;
    }
    let x0 = (fx.min(tx) - PATH_PAD_X).max(0);
    let y0 = (fy.min(ty) - PATH_PAD_Y).max(0);
    let x1 = (fx.max(tx) + PATH_PAD_X + 1).min(world.width as i32);
    let y1 = (fy.max(ty) + PATH_PAD_Y + 1).min(world.height as i32);
    let bw = (x1 - x0) as usize;
    let bh = (y1 - y0) as usize;
    let idx = |x: i32, y: i32| ((y - y0) as usize) * bw + (x - x0) as usize;
    // Parent index per box cell; u32::MAX = unvisited.
    let mut parent = vec![u32::MAX; bw * bh];
    let mut queue: Vec<(i32, i32)> = Vec::with_capacity(bw * bh / 4);
    let start = idx(fx, fy);
    parent[start] = start as u32;
    queue.push((fx, fy));
    let mut head = 0;
    let mut found = false;
    while head < queue.len() {
        let (x, y) = queue[head];
        head += 1;
        if (x, y) == (tx, ty) {
            found = true;
            break;
        }
        for &(dx, dy) in &OFF8 {
            let (nx, ny) = (x + dx, y + dy);
            if nx < x0 || ny < y0 || nx >= x1 || ny >= y1 {
                continue;
            }
            let ni = idx(nx, ny);
            if parent[ni] != u32::MAX {
                continue;
            }
            if !world.cell(nx as usize, ny as usize).terrain.walkable() {
                continue;
            }
            parent[ni] = idx(x, y) as u32;
            queue.push((nx, ny));
        }
    }
    if !found {
        return None;
    }
    // Walk back from the target to the start.
    let mut full: Vec<(usize, usize)> = Vec::new();
    let mut cur = idx(tx, ty);
    while cur != start {
        let cx = (cur % bw) as i32 + x0;
        let cy = (cur / bw) as i32 + y0;
        full.push((cx as usize, cy as usize));
        cur = parent[cur] as usize;
    }
    full.reverse();
    full.truncate(keep);
    Some(full)
}

fn act(c: &mut Creature, world: &mut World, events: &mut EventRing, time: &Time, cp: &CreaturesParams, rng: &mut Rng) {
    match c.goal {
        Goal::Graze => graze(c, world, cp),
        Goal::Drink => drink(c, world, cp),
        Goal::Rest => maybe_make_den(c, world, events, time, cp, rng),
        _ => {}
    }
}

fn graze(c: &mut Creature, world: &mut World, cp: &CreaturesParams) {
    let cell = world.cell_mut(c.x, c.y);
    let g = cell.vegetation.min(cp.graze_per_hour);
    cell.vegetation -= g;
    c.hunger = (c.hunger - cp.graze_nutrition * g).max(0.0);
}

fn drink(c: &mut Creature, world: &World, cp: &CreaturesParams) {
    if world.is_shore(c.x, c.y) {
        c.thirst = (c.thirst - cp.drink_per_hour).max(0.0);
        c.last_water = Some((c.x, c.y));
    }
}

fn maybe_make_den(c: &mut Creature, world: &mut World, events: &mut EventRing, time: &Time, cp: &CreaturesParams, rng: &mut Rng) {
    if !is_resting(c) || in_den(c, world) {
        return;
    }
    let cell = world.cell(c.x, c.y);
    if cell.vegetation >= 0.2 || !matches!(cell.terrain, Terrain::Dirt | Terrain::GrassDense | Terrain::Forest) {
        return;
    }
    let ri = world.region_index(c.x, c.y);
    let region_dens = world.dens.iter().filter(|&&(dx, dy)| world.region_index(dx, dy) == ri).count();
    if region_dens >= cp.max_dens_per_region {
        return;
    }
    if rng.chance(cp.den_create_chance_per_rest_hour) {
        world.dens.push((c.x, c.y));
        events.push(Event {
            year: time.year(),
            day: time.day_of_year(),
            hour: time.hour(),
            kind: EventKind::Note,
            species: Some(c.species),
            subject: Some(c.id),
            text: format!("{} {} discovered a new den site in {}", c.name_str(), c.tag(), world.region_name(c.x, c.y)),
            pos: Some((c.x, c.y)),
            detail: String::new(),
        });
    }
}

pub(crate) fn needs(c: &mut Creature, world: &World, time: &Time, cp: &CreaturesParams, ep: &EcologyParams, gp: &GeneticsParams) {
    let season_metabolism = ep.season_metabolism.get(&time.season()).copied().unwrap_or(1.0);
    let pregnancy = if c.pregnant_due.is_some() { gp.pregnancy_hunger_factor } else { 1.0 };
    c.hunger = (c.hunger + cp.hunger_per_hour(c.genome.size(), c.genome.metabolism(), season_metabolism) * pregnancy).min(2.0);
    c.thirst = (c.thirst + cp.thirst_per_hour).min(2.0);

    if is_resting(c) {
        let bonus = if in_den(c, world) { cp.den_rest_bonus } else { 1.0 };
        c.energy = (c.energy + cp.energy_rest_per_hour * bonus).min(1.0);
    } else {
        c.energy -= cp.energy_awake_per_hour;
    }

    if c.hunger >= 1.0 || c.thirst >= 1.0 {
        c.hp -= cp.hp_loss_per_hour;
    } else if c.hunger < 0.5 && c.thirst < 0.5 {
        c.hp = (c.hp + cp.hp_regen_per_hour).min(1.0);
    }
}

fn maybe_die(c: &mut Creature, world: &mut World, events: &mut EventRing, time: &Time, tallies: &mut DeathTallies, lineage: &mut Lineage) {
    if c.alive && c.hp <= 0.0 {
        let cause = if c.thirst >= 1.0 {
            Cause::Thirst
        } else if c.hunger >= 1.0 {
            Cause::Starved
        } else {
            Cause::Injury
        };
        kill(c, cause, world, events, time, tallies, lineage, None, 0, None);
    }
}

fn pressure(c: &mut Creature, world: &mut World, cp: &CreaturesParams) {
    match c.species.kind() {
        Kind::Prey => {
            let cell = world.cell_mut(c.x, c.y);
            cell.prey_pressure = (cell.prey_pressure + cp.pressure_per_creature_tick).min(1.0);
        }
        Kind::Predator => {
            let cell = world.cell_mut(c.x, c.y);
            cell.pred_pressure = (cell.pred_pressure + cp.pressure_per_creature_tick).min(1.0);
        }
    }
}

fn is_resting(c: &Creature) -> bool {
    c.goal == Goal::Rest && (c.target.is_none() || c.target == Some((c.x, c.y)))
}

fn in_den(c: &Creature, world: &World) -> bool {
    world.dens.iter().any(|&(x, y)| x == c.x && y == c.y)
}

/// Mark a creature dead, add a carcass, record the tally and emit the event.
/// `killer`/`chase_ticks`/`killer_label` are set for predation deaths.
#[allow(clippy::too_many_arguments)]
fn kill(
    c: &mut Creature,
    cause: Cause,
    world: &mut World,
    events: &mut EventRing,
    time: &Time,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
    killer: Option<CreatureId>,
    chase_ticks: u16,
    killer_label: Option<&str>,
) {
    c.alive = false;
    c.hp = 0.0;
    c.target = None;
    c.pregnant_due = None;
    c.death = Some(Death { cause, day: time.day_index() as u32, killer, chase_ticks });
    world.carcasses.push((c.x, c.y));
    tallies.deaths[c.species.index()] += 1;
    lineage.record_death(c.id, time.day_index() as u32);
    tallies.last_death[c.species.index()] = Some(crate::sim::creatures::ExtinctionRecord {
        species: c.species,
        last: c.id,
        name: c.name_str().to_string(),
        tag: c.tag(),
        cause,
        day: time.day_index() as u32,
        age: c.age_days(time.day_index()),
        region: world.region_name(c.x, c.y).to_string(),
        pos: (c.x, c.y),
    });

    let kind = match cause {
        Cause::Starved => {
            tallies.starved += 1;
            EventKind::DeathStarved
        }
        Cause::Thirst => {
            tallies.thirst += 1;
            EventKind::DeathThirst
        }
        Cause::Age => {
            tallies.age += 1;
            EventKind::DeathAge
        }
        Cause::Predation => {
            tallies.predation += 1;
            EventKind::DeathPredation
        }
        Cause::Injury => return, // no event until C5
    };

    let region = world.region_name(c.x, c.y).to_string();
    let text = match cause {
        Cause::Starved => format!("{} {} starved in {}", c.name_str(), c.tag(), region),
        Cause::Thirst => format!("{} {} died of thirst in {}", c.name_str(), c.tag(), region),
        Cause::Age => format!("{} {} died of old age at {} days in {}", c.name_str(), c.tag(), c.age_days(time.day_index()), region),
        Cause::Predation => match killer_label {
            Some(k) => format!("{} {} was killed by {} in {}", c.name_str(), c.tag(), k, region),
            None => format!("{} {} was killed in {}", c.name_str(), c.tag(), region),
        },
        Cause::Injury => unreachable!(),
    };
    let detail = format!("hunger {:.2} thirst {:.2} energy {:.2}", c.hunger, c.thirst, c.energy);

    events.push(Event {
        year: time.year(),
        day: time.day_of_year(),
        hour: time.hour(),
        kind,
        species: Some(c.species),
        subject: Some(c.id),
        text,
        pos: Some((c.x, c.y)),
        detail,
    });
}

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
fn mark_threats(store: &mut CreatureStore, spatial: &SpatialIndex, world: &World, tick: u64, pp: &PredationParams) {
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
    let mut preds: Vec<Pred> = store
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
        .collect();
    preds.sort_unstable_by_key(|p| p.id);

    struct Acc {
        pos: (usize, usize),
        species: SpeciesId,
        dist: f32,
        count: u32,
    }
    let mut acc: BTreeMap<CreatureId, Acc> = BTreeMap::new();
    for pred in &preds {
        for prey_id in spatial.within(pred.x, pred.y, MAX_SENSE_CELLS) {
            let Some(prey) = store.get(prey_id) else { continue };
            if prey.species.kind() != Kind::Prey {
                continue;
            }
            let d = geom::dist(pred.x, pred.y, prey.x, prey.y);
            let in_range = d <= prey.genome.sense_cells() as f32;
            let detects = predation::prey_detects_pred_at(prey, pred.x, pred.y, pred.camouflage, pp);
            let e = acc.entry(prey_id).or_insert(Acc { pos: (pred.x, pred.y), species: pred.species, dist: f32::INFINITY, count: 0 });
            if in_range {
                e.count += 1;
            }
            let danger = pred.hunting == Some(prey_id) || (pred.hungry && geom::cheb(pred.x, pred.y, prey.x, prey.y) <= pp.chase_trigger_cheb);
            if detects && danger && d < pp.flee_distance && d < e.dist {
                e.dist = d;
                e.pos = (pred.x, pred.y);
                e.species = pred.species;
            }
        }
    }

    for prey in store.living_mut() {
        if prey.species.kind() != Kind::Prey {
            continue;
        }
        // A forced flee (FR4) keeps its last known threat until the timer ends.
        let keep_forced = prey.goal == Goal::Flee && tick < prey.flee_until && prey.threatened_by.is_some();
        match acc.get(&prey.id) {
            Some(a) => {
                if a.dist < f32::INFINITY {
                    prey.threatened_by = Some((a.pos.0, a.pos.1, a.species));
                } else if !keep_forced {
                    prey.threatened_by = None;
                }
                let pressure = world.cell(prey.x, prey.y).pred_pressure;
                prey.predation_risk = (0.5 * pressure + 0.5 * (a.count as f32) / 3.0).min(1.0);
            }
            None => {
                if !keep_forced {
                    prey.threatened_by = None;
                }
                prey.predation_risk = (0.5 * world.cell(prey.x, prey.y).pred_pressure).min(1.0);
            }
        }
    }
}

/// The cell `flee_distance` away from the threatening predator (FR5).
fn flee_target(c: &Creature, world: &World, pp: &PredationParams) -> Option<(usize, usize)> {
    let (px, py, _) = c.threatened_by?;
    let dx = c.x as i32 - px as i32;
    let dy = c.y as i32 - py as i32;
    if dx == 0 && dy == 0 {
        return Some((c.x, c.y));
    }
    let k = pp.flee_distance.ceil() as i32;
    let nx = (c.x as i32 + dx.signum() * k).clamp(0, world.width as i32 - 1) as usize;
    let ny = (c.y as i32 + dy.signum() * k).clamp(0, world.height as i32 - 1) as usize;
    Some((nx, ny))
}

/// Chase the current hunt target: keep it targeted, transition Stalk → Chase at
/// `chase_trigger_cheb`, and fail on timeout or when the prey leaves sense range.
fn update_hunt_stalk(c: &mut Creature, view: &TickView, time: &Time, pp: &PredationParams, tallies: &mut DeathTallies) {
    let Some(prey_id) = c.hunt_target else { return };
    let Some(prey) = view.get(prey_id) else {
        fail_hunt(c, time, pp, tallies);
        return;
    };
    if geom::dist(c.x, c.y, prey.x, prey.y) > c.genome.sense_cells() as f32 {
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
            if time.tick.saturating_sub(start) >= pp.chase_max_ticks as u64 {
                fail_hunt(c, time, pp, tallies);
            }
        }
    }
}

/// Record a failed hunt and re-arm for the next decision (FR4).
fn fail_hunt(c: &mut Creature, time: &Time, pp: &PredationParams, tallies: &mut DeathTallies) {
    c.attempts += 1;
    tallies.hunt_attempts[c.species.index()] += 1;
    c.hunt_cooldown_until = time.tick + pp.hunt_cooldown_hours as u64;
    c.hunt_phase = HuntPhase::Stalk;
    c.hunt_target = None;
    c.chase_start_tick = None;
    c.goal = Goal::Patrol;
    c.target = None;
    c.replan_at = time.tick + 1; // idle one tick
}

/// Highest `preference / (1 + dist/4)` detectable prey (preference 0 = never).
fn pick_hunt_target(
    c: &Creature,
    candidates: &[CreatureId],
    view: &TickView,
    world: &World,
    pp: &PredationParams,
) -> Option<(CreatureId, (usize, usize))> {
    let mut ids: Vec<CreatureId> = candidates.to_vec();
    ids.sort_unstable();
    let mut best: Option<(CreatureId, (usize, usize), f32)> = None;
    for id in ids {
        let Some(peer) = view.get(id) else { continue };
        if peer.species.kind() != Kind::Prey {
            continue;
        }
        let pref = pp.preference(c.species, peer.species);
        if pref <= 0.0 {
            continue;
        }
        if !predation::can_detect_peer(c, peer.x, peer.y, peer.camouflage, peer.goal == Goal::Rest, world, pp) {
            continue;
        }
        let d = geom::dist(c.x, c.y, peer.x, peer.y);
        let score = pref / (1.0 + d / 4.0);
        if best.is_none_or(|b| score > b.2) {
            best = Some((id, (peer.x, peer.y), score));
        }
    }
    best.map(|(id, pos, _)| (id, pos))
}

/// Nearest prey carcass within sense range (ties by id).
fn pick_scavenge_target(c: &Creature, view: &TickView) -> Option<(CreatureId, (usize, usize))> {
    let r = c.genome.sense_cells() as f32;
    let mut cs: Vec<&crate::sim::genetics::Carcass> = view.carcasses.iter().filter(|k| k.species.kind() == Kind::Prey).collect();
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
fn hunt_contacts(
    store: &mut CreatureStore,
    world: &mut World,
    events: &mut EventRing,
    time: &Time,
    pp: &PredationParams,
    rng: &mut Rng,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
) {
    let hunters: Vec<(CreatureId, CreatureId)> = store
        .living()
        .filter(|c| c.goal == Goal::Hunt && c.hunt_phase != HuntPhase::Eat)
        .filter_map(|c| c.hunt_target.map(|t| (c.id, t)))
        .collect();
    let mut hunters = hunters;
    hunters.sort_unstable();

    for (pred_id, prey_id) in hunters {
        // Snapshot the facts we need before mutating either creature.
        let snap = match (store.get(pred_id), store.get(prey_id)) {
            (Some(p), Some(q)) if p.alive && q.alive && p.goal == Goal::Hunt && p.hunt_target == Some(prey_id) => {
                Some((
                    p.genome.speed(),
                    p.genome.aggression(),
                    p.chase_start_tick,
                    q.species,
                    q.genome.size(),
                    q.genome.speed(),
                    q.x,
                    q.y,
                    geom::cheb(p.x, p.y, q.x, q.y),
                    format!("{} {}", p.name_str(), p.tag()),
                    (p.x, p.y, p.species),
                ))
            }
            _ => None,
        };
        let Some((pred_speed, pred_aggr, chase_start, prey_species, prey_size, prey_speed, px, py, cheb, killer_label, pred_at)) = snap
        else {
            continue;
        };
        if cheb > pp.catch_distance_cheb {
            continue;
        }

        let chance = pp.kill_chance(pred_speed, prey_speed, pred_aggr, prey_size);
        let success = rng.chance(chance);
        let chase_ticks = chase_start.map(|s| time.tick.saturating_sub(s) as u16).unwrap_or(0);

        if success {
            if let Some(prey) = store.get_mut(prey_id) {
                if prey.alive {
                    kill(prey, Cause::Predation, world, events, time, tallies, lineage, Some(pred_id), chase_ticks, Some(&killer_label));
                }
            }
            if let Some(p) = store.get_mut(pred_id) {
                p.kills += 1;
                p.kills_by_species[prey_species.index()] += 1;
                p.attempts += 1;
                tallies.hunt_attempts[p.species.index()] += 1;
                tallies.hunt_kills[p.species.index()] += 1;
                p.chase_stats.0 += chase_ticks as u32;
                if chase_ticks as u32 > p.chase_stats.1 {
                    p.chase_stats.1 = chase_ticks as u32;
                    p.chase_longest_year = time.year();
                }
                p.last_kill = Some((prey_id, time.day_index() as u32, world.region_index(px, py) as u8));
                p.hunt_cooldown_until = time.tick + pp.hunt_cooldown_hours as u64;
                p.hunt_phase = HuntPhase::Eat;
                p.hunt_target = None;
                p.chase_start_tick = None;
                p.eat_until = Some(time.tick + pp.eat_hours(prey_size) as u64);
                p.target = Some((px, py));
                // FR4: `hunger −= hunger_per_kill` — allowed to go negative, so a
                // big kill extends the satiation period and bounds the hunt rate.
                p.hunger -= pp.hunger_per_kill(prey_size);
            }
            if let Some(carcass) = store.get_mut(prey_id) {
                carcass.decay = (carcass.decay + pp.kill_consumes_decay).min(1.0);
            }
        } else {
            if let Some(p) = store.get_mut(pred_id) {
                fail_hunt(p, time, pp, tallies);
            }
            // The prey enters Flee regardless of whether it had detected the
            // predator: the threat is forced in so the away-vector exists, and
            // `mark_threats` retains it until the flee timer expires.
            if let Some(q) = store.get_mut(prey_id) {
                if q.alive {
                    if q.goal != Goal::Flee {
                        q.chased += 1;
                        q.threats_by_species[pred_at.2.index()] += 1;
                    }
                    q.goal = Goal::Flee;
                    q.flee_until = time.tick + pp.flee_ticks as u64;
                    q.threatened_by = Some(pred_at);
                    q.target = flee_target(q, world, pp);
                    q.replan_at = time.tick + 1;
                }
            }
        }
    }
}

/// Post-loop pass: a predator adjacent to its scavenge target eats once (FR3).
fn scavenge_contacts(store: &mut CreatureStore, time: &Time, pp: &PredationParams) {
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
                Some(k) if !k.alive && k.species.kind() == Kind::Prey => Some((t, k.x, k.y, k.decay)),
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::creatures::{place_founders, CreatureStore, Sex};
    use crate::sim::params::{CreaturesParams, EcologyParams, GeneticsParams, WorldParams};
    use crate::sim::species::SpeciesId;
    use crate::sim::{Params, Sim};

    fn test_world() -> World {
        World::generate(7, &WorldParams::default())
    }

    fn test_creature(x: usize, y: usize) -> Creature {
        Creature {
            id: CreatureId(0),
            species: SpeciesId::Vole,
            name: 0,
            sex: Sex::Female,
            x,
            y,
            born_day: -100,
            generation: 1,
            parents: None,
            genome: SpeciesId::Vole.base_genome(),
            hp: 1.0,
            hunger: 0.3,
            thirst: 0.3,
            energy: 0.8,
            adult: true,
            goal: Goal::Wander,
            target: None,
            replan_at: 0,
            trail: Vec::new(),
            alive: true,
            death: None,
            decay: 0.0,
            mutations: Vec::new(),
            last_water: None,
            move_budget: 0.0,
            path: Vec::new(),
            rest_reason: None,
            pregnant_due: None,
            cooldown_until: 0,
            mate_id: None,
            mother: None,
            offspring: 0,
            kills: 0,
            attempts: 0,
            chased: 0,
            escaped: 0,
            threats_by_species: [0; 6],
            kills_by_species: [0; 6],
            last_kill: None,
            chase_stats: (0, 0),
            chase_longest_year: 0,
            hunt_phase: crate::sim::creatures::HuntPhase::Stalk,
            hunt_target: None,
            chase_start_tick: None,
            hunt_cooldown_until: 0,
            eat_until: None,
            scavenge_target: None,
            flee_until: 0,
            threatened_by: None,
            predation_risk: 0.0,
            migrate_until: 0,
            migrate_target: None,
            path_for: None,
        }
    }

    fn empty_index(world: &World) -> SpatialIndex {
        SpatialIndex::new(world)
    }

    fn plan(c: &mut Creature, idx: &SpatialIndex, w: &World, t: &Time, cp: &CreaturesParams, rng: &mut Rng) {
        replan(c, idx, w, t, cp, &GeneticsParams::default(), &PredationParams::default(), &TickView::empty(), rng);
    }

    fn day_time(hour: u32) -> Time {
        // start_hour 6: hour = (tick + 6) % 24.
        let tick = (hour + 24 - 6) % 24;
        let mut t = Time::new(6, 90, 24, 6, 20);
        t.tick = tick as u64;
        t
    }

    fn all_grass_world() -> World {
        let mut w = World::generate(7, &WorldParams { width: 60, height: 10, ..WorldParams::default() });
        for c in &mut w.cells {
            c.terrain = Terrain::Grass;
            c.vegetation = 0.5;
        }
        w.refresh_shore();
        w
    }

    #[test]
    fn drink_goal_when_thirsty() {
        let w = test_world();
        let mut c = test_creature(75, 20);
        c.thirst = 0.9;
        c.last_water = Some((74, 20));
        let idx = empty_index(&w);
        let mut rng = Rng::new(1);
        plan(&mut c, &idx, &w, &day_time(12), &CreaturesParams::default(), &mut rng);
        assert_eq!(c.goal, Goal::Drink);
    }

    #[test]
    fn graze_hysteresis() {
        let w = test_world();
        let idx = empty_index(&w);
        let mut rng = Rng::new(1);
        let cp = CreaturesParams::default();

        let mut c = test_creature(75, 20);
        c.hunger = 0.6;
        plan(&mut c, &idx, &w, &day_time(12), &cp, &mut rng);
        assert_eq!(c.goal, Goal::Graze, "hunger 0.6 should graze");

        // Still above the exit threshold (0.2) but below the entry (0.5): stay grazing.
        c.hunger = 0.3;
        plan(&mut c, &idx, &w, &day_time(12), &cp, &mut rng);
        assert_eq!(c.goal, Goal::Graze, "hysteresis should keep grazing at hunger 0.3");

        // Below the exit threshold: leave Graze.
        c.hunger = 0.1;
        plan(&mut c, &idx, &w, &day_time(12), &cp, &mut rng);
        assert_ne!(c.goal, Goal::Graze, "hunger 0.1 should stop grazing");
    }

    #[test]
    fn rest_at_night() {
        let w = test_world();
        let mut c = test_creature(75, 20);
        c.energy = 0.8;
        c.hunger = 0.3;
        c.thirst = 0.3;
        let idx = empty_index(&w);
        let mut rng = Rng::new(1);
        plan(&mut c, &idx, &w, &day_time(22), &CreaturesParams::default(), &mut rng);
        assert_eq!(c.goal, Goal::Rest);
        assert_eq!(c.rest_reason, Some(RestReason::Night));
    }

    #[test]
    fn forced_rest_at_zero_energy() {
        let w = test_world();
        let mut c = test_creature(75, 20);
        c.energy = 0.0;
        c.hunger = 0.9;
        c.thirst = 0.9;
        let idx = empty_index(&w);
        let mut rng = Rng::new(1);
        plan(&mut c, &idx, &w, &day_time(12), &CreaturesParams::default(), &mut rng);
        assert_eq!(c.goal, Goal::Rest);
        assert_eq!(c.rest_reason, Some(RestReason::Forced));
    }

    #[test]
    fn fractional_movement_budget() {
        let w = all_grass_world();
        let mut c = test_creature(0, 5);
        c.target = Some((30, 5));
        c.goal = Goal::Wander;
        c.move_budget = 0.9;
        let cp = CreaturesParams { move_speed_base: 0.1, move_speed_per_trait: 1.0, ..CreaturesParams::default() };
        // speed = 0.1 + 1.0*genome.speed(0.45) = 0.55; budget 0.9+0.55 = 1.45 → 1 step.
        move_toward(&mut c, &w, &day_time(12), &cp, &PredationParams::default());
        assert_eq!(c.x, 1, "one step should be taken");
        assert!((c.move_budget - 0.45).abs() < 1e-4, "budget should carry the remainder, got {}", c.move_budget);
    }

    #[test]
    fn needs_tick_rates() {
        let w = test_world();
        let mut c = test_creature(75, 20);
        let cp = CreaturesParams::default();
        let ep = EcologyParams::default();
        let t = day_time(12);
        let before_h = c.hunger;
        let before_t = c.thirst;
        let before_e = c.energy;
        needs(&mut c, &w, &t, &cp, &ep, &GeneticsParams::default());
        let season = ep.season_metabolism.get(&t.season()).copied().unwrap_or(1.0);
        let expect = cp.hunger_per_hour(c.genome.size(), c.genome.metabolism(), season);
        assert!((c.hunger - before_h - expect).abs() < 1e-6);
        assert!((c.thirst - before_t - cp.thirst_per_hour).abs() < 1e-6);
        assert!((c.energy - (before_e - cp.energy_awake_per_hour)).abs() < 1e-6);
    }

    #[test]
    fn hp_death_attribution() {
        let mut w = test_world();
        let mut events = EventRing::new(100);
        let mut tallies = DeathTallies::default();
        let t = day_time(12);

        let mut starved = test_creature(75, 20);
        starved.hp = 0.0;
        starved.hunger = 1.5;
        starved.thirst = 0.2;
        maybe_die(&mut starved, &mut w, &mut events, &t, &mut tallies, &mut Lineage::new());
        assert!(!starved.alive);
        assert_eq!(starved.death.unwrap().cause, Cause::Starved);
        assert_eq!(tallies.starved, 1);

        let mut thirsty = test_creature(76, 20);
        thirsty.hp = 0.0;
        thirsty.hunger = 0.2;
        thirsty.thirst = 1.5;
        maybe_die(&mut thirsty, &mut w, &mut events, &t, &mut tallies, &mut Lineage::new());
        assert_eq!(thirsty.death.unwrap().cause, Cause::Thirst);
        assert_eq!(tallies.thirst, 1);
    }

    #[test]
    fn age_death_at_day_boundary() {
        let mut w = test_world();
        let mut store = CreatureStore::new();
        let mut c = test_creature(75, 20);
        c.born_day = -2000; // age 2000 ≥ any max_age
        let id = store.insert(c);
        let mut events = EventRing::new(100);
        let mut tallies = DeathTallies::default();
        let t = day_time(0);
        day_boundary(&mut store, &mut w, &mut events, &t, &CreaturesParams::default(), &mut tallies, &mut Lineage::new());
        let c = store.get(id).unwrap();
        assert!(!c.alive);
        assert_eq!(c.death.unwrap().cause, Cause::Age);
        assert_eq!(tallies.age, 1);
        assert!(w.carcasses.contains(&(75, 20)));
    }

    #[test]
    fn carcass_decay_frees_slot() {
        let mut w = test_world();
        let mut store = CreatureStore::new();
        let mut c = test_creature(75, 20);
        c.alive = false;
        c.decay = 0.99;
        c.death = Some(Death { cause: Cause::Starved, day: 0, killer: None, chase_ticks: 0 });
        w.carcasses.push((75, 20));
        let id = store.insert(c);
        let mut events = EventRing::new(100);
        let mut tallies = DeathTallies::default();
        day_boundary(&mut store, &mut w, &mut events, &day_time(0), &CreaturesParams::default(), &mut tallies, &mut Lineage::new());
        assert!(store.get(id).is_none(), "decayed carcass slot should be freed");
        assert!(!w.carcasses.contains(&(75, 20)));
    }

    #[test]
    fn trail_cap() {
        let w = all_grass_world();
        let mut c = test_creature(0, 5);
        c.target = Some((40, 5));
        c.goal = Goal::Wander;
        let cp = CreaturesParams { move_speed_base: 2.0, move_speed_per_trait: 0.0, trail_len: 12, ..CreaturesParams::default() };
        // Walk far enough to exceed the trail cap.
        for _ in 0..20 {
            move_toward(&mut c, &w, &day_time(12), &cp, &PredationParams::default());
        }
        assert!(c.trail.len() <= cp.trail_len, "trail {} exceeds cap {}", c.trail.len(), cp.trail_len);
        assert_eq!(c.trail.len(), cp.trail_len);
    }

    #[test]
    fn den_creation_capped() {
        let mut w = test_world();
        let mut events = EventRing::new(100);
        let mut rng = Rng::new(1);
        // Force den creation (chance 1.0), cap 1 per region.
        let cp = CreaturesParams { den_create_chance_per_rest_hour: 1.0, max_dens_per_region: 1, ..CreaturesParams::default() };
        let t = day_time(2); // night → rest

        // Resting creature on a bare Dirt cell (vegetation < 0.2).
        let mut c = test_creature(40, 5);
        c.goal = Goal::Rest;
        c.rest_reason = Some(RestReason::Night);
        c.target = None;
        w.cell_mut(40, 5).terrain = Terrain::Dirt;
        w.cell_mut(40, 5).vegetation = 0.0;
        maybe_make_den(&mut c, &mut w, &mut events, &t, &cp, &mut rng);
        assert_eq!(w.dens.len(), 1);

        // Second resting creature in the same region: capped.
        let mut c2 = test_creature(41, 5);
        c2.goal = Goal::Rest;
        c2.rest_reason = Some(RestReason::Night);
        c2.target = None;
        w.cell_mut(41, 5).terrain = Terrain::Dirt;
        w.cell_mut(41, 5).vegetation = 0.0;
        maybe_make_den(&mut c2, &mut w, &mut events, &t, &cp, &mut rng);
        assert_eq!(w.dens.len(), 1, "region den cap should hold");
    }

    #[test]
    fn path_search_rounds_an_obstacle() {
        let mut w = all_grass_world();
        // A rock wall at x = 10 spanning rows 2..=8 with a gap at row 9.
        for y in 2..=8 {
            w.cell_mut(10, y).terrain = Terrain::Rock;
        }
        let mut c = test_creature(8, 5);
        c.target = Some((12, 5));
        c.goal = Goal::Drink;
        let cp = CreaturesParams { move_speed_base: 1.0, move_speed_per_trait: 0.0, ..CreaturesParams::default() };
        for _ in 0..30 {
            move_toward(&mut c, &w, &day_time(12), &cp, &PredationParams::default());
            if (c.x, c.y) == (12, 5) {
                break;
            }
        }
        assert_eq!((c.x, c.y), (12, 5), "creature should route around the wall via the gap");
        assert!(c.target.is_some(), "target is kept for non-wander goals");
    }

    #[test]
    fn unreachable_target_is_dropped() {
        let mut w = all_grass_world();
        // Fully enclose the target.
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx != 0 || dy != 0 {
                    w.cell_mut((30 + dx) as usize, (5 + dy) as usize).terrain = Terrain::Rock;
                }
            }
        }
        let mut c = test_creature(20, 5);
        c.target = Some((30, 5));
        c.goal = Goal::Drink;
        c.last_water = Some((30, 5));
        let cp = CreaturesParams { move_speed_base: 1.0, move_speed_per_trait: 0.0, ..CreaturesParams::default() };
        for _ in 0..40 {
            move_toward(&mut c, &w, &day_time(12), &cp, &PredationParams::default());
        }
        assert!(c.target.is_none(), "unreachable target must be dropped");
        assert!(c.last_water.is_none(), "an unreachable water memory is forgotten");
    }

    #[test]
    fn movement_never_impassable() {
        let mut sim = Sim::new(42, Params::default());
        for _ in 0..24 * 120 {
            sim.step();
        }
        for c in sim.creatures.living() {
            let cell = sim.world.cell(c.x, c.y);
            assert!(cell.terrain.walkable(), "creature {} on {:?}", c.id.0, cell.terrain);
        }
    }

    #[test]
    fn founders_are_placed() {
        let sim = Sim::new(42, Params::default());
        let total: u32 = sim.params.creatures.initial_counts.values().sum();
        assert_eq!(sim.creatures.len_living(), total as usize);
        let _ = place_founders;
    }

    #[test]
    fn pressure_decay() {
        let mut w = test_world();
        let mut store = CreatureStore::new();
        let mut events = EventRing::new(10);
        let mut tallies = DeathTallies::default();
        w.cell_mut(50, 10).prey_pressure = 1.0;
        w.cell_mut(50, 10).pred_pressure = 0.0;
        let cp = CreaturesParams::default();
        day_boundary(&mut store, &mut w, &mut events, &day_time(0), &cp, &mut tallies, &mut Lineage::new());
        assert!((w.cell(50, 10).prey_pressure - cp.pressure_decay_per_day).abs() < 1e-6);
    }

    #[test]
    fn pressure_clamped() {
        let mut w = test_world();
        let cp = CreaturesParams { pressure_per_creature_tick: 1.0, ..CreaturesParams::default() };
        let mut wolf = test_creature(75, 20);
        wolf.species = SpeciesId::Wolf;
        pressure(&mut wolf, &mut w, &cp);
        assert_eq!(w.cell(75, 20).pred_pressure, 1.0, "predator traffic clamps at 1.0");
        let mut vole = test_creature(75, 20);
        vole.species = SpeciesId::Vole;
        pressure(&mut vole, &mut w, &cp);
        assert_eq!(w.cell(75, 20).prey_pressure, 1.0, "prey traffic clamps at 1.0");
    }

    #[test]
    fn nocturnal_rest_by_day() {
        let w = test_world();
        let idx = empty_index(&w);
        let mut rng = Rng::new(1);
        let cp = CreaturesParams::default();
        // Fox (nocturnal) rests during the day.
        let mut fox = test_creature(75, 20);
        fox.species = SpeciesId::Fox;
        plan(&mut fox, &idx, &w, &day_time(12), &cp, &mut rng);
        assert_eq!(fox.goal, Goal::Rest, "nocturnal fox rests by day");
        // Wolf (diurnal) rests at night.
        let mut wolf = test_creature(75, 20);
        wolf.species = SpeciesId::Wolf;
        plan(&mut wolf, &idx, &w, &day_time(22), &cp, &mut rng);
        assert_eq!(wolf.goal, Goal::Rest, "diurnal wolf rests at night");
    }

    #[test]
    fn flee_query_is_predator_first() {
        let w = test_world();
        let mut store = CreatureStore::new();
        let mut wolf = test_creature(75, 20);
        wolf.species = SpeciesId::Wolf;
        wolf.genome = SpeciesId::Wolf.base_genome();
        wolf.hunger = 0.9; // hungry wolves are a danger within chase range
        let mut vole = test_creature(76, 20);
        vole.species = SpeciesId::Vole;
        vole.genome = SpeciesId::Vole.base_genome();
        store.insert(wolf);
        store.insert(vole);
        let mut idx = SpatialIndex::new(&w);
        idx.rebuild(&store, &w);
        // A second, nearer wolf with a higher id: the nearest detected threat wins
        // and both count toward predation risk.
        let mut near = test_creature(76, 20); // same cell → distance 0 (the first wolf is 0.5 away)
        near.species = SpeciesId::Wolf;
        near.genome = SpeciesId::Wolf.base_genome();
        near.genome.0[5] = 0.0;
        near.hunger = 0.9;
        store.insert(near);
        let mut idx = SpatialIndex::new(&w);
        idx.rebuild(&store, &w);
        mark_threats(&mut store, &idx, &w, 0, &PredationParams::default());
        let vole_id = store.living().find(|c| c.species == SpeciesId::Vole).unwrap().id;
        let v = store.get(vole_id).unwrap();
        assert!(v.threatened_by.is_some(), "nearby vole should be threatened by a wolf");
        assert_eq!(v.threatened_by.map(|t| (t.0, t.1)), Some((76, 20)), "nearest detected predator drives the away-vector");
        assert!(v.predation_risk >= 0.5 * 2.0 / 3.0 - 1e-6, "both predators in range count: {}", v.predation_risk);
    }

    /// Run one full creature tick over `store` in `w` at noon.
    fn tick_once(store: &mut CreatureStore, w: &mut World, time: &Time, pp: &PredationParams) -> EventRing {
        let mut idx = SpatialIndex::new(w);
        idx.rebuild(store, w);
        let mut events = EventRing::new(64);
        let mut tallies = DeathTallies::default();
        let mut noted = false;
        tick_creatures(
            store,
            &idx,
            w,
            &mut events,
            time,
            &CreaturesParams::default(),
            &EcologyParams::default(),
            &GeneticsParams::default(),
            pp,
            &mut Rng::new(1),
            &mut tallies,
            &mut Lineage::new(),
            &mut noted,
        );
        events
    }

    #[test]
    fn flee_reacts_within_one_tick() {
        let mut w = all_grass_world();
        let mut store = CreatureStore::new();
        let mut wolf = test_creature(20, 5);
        wolf.species = SpeciesId::Wolf;
        wolf.genome = SpeciesId::Wolf.base_genome();
        wolf.genome.0[5] = 0.0; // no camouflage
        wolf.hunger = 0.9; // hungry: a danger within chase range even before it hunts
        let mut hare = test_creature(22, 5); // dx 2 → distance 1
        hare.species = SpeciesId::Hare;
        hare.genome = SpeciesId::Hare.base_genome();
        hare.genome.0[2] = 0.6; // sense > wolf camouflage
        store.insert(wolf);
        let hare_id = store.insert(hare);
        let pp = PredationParams::default();
        tick_once(&mut store, &mut w, &day_time(12), &pp);
        let h = store.get(hare_id).unwrap();
        assert_eq!(h.goal, Goal::Flee, "detection pre-empts every goal within the tick");
        assert_eq!(h.chased, 1);
        assert_eq!(h.threats_by_species[SpeciesId::Wolf.index()], 1);
        assert!(h.x > 22, "moved away along the predator→prey vector: x = {}", h.x);
    }

    #[test]
    fn flee_costs_energy() {
        let w = all_grass_world();
        let cp = CreaturesParams { move_speed_base: 2.0, move_speed_per_trait: 0.0, ..CreaturesParams::default() };
        let pp = PredationParams::default();
        let mut wander = test_creature(0, 5);
        wander.target = Some((30, 5));
        wander.goal = Goal::Wander;
        let e0 = wander.energy;
        move_toward(&mut wander, &w, &day_time(12), &cp, &pp);
        let wander_cost = e0 - wander.energy;
        assert!(wander_cost > 0.0);

        let mut flee = test_creature(0, 5);
        flee.target = Some((30, 5));
        flee.goal = Goal::Flee;
        let e0 = flee.energy;
        move_toward(&mut flee, &w, &day_time(12), &cp, &pp);
        let flee_cost = e0 - flee.energy;
        assert_eq!(flee.x, wander.x, "same steps taken");
        assert!((flee_cost - wander_cost * pp.flee_energy_factor).abs() < 1e-6, "flee steps cost flee_energy_factor × move_cost_energy");
    }

    /// A walkable land cell inside region `ri` of `w`.
    fn land_cell_in(w: &World, ri: usize) -> (usize, usize) {
        let r = &w.regions[ri];
        for y in r.2..r.4 {
            for x in r.1..r.3 {
                let t = w.cell(x, y).terrain;
                if t.walkable() && !t.is_water() {
                    return (x, y);
                }
            }
        }
        panic!("region {ri} has no land");
    }

    #[test]
    fn migration_destination() {
        let w = test_world();
        let mut store = CreatureStore::new();
        let (x, y) = land_cell_in(&w, 0);
        for _ in 0..5 {
            let mut h = test_creature(x, y);
            h.species = SpeciesId::Hare;
            store.insert(h);
        }
        let mut events = EventRing::new(16);
        let time = day_time(0);
        migrate_group(&mut store, &w, &mut events, &time, SpeciesId::Hare, 0);
        let ev = events.iter().find(|e| e.kind == EventKind::Migration).expect("one Migration event");
        assert!(ev.text.to_lowercase().contains("herd of 5 hares"), "{}", ev.text);
        let dest: usize = ev.detail.split('>').nth(1).unwrap().parse().unwrap();
        assert!(regions_adjacent(&w.regions[0], &w.regions[dest]), "destination shares an edge with the origin");
        // The destination maximises mean_vegetation × (1 − mean_pred_pressure).
        let score = |ri: usize| crate::sim::ecology::region_land_veg_mean(&w, &w.regions[ri]) * (1.0 - mean_pred_pressure(&w, &w.regions[ri]));
        for (ri, r) in w.regions.iter().enumerate() {
            if ri != 0 && regions_adjacent(&w.regions[0], r) {
                assert!(score(dest) >= score(ri), "dest {dest} beats {ri}");
            }
        }
        for h in store.living() {
            assert_eq!(h.goal, Goal::Migrate);
            let (tx, ty) = h.migrate_target.expect("target cell");
            assert_eq!(w.region_index(tx, ty), dest);
            assert!(w.cell(tx, ty).terrain.walkable());
            assert_eq!(h.migrate_until, time.tick + 2 * time.ticks_per_day as u64, "for up to 2 days");
        }
        assert_eq!(ev.pos, Some(((w.regions[0].1 + w.regions[0].3) / 2, (w.regions[0].2 + w.regions[0].4) / 2)), "pos = origin region centre");
    }

    /// Four wolves in region 0 and no prey anywhere: the predator trigger holds every day.
    fn hungry_pack() -> (World, CreatureStore) {
        let w = test_world();
        let mut store = CreatureStore::new();
        let (x, y) = land_cell_in(&w, 0);
        for _ in 0..4 {
            let mut c = test_creature(x, y);
            c.species = SpeciesId::Wolf;
            c.genome = SpeciesId::Wolf.base_genome();
            store.insert(c);
        }
        (w, store)
    }

    fn run_migration_days(w: &World, store: &mut CreatureStore, events: &mut EventRing, time: &mut Time, cd: &mut [u64; 48], db: &mut [u32; 48], days: u32) {
        let pp = PredationParams::default();
        for _ in 0..days {
            time.tick += time.ticks_per_day as u64;
            migration_daily(store, w, events, time, &EcologyParams::default(), &pp, cd, db);
        }
    }

    #[test]
    fn predator_migration_on_low_prey() {
        let (w, mut store) = hungry_pack();
        let mut events = EventRing::new(16);
        let mut time = day_time(0);
        let (mut cd, mut db) = ([0u64; 48], [0u32; 48]);
        let pp = PredationParams::default();
        run_migration_days(&w, &mut store, &mut events, &mut time, &mut cd, &mut db, pp.migrate_days - 1);
        assert!(!events.iter().any(|e| e.kind == EventKind::Migration), "needs migrate_days consecutive days");
        run_migration_days(&w, &mut store, &mut events, &mut time, &mut cd, &mut db, 1);
        let ev: Vec<_> = events.iter().filter(|e| e.kind == EventKind::Migration).collect();
        assert_eq!(ev.len(), 1);
        assert!(ev[0].text.to_lowercase().contains("pack of 4 wolves"), "{}", ev[0].text);
        assert!(store.living().all(|c| c.goal == Goal::Migrate));
    }

    #[test]
    fn migration_cooldown() {
        let (w, mut store) = hungry_pack();
        let mut events = EventRing::new(16);
        let mut time = day_time(0);
        let (mut cd, mut db) = ([0u64; 48], [0u32; 48]);
        let pp = PredationParams::default();
        run_migration_days(&w, &mut store, &mut events, &mut time, &mut cd, &mut db, pp.migrate_days);
        // Keep the pack in region 0 so the trigger keeps holding.
        let (x, y) = land_cell_in(&w, 0);
        let count = |events: &EventRing| events.iter().filter(|e| e.kind == EventKind::Migration).count();
        assert_eq!(count(&events), 1);
        for _ in 0..(pp.migrate_cooldown_days - 1) {
            for c in store.living_mut() {
                c.x = x;
                c.y = y;
            }
            run_migration_days(&w, &mut store, &mut events, &mut time, &mut cd, &mut db, 1);
        }
        assert_eq!(count(&events), 1, "no second migration within migrate_cooldown_days");
        for _ in 0..(pp.migrate_days + 1) {
            for c in store.living_mut() {
                c.x = x;
                c.y = y;
            }
            run_migration_days(&w, &mut store, &mut events, &mut time, &mut cd, &mut db, 1);
        }
        assert_eq!(count(&events), 2, "migrates again once the cooldown has passed");
    }
}
