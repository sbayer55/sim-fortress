//! Social tunables.

use serde::{Deserialize, Serialize};

/// Herd and pack behaviour tunables (C8 FR1, the `[social]` table).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SocialParams {
    /// Preferred group size = `sociality × this`, for herds and packs alike.
    pub group_size_max: f32,
    /// Below this sociality a creature never herds: it wanders as before.
    pub cohesion_min: f32,
    /// Graze score ÷ `(1 + sociality × w × dist_to_kin_centroid / 8)` when herding.
    pub graze_cohesion_w: f32,
    /// A fleeing prey alerts same-species kin within `sociality × this` cells.
    pub alarm_cells: f32,
    /// Hunt score × `(1 + sociality × bonus × packmates_on_target(≤ 3))`.
    pub pack_join_bonus: f32,
    /// Kill chance `+= bonus × extra participants (≤ 3)`.
    pub pack_kill_bonus: f32,
    /// Hunger relief a non-killer participant gets, as a share of a full kill.
    pub pack_share: f32,
    /// A participant is a same-species hunter on that prey within this Chebyshev distance.
    pub pack_share_cheb: usize,
}

impl Default for SocialParams {
    fn default() -> Self {
        Self {
            group_size_max: 12.0,
            cohesion_min: 0.30,
            graze_cohesion_w: 1.0,
            alarm_cells: 6.0,
            pack_join_bonus: 1.5,
            pack_kill_bonus: 0.08,
            pack_share: 0.5,
            pack_share_cheb: 6,
        }
    }
}

impl SocialParams {
    /// Preferred group size for a sociality value.
    pub fn preferred_group(&self, sociality: f32) -> f32 {
        sociality * self.group_size_max
    }

    /// Whether a creature with this sociality and this many visible kin is
    /// herding (C8 FR2): social enough, not alone, and not over the dispersal
    /// threshold. One rule, shared by cohesion, the graze bias and the inspector.
    pub fn herding(&self, sociality: f32, kin_count: u8) -> bool {
        sociality >= self.cohesion_min && kin_count > 0 && f32::from(kin_count) <= 1.5 * self.preferred_group(sociality)
    }
}
