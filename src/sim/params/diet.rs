//! Diet breadth tunables: which terrains a herbivore can graze.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::sim::world::Terrain;

/// The `[diet]` table (C4 FR, genome slot 12).
///
/// Every vegetated terrain sits at a fixed position on a grass→browse axis; a
/// creature with Diet breadth `b` eats everything at or below `b` fully and
/// loses edibility over `edge` above it. Specialists bite faster, generalists
/// reach further.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DietParams {
    /// Grass→browse position of each vegetated terrain, 0..=1. Terrain absent
    /// here is never edible.
    pub terrain_position: BTreeMap<Terrain, f32>,
    /// Width of the soft edge above a creature's reach: edibility falls 1→0 over it.
    pub edge: f32,
    /// Bite multiplier at breadth 0: `bite = 1 + (1 − breadth) × this`.
    pub specialist_bonus: f32,
}

impl Default for DietParams {
    fn default() -> Self {
        use Terrain::{Dirt, Forest, Grass, GrassDense, GrassSparse, Marsh, Sand};
        Self {
            terrain_position: BTreeMap::from([
                (GrassDense, 0.00),
                (Grass, 0.10),
                (GrassSparse, 0.25),
                (Marsh, 0.40),
                (Dirt, 0.55),
                (Sand, 0.65),
                (Forest, 0.90),
            ]),
            edge: 0.15,
            specialist_bonus: 0.5,
        }
    }
}

impl DietParams {
    /// How much of this terrain's vegetation a creature of this breadth can
    /// digest: 1 at or below its reach, 0 once `edge` past it.
    pub fn edibility(&self, terrain: Terrain, breadth: f32) -> f32 {
        let Some(&p) = self.terrain_position.get(&terrain) else { return 0.0 };
        if self.edge <= 0.0 {
            return if p <= breadth { 1.0 } else { 0.0 };
        }
        (1.0 - (p - breadth) / self.edge).clamp(0.0, 1.0)
    }

    /// Bite-rate multiplier: a pure specialist bites `1 + specialist_bonus`
    /// times as fast, a full generalist 1×.
    pub fn bite(&self, breadth: f32) -> f32 {
        1.0 + (1.0 - breadth).clamp(0.0, 1.0) * self.specialist_bonus
    }

    /// Edibility indexed by `terrain as usize`, built once per replan so the
    /// perception loop never touches the map.
    pub fn table(&self, breadth: f32) -> [f32; Terrain::COUNT] {
        let mut out = [0.0; Terrain::COUNT];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = self.edibility(Terrain::from_code(crate::cast!(i => u8)), breadth);
        }
        out
    }

    /// The furthest terrain along the axis this breadth eats fully, for the
    /// inspector; `None` when nothing is fully edible.
    pub fn reach_name(&self, breadth: f32) -> Option<&'static str> {
        self.terrain_position
            .iter()
            .filter(|&(_, &p)| p <= breadth)
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(t, _)| t.name())
    }

    /// The overlay that reproduces pre-diet grazing: every terrain fully
    /// edible at every breadth and a 1× bite. The control for tests and sweeps.
    pub fn neutral(&mut self) {
        for p in self.terrain_position.values_mut() {
            *p = 0.0;
        }
        self.specialist_bonus = 0.0;
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn edibility_is_one_below_reach_and_zero_past_the_edge() {
        let d = DietParams::default();
        assert_eq!(d.edibility(Terrain::GrassDense, 0.0), 1.0);
        assert_eq!(d.edibility(Terrain::Grass, 0.10), 1.0);
        assert_eq!(d.edibility(Terrain::Forest, 0.35), 0.0, "forest is far past a vole's edge");
        let half = d.edibility(Terrain::Marsh, 0.325);
        assert!((half - 0.5).abs() < 1e-5, "marsh at 0.40 is half edible 0.075 below it: {half}");
        assert_eq!(d.edibility(Terrain::Rock, 0.98), 0.0, "rock has no position");
        assert_eq!(d.edibility(Terrain::DeepWater, 0.98), 0.0);
    }

    #[test]
    fn bite_is_neutral_for_a_generalist() {
        let d = DietParams::default();
        assert_eq!(d.bite(1.0), 1.0);
        assert_eq!(d.bite(0.0), 1.5);
        assert_eq!(d.bite(0.5), 1.25);
    }

    #[test]
    fn neutral_overlay_makes_every_terrain_edible() {
        let mut d = DietParams::default();
        d.neutral();
        for &t in DietParams::default().terrain_position.keys() {
            assert_eq!(d.edibility(t, 0.02), 1.0, "{t:?}");
        }
        assert_eq!(d.bite(0.02), 1.0);
    }

    #[test]
    fn table_matches_edibility() {
        let d = DietParams::default();
        let t = d.table(0.5);
        for code in 0..Terrain::COUNT {
            let terrain = Terrain::from_code(crate::cast!(code => u8));
            assert_eq!(t[code], d.edibility(terrain, 0.5), "{terrain:?}");
        }
    }

    #[test]
    fn reach_name_walks_the_axis() {
        let d = DietParams::default();
        assert_eq!(d.reach_name(0.35), Some("sparse grass"));
        assert_eq!(d.reach_name(0.50), Some("marsh"));
        assert_eq!(d.reach_name(0.95), Some("forest"));
        assert_eq!(d.reach_name(0.0), Some("meadow"));
    }
}
