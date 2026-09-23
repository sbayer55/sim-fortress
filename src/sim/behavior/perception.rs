//! Perception and social-kin summaries.

use crate::sim::creatures::{
    Creature, CreatureId,
};
use crate::sim::genetics::TickView;
use crate::sim::geom;
use crate::sim::params::{CreaturesParams, DietParams, SocialParams};
use crate::sim::spatial::SpatialIndex;
use crate::sim::world::World;
use super::territory::Scent;

/// What a creature can see at a replan. Creature ids feed mate selection (C4)
/// and predation (C5).
#[derive(Debug)]
pub struct Perception {
    pub nearest_water: Option<(usize, usize)>,
    pub best_graze: Option<((usize, usize), f32)>,
    pub nearest_den: Option<(usize, usize)>,
    /// Highest `prey_pressure` cell within sense range (predator Patrol, C5),
    /// discounted by foreign scent for a solitary predator (C5 FR13).
    pub best_patrol: Option<(usize, usize)>,
    pub creatures: Vec<CreatureId>,
}

/// Same-species neighbours visible at this replan, for sociality (C8 FR2). Built
/// from the perception id list and the per-tick `TickView`, so it costs no extra
/// spatial query (`docs/PERFORMANCE.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Kin {
    pub count: u8,
    /// Centroid of the neighbours, rounded on use.
    pub cx: f32,
    pub cy: f32,
}

impl Kin {
    pub fn centroid(self) -> (usize, usize) {
        (crate::cast!(self.cx.round().max(0.0) => usize), crate::cast!(self.cy.round().max(0.0) => usize))
    }
}

/// Summarize the same-species living neighbours among `candidates` (the peers are
/// living by construction, and the creature itself is skipped).
pub(super) fn kin_summary(c: &Creature, candidates: &[CreatureId], view: &TickView) -> Kin {
    // `perceive` feeds the already-sorted output of `spatial.within`, so the sort
    // (and its allocation) only happens for a caller that hands over a raw list.
    let mut sorted: Vec<CreatureId>;
    let ids: &[CreatureId] = if candidates.windows(2).all(|w| w[0] <= w[1]) {
        candidates
    } else {
        sorted = candidates.to_vec();
        sorted.sort_unstable();
        &sorted
    };
    let (mut count, mut sx, mut sy) = (0u32, 0.0f32, 0.0f32);
    for &id in ids {
        if id == c.id {
            continue;
        }
        let Some(p) = view.get(id) else { continue };
        if p.species != c.species {
            continue;
        }
        count += 1;
        sx += crate::cast!(p.x => f32);
        sy += crate::cast!(p.y => f32);
    }
    if count == 0 {
        Kin::default()
    } else {
        Kin { count: crate::cast!(count.min(u32::from(u8::MAX)) => u8), cx: sx / crate::cast!(count => f32), cy: sy / crate::cast!(count => f32) }
    }
}

pub(super) fn perceive(c: &Creature, spatial: &SpatialIndex, world: &World, cp: &CreaturesParams, view: &TickView, sp: &SocialParams, diet: &DietParams, scent: &Scent<'_>) -> (Perception, Kin) {
    // Diet breadth: edibility per terrain, built once here so the cell loop
    // below is one multiply per cell and never touches the params map.
    let edible = diet.table(c.genome.diet_breadth());
    let r = c.sense_cells(); // u16
    let r_i = i32::from(r);
    let r_f = f32::from(r);
    let (cx, cy) = (c.x, c.y);
    let y0 = crate::cast!((crate::cast!(cy => i32) - r_i).max(0) => usize);
    let y1 = crate::cast!(((crate::cast!(cy => i32) + r_i + 1).min(crate::cast!(world.height => i32))).max(0) => usize);

    // C8 FR2: the kin summary comes from the same spatial query the perception
    // already issues, and biases grazing so herds strip the same cells.
    let creatures = spatial.within(cx, cy, r);
    let kin = kin_summary(c, &creatures, view);
    let sociality = c.sociality();
    let herding = sp.herding(sociality, kin.count);
    let (kx, ky) = kin.centroid();

    let mut nearest_water: Option<(usize, usize)> = None;
    let mut nearest_water_d = f32::INFINITY;
    let mut best_graze: Option<((usize, usize), f32)> = None;
    let mut best_score = f32::NEG_INFINITY;
    let mut best_patrol: Option<(usize, usize)> = None;
    let mut best_patrol_score = f32::NEG_INFINITY;
    let use_shore = world.shore.len() == world.cells.len();

    // Visit only the rows of the ellipse; each row's cell span is contiguous.
    for y in y0..y1 {
        let dy = crate::cast!(y => f32) - crate::cast!(cy => f32);
        let half = 2.0 * (r_f * r_f - dy * dy).max(0.0).sqrt();
        let x0 = crate::cast!((crate::cast!((crate::cast!(cx => f32) - half).ceil() => i32)).max(0) => usize);
        let x1 = crate::cast!(((crate::cast!((crate::cast!(cx => f32) + half).floor() => i32)) + 1).min(crate::cast!(world.width => i32)).max(0) => usize);
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
            let dx = (crate::cast!(x => f32) - crate::cast!(cx => f32)) / 2.0;
            let d = (dx * dx + dy * dy).sqrt();
            if d > r_f {
                continue;
            }
            let shore = if use_shore { world.shore[row + x] } else { world.is_shore(x, y) };
            if shore && d < nearest_water_d {
                nearest_water_d = d;
                nearest_water = Some((x, y));
            }
            let food = cell.vegetation * edible[crate::cast!(cell.terrain => usize)];
            if food >= cp.graze_min_vegetation {
                let mut score = food / (1.0 + d / 4.0);
                // C8 FR2: when herding, near the group's centroid scores better,
                // so a herd grazes the same cells down (no new vegetation code).
                if herding {
                    let to_kin = geom::dist(x, y, kx, ky);
                    score /= 1.0 + sociality * sp.graze_cohesion_w * to_kin / 8.0;
                }
                if score > best_score {
                    best_score = score;
                    best_graze = Some(((x, y), score));
                }
            }
            // C5 FR13: foreign scent scales the patrol score down for a
            // solitary predator; `Scent::NONE` leaves it exactly as it was.
            let patrol = cell.prey_pressure * scent.patrol_factor(scent.foreign_at(row + x));
            if patrol > best_patrol_score {
                best_patrol_score = patrol;
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

    (Perception { nearest_water, best_graze, nearest_den, best_patrol, creatures }, kin)
}
