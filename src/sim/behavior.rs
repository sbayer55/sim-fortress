//! Creature behaviour: perception, goal selection (with hysteresis), fractional
//! movement, grazing/drinking/resting, mating (C4), den creation, death and the
//! day boundary.

use crate::sim::creatures::{Cause, Creature, CreatureId, CreatureStore, Death, DeathTallies, Goal, RestReason};
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::genetics::{self, TickView, OFF8};
use crate::sim::geom;
use crate::sim::lineage::Lineage;
use crate::sim::params::{CreaturesParams, EcologyParams, GeneticsParams};
use crate::sim::rng::Rng;
use crate::sim::spatial::SpatialIndex;
use crate::sim::time::Time;
use crate::sim::world::{Terrain, World};

/// What a creature can see at a replan. Creature ids feed mate selection (C4)
/// and predation (C5).
pub struct Perception {
    pub nearest_water: Option<(usize, usize)>,
    pub best_graze: Option<((usize, usize), f32)>,
    pub nearest_den: Option<(usize, usize)>,
    pub creatures: Vec<CreatureId>,
}

/// Advance every living creature one tick, in slot order (FR9), then run the
/// two C4 passes: consummation of adjacent mating pairs and deliveries.
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
    rng: &mut Rng,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
    soft_cap_noted: &mut bool,
) {
    let view = TickView::build(store, time, world, gp);
    for c in store.living_mut() {
        update_one(c, spatial, world, events, time, cp, ep, gp, &view, rng, tallies, lineage);
    }
    genetics::consummate(store, time, gp, events, &view, soft_cap_noted);
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
            kill(c, Cause::Age, world, events, time, tallies, lineage);
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

    // 3. Pressure decay (FR8). `pred_pressure` stays 0 until C5.
    for cell in &mut world.cells {
        cell.prey_pressure *= cp.pressure_decay_per_day;
        cell.pred_pressure *= cp.pressure_decay_per_day;
    }
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
    view: &TickView,
    rng: &mut Rng,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
) {
    // Goal satisfied → replan now.
    if goal_satisfied(c, time) {
        c.replan_at = time.tick;
    }
    // Replan when due.
    if time.tick >= c.replan_at {
        replan(c, spatial, world, time, cp, gp, view, rng);
    }
    // Move toward the target.
    move_toward(c, world, time, cp);
    // Act on the goal at the current location.
    act(c, world, events, time, cp, rng);
    // Needs and hp.
    needs(c, world, time, cp, ep, gp);
    // Death.
    maybe_die(c, world, events, time, tallies, lineage);
    // Pressure.
    pressure(c, world, cp);
}

fn goal_satisfied(c: &Creature, time: &Time) -> bool {
    match c.goal {
        Goal::Drink => c.thirst <= 0.1,
        Goal::Graze => c.hunger <= 0.2,
        Goal::Rest => match c.rest_reason {
            Some(RestReason::Forced) => c.energy >= 0.3,
            Some(RestReason::Energy) => c.energy >= 0.9,
            Some(RestReason::Night) => !time.is_night(),
            None => true,
        },
        Goal::Wander => c.target.is_none(),
        // Mated (cooldown just set) or the partner is gone.
        Goal::Mate => c.mate_id.is_none() || c.cooldown_until > time.tick,
        _ => false,
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
                    cur.vegetation / (1.0 + 0.0)
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
                // Nothing edible in sight. Graze in place if this cell has any
                // vegetation, otherwise keep moving (a bare patch wider than the
                // sense range would otherwise starve the creature where it stands).
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
    let rest_stay = c.goal == Goal::Rest && !goal_satisfied(c, time);
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
    Perception { nearest_water, best_graze, nearest_den, creatures }
}

fn move_toward(c: &mut Creature, world: &World, time: &Time, cp: &CreaturesParams) {
    if c.target.is_none() {
        c.path.clear();
        return;
    }
    let speed = cp.move_speed_base + cp.move_speed_per_trait * c.genome.speed();
    let speed = if c.adult { speed } else { speed * 0.75 };
    c.move_budget = (c.move_budget + speed).min(2.0);
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
                c.energy -= cp.move_cost_energy;
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
        kill(c, cause, world, events, time, tallies, lineage);
    }
}

fn pressure(c: &mut Creature, world: &mut World, cp: &CreaturesParams) {
    if c.species.kind() == crate::sim::species::Kind::Prey {
        let cell = world.cell_mut(c.x, c.y);
        cell.prey_pressure = (cell.prey_pressure + cp.pressure_per_creature_tick).min(1.0);
    }
}

fn is_resting(c: &Creature) -> bool {
    c.goal == Goal::Rest && (c.target.is_none() || c.target == Some((c.x, c.y)))
}

fn in_den(c: &Creature, world: &World) -> bool {
    world.dens.iter().any(|&(x, y)| x == c.x && y == c.y)
}

/// Mark a creature dead, add a carcass, record the tally and emit the event.
#[allow(clippy::too_many_arguments)]
fn kill(c: &mut Creature, cause: Cause, world: &mut World, events: &mut EventRing, time: &Time, tallies: &mut DeathTallies, lineage: &mut Lineage) {
    c.alive = false;
    c.hp = 0.0;
    c.target = None;
    c.pregnant_due = None;
    c.death = Some(Death { cause, day: time.day_index() as u32, killer: None, chase_ticks: 0 });
    world.carcasses.push((c.x, c.y));
    tallies.deaths[c.species.index()] += 1;
    lineage.record_death(c.id, time.day_index() as u32);

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
        Cause::Predation => format!("{} {} was killed in {}", c.name_str(), c.tag(), region),
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
        }
    }

    fn empty_index(world: &World) -> SpatialIndex {
        SpatialIndex::new(world)
    }

    fn plan(c: &mut Creature, idx: &SpatialIndex, w: &World, t: &Time, cp: &CreaturesParams, rng: &mut Rng) {
        replan(c, idx, w, t, cp, &GeneticsParams::default(), &TickView::empty(), rng);
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
        move_toward(&mut c, &w, &day_time(12), &cp);
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
            move_toward(&mut c, &w, &day_time(12), &cp);
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
            move_toward(&mut c, &w, &day_time(12), &cp);
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
            move_toward(&mut c, &w, &day_time(12), &cp);
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
        let vole = sim.params.creatures.initial_counts.get(&SpeciesId::Vole).copied().unwrap_or(0);
        let hare = sim.params.creatures.initial_counts.get(&SpeciesId::Hare).copied().unwrap_or(0);
        let deer = sim.params.creatures.initial_counts.get(&SpeciesId::Deer).copied().unwrap_or(0);
        assert_eq!(sim.creatures.len_living(), (vole + hare + deer) as usize);
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
}
