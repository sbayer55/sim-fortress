//! Grid spatial index: one bucket of living creature ids per cell (FR Scope).

use crate::sim::creatures::{CreatureId, CreatureStore};
use crate::sim::geom;
use crate::sim::world::World;

#[derive(Clone, Debug, Default)]
pub struct SpatialIndex {
    /// Per-cell buckets of `(id, x, y)`, sorted ascending by id after rebuild.
    buckets: Vec<Vec<(CreatureId, usize, usize)>>,
    width: usize,
    height: usize,
}

impl SpatialIndex {
    pub fn new(world: &World) -> Self {
        SpatialIndex { buckets: vec![Vec::new(); world.width * world.height], width: world.width, height: world.height }
    }

    /// Rebuild all buckets from the living creatures (positions as they now stand).
    pub fn rebuild(&mut self, store: &CreatureStore, world: &World) {
        self.width = world.width;
        self.height = world.height;
        self.buckets.clear();
        self.buckets.resize(world.width * world.height, Vec::new());
        for c in store.living() {
            let idx = c.y * world.width + c.x;
            if idx < self.buckets.len() {
                self.buckets[idx].push((c.id, c.x, c.y));
            }
        }
        for b in &mut self.buckets {
            b.sort_unstable();
        }
    }

    /// All living creature ids inside the half-open rectangle `[x0, x1) × [y0, y1)`, ascending.
    pub fn in_rect(&self, x0: usize, y0: usize, x1: usize, y1: usize) -> Vec<CreatureId> {
        let x0 = x0.min(self.width);
        let y0 = y0.min(self.height);
        let x1 = x1.min(self.width);
        let y1 = y1.min(self.height);
        let mut out = Vec::new();
        for y in y0..y1 {
            for x in x0..x1 {
                for &(id, _, _) in &self.buckets[y * self.width + x] {
                    out.push(id);
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// Living creature ids within ellipse radius `r` of `(cx, cy)`, ascending.
    /// Only the buckets inside the ellipse's bounding box are visited.
    pub fn within(&self, cx: usize, cy: usize, r: u16) -> Vec<CreatureId> {
        let r = r as i64;
        let cx = cx as i64;
        let cy = cy as i64;
        let x0 = (cx - 2 * r).max(0) as usize;
        let y0 = (cy - r).max(0) as usize;
        let x1 = ((cx + 2 * r + 1).min(self.width as i64)).max(0) as usize;
        let y1 = ((cy + r + 1).min(self.height as i64)).max(0) as usize;
        let r_f = r as f32;
        let mut out = Vec::new();
        for y in y0..y1 {
            for x in x0..x1 {
                for &(id, px, py) in &self.buckets[y * self.width + x] {
                    if geom::dist(cx as usize, cy as usize, px, py) <= r_f {
                        out.push(id);
                    }
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
