//! Flow routing primitives over the cell grid: steepest-descent receivers,
//! the downstream-first processing order, drainage accumulation and
//! depression filling. Everything here is pure and index-ordered, so the
//! result depends only on the input surface.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

const SQRT5: f32 = 2.236_068;

/// The eight neighbours as (dx, dy, distance). Cells are twice as tall as
/// they are wide, so a vertical step covers two horizontal units and a
/// diagonal covers sqrt(5).
const NEIGHBOURS: [(i32, i32, f32); 8] = [
    (-1, -1, SQRT5),
    (0, -1, 2.0),
    (1, -1, SQRT5),
    (-1, 0, 1.0),
    (1, 0, 1.0),
    (-1, 1, SQRT5),
    (0, 1, 2.0),
    (1, 1, SQRT5),
];

#[derive(Clone, Copy, Debug)]
pub(super) struct Grid {
    pub(super) w: usize,
    pub(super) h: usize,
}

impl Grid {
    pub(super) const fn len(self) -> usize {
        self.w * self.h
    }

    /// Is `i` on the outer ring of the grid?
    pub(super) const fn is_edge(self, i: usize) -> bool {
        let (x, y) = (i % self.w, i.div_euclid(self.w));
        x == 0 || y == 0 || x + 1 == self.w || y + 1 == self.h
    }

    /// Call `f(neighbour, distance)` for every in-bounds 8-neighbour of `i`,
    /// in the fixed `NEIGHBOURS` order.
    pub(super) fn for_neighbours(self, i: usize, mut f: impl FnMut(usize, f32)) {
        let x = crate::cast!(i % self.w => i32);
        let y = crate::cast!(i.div_euclid(self.w) => i32);
        for &(dx, dy, d) in &NEIGHBOURS {
            let (nx, ny) = (x + dx, y + dy);
            if nx >= 0 && ny >= 0 && crate::cast!(nx => usize) < self.w && crate::cast!(ny => usize) < self.h {
                f(crate::cast!(ny => usize) * self.w + crate::cast!(nx => usize), d);
            }
        }
    }
}

/// Where each cell drains to, and in what order to walk the drainage tree.
#[derive(Debug)]
pub(super) struct Flow {
    /// Steepest-descent receiver; a pit or base-level cell is its own receiver.
    pub(super) recv: Vec<usize>,
    /// Distance from each cell to its receiver (0 for roots).
    pub(super) dist: Vec<f32>,
    /// Every cell, each root before all the cells that drain into it.
    pub(super) order: Vec<usize>,
}

/// Route `surface` downhill. `fixed` cells (base level) never drain anywhere.
pub(super) fn route(grid: Grid, surface: &[f32], fixed: &[bool]) -> Flow {
    let n = grid.len();
    let mut recv = Vec::with_capacity(n);
    let mut dist = Vec::with_capacity(n);
    for i in 0..n {
        let (mut best, mut best_d, mut best_slope) = (i, 0.0f32, 0.0f32);
        if !fixed[i] {
            grid.for_neighbours(i, |j, d| {
                let s = (surface[i] - surface[j]) / d;
                if s > best_slope {
                    best_slope = s;
                    best = j;
                    best_d = d;
                }
            });
        }
        recv.push(best);
        dist.push(best_d);
    }
    let order = stack_order(&recv);
    Flow { recv, dist, order }
}

/// Downstream-first order (Braun & Willett): donors are gathered per receiver
/// and every drainage tree is walked from its root.
fn stack_order(recv: &[usize]) -> Vec<usize> {
    let n = recv.len();
    let mut start = vec![0usize; n + 1];
    for i in 0..n {
        if recv[i] != i {
            start[recv[i] + 1] += 1;
        }
    }
    for i in 0..n {
        start[i + 1] += start[i];
    }
    let mut fill = start.clone();
    let mut donors = vec![0usize; n];
    for i in 0..n {
        let r = recv[i];
        if r != i {
            donors[fill[r]] = i;
            fill[r] += 1;
        }
    }
    let mut order = Vec::with_capacity(n);
    let mut stack = Vec::new();
    for root in 0..n {
        if recv[root] != root {
            continue;
        }
        stack.push(root);
        while let Some(c) = stack.pop() {
            order.push(c);
            for k in start[c]..start[c + 1] {
                stack.push(donors[k]);
            }
        }
    }
    order
}

/// Drainage area: each cell's own `rain` plus everything upstream of it.
pub(super) fn accumulate(flow: &Flow, rain: &[f32]) -> Vec<f32> {
    let mut acc = rain.to_vec();
    for &i in flow.order.iter().rev() {
        let r = flow.recv[i];
        if r != i {
            let a = acc[i];
            acc[r] += a;
        }
    }
    acc
}

/// Min-heap key: lowest surface first, then lowest index, so the flood is
/// fully determined by the input.
#[derive(Debug)]
struct Key {
    z: f32,
    i: usize,
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Key {}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        other.z.total_cmp(&self.z).then_with(|| other.i.cmp(&self.i))
    }
}

/// Priority-flood depression filling from the `outlet` cells. Every cell
/// ends at least `eps` above the cell it was reached from, so the filled
/// surface drains everywhere and lakes have a spill direction.
pub(super) fn fill_depressions(grid: Grid, height: &[f32], outlet: &[bool], eps: f32) -> Vec<f32> {
    let n = grid.len();
    let mut filled = height.to_vec();
    let mut seen = vec![false; n];
    let mut heap = BinaryHeap::new();
    for i in 0..n {
        if outlet[i] {
            seen[i] = true;
            heap.push(Key { z: height[i], i });
        }
    }
    while let Some(Key { z, i }) = heap.pop() {
        grid.for_neighbours(i, |j, _| {
            if !seen[j] {
                seen[j] = true;
                filled[j] = height[j].max(z + eps);
                heap.push(Key { z: filled[j], i: j });
            }
        });
    }
    filled
}

/// Steepest downhill gradient at every cell (0 in a pit).
pub(super) fn slopes(grid: Grid, height: &[f32]) -> Vec<f32> {
    let n = grid.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut best = 0.0f32;
        grid.for_neighbours(i, |j, d| best = best.max((height[i] - height[j]) / d));
        out.push(best);
    }
    out
}

/// The `count` cells from `candidates` with the lowest `key` (ties broken by
/// index). Only the set is meaningful: the returned order is unspecified.
pub(super) fn lowest(candidates: impl Iterator<Item = usize>, key: &[f32], count: usize) -> Vec<usize> {
    select(candidates, key, count, false)
}

/// The `count` cells from `candidates` with the highest `key`.
pub(super) fn highest(candidates: impl Iterator<Item = usize>, key: &[f32], count: usize) -> Vec<usize> {
    select(candidates, key, count, true)
}

fn select(candidates: impl Iterator<Item = usize>, key: &[f32], count: usize, desc: bool) -> Vec<usize> {
    let mut v: Vec<usize> = candidates.collect();
    let count = count.min(v.len());
    if count == 0 {
        return Vec::new();
    }
    // A total order (ties by index) makes the selected set a pure function
    // of the input, whatever partition order the algorithm leaves behind.
    v.select_nth_unstable_by(count - 1, |&a, &b| {
        let o = key[a].total_cmp(&key[b]);
        (if desc { o.reverse() } else { o }).then(a.cmp(&b))
    });
    v.truncate(count);
    v
}
