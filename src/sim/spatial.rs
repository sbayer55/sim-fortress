//! Grid spatial index: one bucket of living creature ids per cell (FR Scope).

use crate::sim::creatures::{CreatureId, CreatureStore};
use crate::sim::geom;
use crate::sim::world::World;

/// Compressed-row layout: `entries` holds `(id, x, y)` grouped by cell in
/// row-major cell order (ids ascending within a cell); `cell_start[i]..cell_start[i+1]`
/// is cell `i`'s slice. A row's `[x0, x1)` span is therefore one contiguous slice.
#[derive(Clone, Debug, Default)]
pub struct SpatialIndex {
    entries: Vec<(CreatureId, usize, usize)>,
    cell_start: Vec<u32>,
    width: usize,
    height: usize,
}

impl SpatialIndex {
    pub fn new(world: &World) -> Self {
        SpatialIndex {
            entries: Vec::new(),
            cell_start: vec![0; world.width * world.height + 1],
            width: world.width,
            height: world.height,
        }
    }

    /// Rebuild from the living creatures (positions as they now stand): a
    /// counting sort by cell, which keeps ids ascending within each cell because
    /// `living()` is visited in slot order and the result is sorted per cell.
    pub fn rebuild(&mut self, store: &CreatureStore, world: &World) {
        self.width = world.width;
        self.height = world.height;
        let cells = world.width * world.height;
        self.cell_start.clear();
        self.cell_start.resize(cells + 1, 0);
        let mut n = 0usize;
        for c in store.living() {
            let idx = c.y * world.width + c.x;
            if idx < cells {
                self.cell_start[idx + 1] += 1;
                n += 1;
            }
        }
        for i in 0..cells {
            self.cell_start[i + 1] += self.cell_start[i];
        }
        let mut cursor: Vec<u32> = self.cell_start[..cells].to_vec();
        self.entries.clear();
        self.entries.resize(n, (CreatureId(0), 0, 0));
        for c in store.living() {
            let idx = c.y * world.width + c.x;
            if idx < cells {
                let at = cursor[idx] as usize;
                self.entries[at] = (c.id, c.x, c.y);
                cursor[idx] += 1;
            }
        }
        for i in 0..cells {
            let (a, b) = (self.cell_start[i] as usize, self.cell_start[i + 1] as usize);
            if b - a > 1 {
                self.entries[a..b].sort_unstable();
            }
        }
    }

    /// The entries of row `y` for cells `[x0, x1)`.
    fn row_span(&self, y: usize, x0: usize, x1: usize) -> &[(CreatureId, usize, usize)] {
        let a = self.cell_start[y * self.width + x0] as usize;
        let b = self.cell_start[y * self.width + x1] as usize;
        &self.entries[a..b]
    }

    /// All living creature ids inside the half-open rectangle `[x0, x1) × [y0, y1)`, ascending.
    pub fn in_rect(&self, x0: usize, y0: usize, x1: usize, y1: usize) -> Vec<CreatureId> {
        let x0 = x0.min(self.width);
        let y0 = y0.min(self.height);
        let x1 = x1.min(self.width);
        let y1 = y1.min(self.height);
        let mut out = Vec::new();
        if x0 >= x1 {
            return out;
        }
        for y in y0..y1 {
            for &(id, _, _) in self.row_span(y, x0, x1) {
                out.push(id);
            }
        }
        out.sort_unstable();
        out
    }

    /// Living creature ids within ellipse radius `r` of `(cx, cy)`, ascending.
    /// Only the rows inside the ellipse are visited, each as one slice.
    pub fn within(&self, cx: usize, cy: usize, r: u16) -> Vec<CreatureId> {
        let r = r as i64;
        let cx = cx as i64;
        let cy = cy as i64;
        let y0 = (cy - r).max(0);
        let y1 = (cy + r + 1).min(self.height as i64).max(0);
        let r2 = (r * r) as f32;
        let mut out = Vec::new();
        for y in y0..y1 {
            let dy = (y - cy) as f32;
            // (dx/2)² + dy² ≤ r²  →  |dx| ≤ 2·sqrt(r² − dy²)
            let half = 2.0 * (r2 - dy * dy).max(0.0).sqrt();
            let x0 = ((cx as f32 - half).ceil() as i64).max(0) as usize;
            let x1 = ((cx as f32 + half).floor() as i64 + 1).min(self.width as i64).max(0) as usize;
            if x0 >= x1 {
                continue;
            }
            for &(id, px, py) in self.row_span(y as usize, x0, x1) {
                if geom::dist(cx as usize, cy as usize, px, py) <= r as f32 {
                    out.push(id);
                }
            }
        }
        out.sort_unstable();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::creatures::{place_founders, CreatureStore};
    use crate::sim::params::{CreaturesParams, WorldParams};
    use crate::sim::rng::Rng;

    fn world() -> World {
        World::generate(7, &WorldParams::default())
    }

    fn store_and_index() -> (CreatureStore, SpatialIndex) {
        let w = world();
        let mut store = CreatureStore::new();
        for c in place_founders(&w, &CreaturesParams::default(), &mut Rng::new(1)) {
            store.insert(c);
        }
        let mut idx = SpatialIndex::new(&w);
        idx.rebuild(&store, &w);
        (store, idx)
    }

    #[test]
    fn index_matches_bruteforce() {
        let (store, idx) = store_and_index();
        for (cx, cy, r) in [(0usize, 0usize, 5u16), (70, 20, 8), (140, 38, 12), (40, 10, 2)] {
            let got = idx.within(cx, cy, r);
            let mut want: Vec<CreatureId> = store
                .living()
                .filter(|c| crate::sim::geom::dist(cx, cy, c.x, c.y) <= r as f32)
                .map(|c| c.id)
                .collect();
            want.sort_unstable();
            assert_eq!(got, want, "within({cx},{cy},{r})");
        }
    }

    #[test]
    fn in_rect_sorted_ascending() {
        let (_store, idx) = store_and_index();
        let ids = idx.in_rect(0, 0, idx.width, idx.height);
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
        assert!(!ids.is_empty());
    }
}
