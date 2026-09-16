//! Walkability, wandering, cohesion, steering and BFS pathfinding.

use crate::sim::creatures::{
    Creature, Goal, HuntPhase,
};
use crate::sim::genetics::{self, TickView, OFF8};
use crate::sim::geom;
use crate::sim::params::{CreaturesParams, GeneticsParams, PredationParams, SocialParams};
use crate::sim::rng::Rng;
use crate::sim::time::Time;
use crate::sim::world::World;
use super::find_walkable_near;
use super::perception::Kin;

/// Planned steps kept after a path search; the greedy step resumes afterwards.
const PATH_KEEP: usize = 24;

/// Padding (cells) around the start/target bounding box searched by `bfs_path`.
const PATH_PAD_X: i32 = 16;

const PATH_PAD_Y: i32 = 8;

/// In bounds and walkable.
fn walkable_cell(world: &World, x: i32, y: i32) -> bool {
    world.in_bounds(x, y) && world.cell(crate::cast!(x => usize), crate::cast!(y => usize)).terrain.walkable()
}

const fn random_dir(rng: &mut Rng) -> (i32, i32) {
    let i = rng.below(8);
    OFF8[i]
}

/// Wander, with C8 herd/pack cohesion (FR2). Below `cohesion_min` sociality — or
/// with no kin in sight — this is exactly the pre-C8 random walk.
pub(super) fn wander(
    c: &mut Creature,
    world: &World,
    time: &Time,
    view: &TickView,
    gp: &GeneticsParams,
    sp: &SocialParams,
    rng: &mut Rng,
    kin: Option<Kin>,
) {
    c.goal = Goal::Wander;
    c.rest_reason = None;
    // C4 FR4: juveniles stay near their living mother.
    if !c.adult {
        if let Some(t) = genetics::follow_target(c, view, world, time, gp, rng) {
            c.target = Some(t);
            return;
        }
    }
    // C8 FR2: social creatures join a group that is below their preferred size,
    // and disperse from one that is above it, so herds stay bounded.
    if let Some(t) = kin.and_then(|k| cohesion_target(c, world, sp, rng, k)) {
        c.target = Some(t);
        return;
    }
    // Keep the current heading with p = 0.7, else choose a new direction.
    let mut dir = random_dir(rng);
    if let Some((tx, ty)) = c.target {
        if rng.chance(0.7) {
            let dx = (crate::cast!(tx => i32) - crate::cast!(c.x => i32)).signum();
            let dy = (crate::cast!(ty => i32) - crate::cast!(c.y => i32)).signum();
            if dx != 0 || dy != 0 {
                dir = (dx, dy);
            }
        }
    }
    let steps = 4 + crate::cast!(rng.below(5) => i32); // 4..=8
    let nx = crate::cast!(c.x => i32) + dir.0 * steps;
    let ny = crate::cast!(c.y => i32) + dir.1 * steps;
    if world.in_bounds(nx, ny) && world.cell(crate::cast!(nx => usize), crate::cast!(ny => usize)).terrain.walkable() {
        c.target = Some((crate::cast!(nx => usize), crate::cast!(ny => usize)));
    } else {
        c.target = find_walkable_near(c.x, c.y, world);
    }
}

/// Where a social creature should head to herd (C8 FR2), or `None` to wander
/// normally: toward the kin centroid when the group is under its preferred size,
/// away from it when the group is over 1.5x that size, and nowhere in between.
fn cohesion_target(c: &Creature, world: &World, sp: &SocialParams, rng: &mut Rng, kin: Kin) -> Option<(usize, usize)> {
    let sociality = c.genome.sociality();
    if kin.count == 0 || sociality < sp.cohesion_min {
        return None;
    }
    let preferred = sp.preferred_group(sociality);
    let (kx, ky) = kin.centroid();
    if f32::from(kin.count) < preferred {
        // Join: with probability `sociality`, aim at a walkable cell near the kin
        // centroid (the `follow_target` pattern).
        if !rng.chance(sociality) {
            return None;
        }
        let (dx, dy) = (crate::cast!(rng.below(5) => i32) - 2, crate::cast!(rng.below(5) => i32) - 2);
        let (nx, ny) = (crate::cast!(kx => i32) + dx, crate::cast!(ky => i32) + dy);
        if walkable_cell(world, nx, ny) {
            return Some((crate::cast!(nx => usize), crate::cast!(ny => usize)));
        }
        return find_walkable_near(kx, ky, world);
    }
    if f32::from(kin.count) > 1.5 * preferred {
        // Disperse: head away from the centroid.
        let (dx, dy) = ((crate::cast!(c.x => i32) - crate::cast!(kx => i32)).signum(), (crate::cast!(c.y => i32) - crate::cast!(ky => i32)).signum());
        if dx == 0 && dy == 0 {
            return None;
        }
        let (nx, ny) = (crate::cast!(c.x => i32) + dx * 5, crate::cast!(c.y => i32) + dy * 5);
        if walkable_cell(world, nx, ny) {
            return Some((crate::cast!(nx => usize), crate::cast!(ny => usize)));
        }
    }
    None
}

/// The next cell to step to: the planned path step when still valid, else a
/// greedy step, else a BFS detour (FR6 fallback around an obstacle).
fn next_move(c: &mut Creature, world: &World, target: (usize, usize)) -> Option<(usize, usize)> {
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
    planned.or_else(|| step_toward(c.x, c.y, target, world)).or_else(|| {
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
    })
}

/// The per-tick move speed: base plus the speed trait, scaled by adulthood and
/// the caller's `speed_factor` (C7 sickness), then by the wary tier and the
/// hunting chase bonus (FR4).
fn move_speed(c: &Creature, cp: &CreaturesParams, pp: &PredationParams, speed_factor: f32) -> f32 {
    let mut speed = cp.move_speed_base + cp.move_speed_per_trait * c.genome.speed();
    speed = if c.adult { speed } else { speed * 0.75 };
    // C7 FR6: sickness slows the animal; the chase bonus below is added after.
    speed *= speed_factor;
    // C5 FR5b: the wary tier is a slow backing-off, not a sprint.
    if c.goal == Goal::Wary {
        speed *= pp.wary_speed_factor;
    }
    // FR4: a hunting predator moves with the chase speed bonus.
    if c.goal == Goal::Hunt && c.hunt_phase != HuntPhase::Eat {
        speed += pp.chase_speed_bonus;
    }
    speed
}

pub(super) fn move_toward(c: &mut Creature, world: &World, time: &Time, cp: &CreaturesParams, pp: &PredationParams, speed_factor: f32) {
    if c.target.is_none() {
        c.path.clear();
        return;
    }
    c.move_budget = (c.move_budget + move_speed(c, cp, pp, speed_factor)).min(2.0);
    // A path searched for an earlier target (a flee vector, a fled prey) must
    // not be followed toward the new one.
    if !c.path.is_empty() && c.path_for != c.target {
        c.path.clear();
    }
    loop {
        if c.move_budget < 1.0 {
            break;
        }
        let Some(target) = c.target else { break };
        if (c.x, c.y) == target {
            c.path.clear();
            if c.goal == Goal::Wander {
                c.target = None; // waypoint reached
            }
            break;
        }
        if let Some((nx, ny)) = next_move(c, world, target) {
            c.x = nx;
            c.y = ny;
            c.energy -= if c.goal == Goal::Flee { cp.move_cost_energy * pp.flee_energy_factor } else { cp.move_cost_energy };
            c.move_budget -= 1.0;
            // Marsh is slow going: the step costs extra budget, so a chase
            // through reeds favours whoever is lighter on the ground.
            if world.cell(nx, ny).terrain == crate::sim::world::Terrain::Marsh {
                c.move_budget -= cp.marsh_step_cost;
            }
            c.trail.push((nx, ny));
            if c.trail.len() > cp.trail_len {
                c.trail.remove(0);
            }
            if (c.x, c.y) == target && c.goal == Goal::Wander {
                c.target = None;
                c.path.clear();
                break;
            }
        } else {
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
        let (nx, ny) = (crate::cast!(x => i32) + dx, crate::cast!(y => i32) + dy);
        if world.in_bounds(nx, ny) && world.cell(crate::cast!(nx => usize), crate::cast!(ny => usize)).terrain.walkable() {
            let d = geom::dist(crate::cast!(nx => usize), crate::cast!(ny => usize), tx, ty);
            if d < best_d {
                best_d = d;
                best = Some((crate::cast!(nx => usize), crate::cast!(ny => usize)));
            }
        }
    }
    if best_d < cur_d {
        return best;
    }
    None
}

/// Bounded breadth-first search over walkable cells inside the padded bounding
/// box of `from` and `target`. Returns the path (excluding `from`, including
/// `target`) truncated to `keep` steps, or `None` when unreachable in the box.
fn bfs_path(from: (usize, usize), target: (usize, usize), world: &World, keep: usize) -> Option<Vec<(usize, usize)>> {
    let (fx, fy) = (crate::cast!(from.0 => i32), crate::cast!(from.1 => i32));
    let (tx, ty) = (crate::cast!(target.0 => i32), crate::cast!(target.1 => i32));
    if !world.in_bounds(tx, ty) {
        return None;
    }
    let x0 = (fx.min(tx) - PATH_PAD_X).max(0);
    let y0 = (fy.min(ty) - PATH_PAD_Y).max(0);
    let x1 = (fx.max(tx) + PATH_PAD_X + 1).min(crate::cast!(world.width => i32));
    let y1 = (fy.max(ty) + PATH_PAD_Y + 1).min(crate::cast!(world.height => i32));
    let bw = crate::cast!((x1 - x0) => usize);
    let bh = crate::cast!((y1 - y0) => usize);
    let idx = |x: i32, y: i32| (crate::cast!((y - y0) => usize)) * bw + crate::cast!((x - x0) => usize);
    // Parent index per box cell; u32::MAX = unvisited.
    let mut parent = vec![u32::MAX; bw * bh];
    let mut queue: Vec<(i32, i32)> = Vec::with_capacity((bw * bh).div_euclid(4));
    let start = idx(fx, fy);
    parent[start] = crate::cast!(start => u32);
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
            if !world.cell(crate::cast!(nx => usize), crate::cast!(ny => usize)).terrain.walkable() {
                continue;
            }
            parent[ni] = crate::cast!(idx(x, y) => u32);
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
        let cx = crate::cast!((cur % bw) => i32) + x0;
        let cy = crate::cast!((cur.div_euclid(bw)) => i32) + y0;
        full.push((crate::cast!(cx => usize), crate::cast!(cy => usize)));
        cur = crate::cast!(parent[cur] => usize);
    }
    full.reverse();
    full.truncate(keep);
    Some(full)
}
