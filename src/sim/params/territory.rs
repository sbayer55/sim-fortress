//! Territory tunables: the scent grid, scent avoidance and the contest.

use serde::{Deserialize, Serialize};

use crate::sim::species::Genome;

/// The `[territory]` table (C5 FR13).
///
/// Adult predators mark the cell under them every tick; the mark decays
/// daily and carries the id of the creature holding it. A solitary predator
/// steers its patrol and its hunts away from ground held by a rival of its
/// own species, and a resident challenges a same-species intruder standing on
/// its ground; one contest roll evicts the loser.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TerritoryParams {
    /// Scent an adult predator adds to the cell under it each tick (capped at 1).
    pub mark_per_tick: f32,
    /// Extra scent a kill adds at the kill cell.
    pub kill_mark: f32,
    /// Daily multiplier on every mark.
    pub decay_per_day: f32,
    /// Below this strength a depositor takes the holder slot; at or above it
    /// the holder is resident there.
    pub hold_min: f32,
    /// Foreign scent below this strength is ignored.
    pub notice_min: f32,
    /// Weight of `aggression × (1 − sociality) × (1 − hunger)` in the avoidance term.
    pub avoid_w: f32,
    /// Prior-residence advantage added to the resident's contest score.
    pub resident_bonus: f32,
    /// Steepness of the contest win logistic.
    pub contest_k: f32,
    /// Energy both sides pay in a contest.
    pub contest_energy: f32,
    /// Hp the loser pays per unit of the winner's size.
    pub contest_injury: f32,
    /// Hp is clamped here after a contest, so contests never kill.
    pub hp_floor: f32,
    /// Ticks the loser flees the winner.
    pub evict_ticks: u32,
    /// A challenge is abandoned after this many ticks.
    pub challenge_ticks: u32,
    /// Days both sides wait before another challenge.
    pub contest_cooldown_days: u32,
}

impl Default for TerritoryParams {
    fn default() -> Self {
        Self {
            mark_per_tick: 0.05,
            kill_mark: 0.50,
            decay_per_day: 0.90,
            hold_min: 0.15,
            notice_min: 0.10,
            avoid_w: 1.0,
            resident_bonus: 0.25,
            contest_k: 4.0,
            contest_energy: 0.15,
            contest_injury: 0.10,
            hp_floor: 0.05,
            evict_ticks: 24,
            challenge_ticks: 12,
            contest_cooldown_days: 3,
        }
    }
}

impl TerritoryParams {
    /// How strongly this animal steers away from foreign scent right now:
    /// `avoid_w × aggression × (1 − sociality) × (1 − hunger)`, never negative.
    /// A social animal shares ground; a starving one trespasses.
    pub fn avoid(&self, genome: &Genome, hunger: f32) -> f32 {
        (self.avoid_w * genome.aggression() * (1.0 - genome.sociality()) * (1.0 - hunger)).max(0.0)
    }

    /// The resident's probability of winning a contest against `intruder`:
    /// a logistic of the score gap, where a score is `aggression × size` and
    /// the resident adds `resident_bonus`.
    pub fn resident_wins(&self, resident: &Genome, intruder: &Genome) -> f32 {
        let gap = resident.aggression() * resident.size() + self.resident_bonus - intruder.aggression() * intruder.size();
        1.0 / (1.0 + (-self.contest_k * gap).exp())
    }

    /// True when the overlay switches the mechanic off: with no scent laid
    /// there is no foreign ground, no resident, no challenge and no contest.
    pub fn is_neutral(&self) -> bool {
        self.mark_per_tick <= 0.0 && self.kill_mark <= 0.0
    }

    /// The overlay that reproduces pre-territory behaviour. The control for
    /// tests and sweeps.
    pub const fn neutral(&mut self) {
        self.mark_per_tick = 0.0;
        self.kill_mark = 0.0;
    }

    /// Reject values the mechanic cannot run with.
    pub fn validate(&self) -> Result<(), String> {
        let rates = [
            ("territory.mark_per_tick", self.mark_per_tick),
            ("territory.kill_mark", self.kill_mark),
            ("territory.avoid_w", self.avoid_w),
            ("territory.resident_bonus", self.resident_bonus),
            ("territory.contest_k", self.contest_k),
            ("territory.contest_energy", self.contest_energy),
            ("territory.contest_injury", self.contest_injury),
        ];
        for (name, v) in rates {
            if v < 0.0 || v.is_nan() {
                return Err(format!("{name} = {v} must be at least 0"));
            }
        }
        for (name, v) in [("territory.decay_per_day", self.decay_per_day), ("territory.hold_min", self.hold_min), ("territory.notice_min", self.notice_min), ("territory.hp_floor", self.hp_floor)] {
            if !(0.0..=1.0).contains(&v) {
                return Err(format!("{name} = {v} must be within 0..=1"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::sim::species::testing::*;

    #[test]
    fn avoid_is_zero_for_the_social_and_the_starving() {
        let t = TerritoryParams::default();
        let mut g = genome(FOX);
        g.0[4] = 0.8; // aggression
        g.0[9] = 0.2; // sociality
        assert!((t.avoid(&g, 0.0) - 0.64).abs() < 1e-6, "{}", t.avoid(&g, 0.0));
        assert!((t.avoid(&g, 0.5) - 0.32).abs() < 1e-6);
        assert_eq!(t.avoid(&g, 1.0), 0.0, "a starving animal trespasses");
        assert_eq!(t.avoid(&g, 1.5), 0.0, "hunger past 1 never goes negative");
        g.0[9] = 1.0;
        assert_eq!(t.avoid(&g, 0.0), 0.0, "a fully social animal shares ground");
    }

    #[test]
    fn resident_wins_more_often_with_the_bonus() {
        let t = TerritoryParams::default();
        let g = genome(FOX);
        let even = t.resident_wins(&g, &g);
        assert!(even > 0.5, "the same genome on both sides: the resident's bonus decides ({even})");
        let mut strong = g;
        strong.0[4] = 0.98;
        strong.0[1] = 0.98;
        let mut weak = g;
        weak.0[4] = 0.02;
        weak.0[1] = 0.02;
        assert!(t.resident_wins(&strong, &weak) > 0.95);
        assert!(t.resident_wins(&weak, &strong) < 0.3);
        let flat = TerritoryParams { resident_bonus: 0.0, ..TerritoryParams::default() };
        assert_eq!(flat.resident_wins(&g, &g), 0.5, "no bonus: a coin toss");
    }

    #[test]
    fn neutral_overlay_is_detected() {
        let mut t = TerritoryParams::default();
        assert!(!t.is_neutral());
        t.neutral();
        assert!(t.is_neutral());
        assert!(t.validate().is_ok());
        let bad = TerritoryParams { hp_floor: 1.5, ..TerritoryParams::default() };
        assert!(bad.validate().unwrap_err().contains("hp_floor"));
        let bad = TerritoryParams { contest_energy: -0.1, ..TerritoryParams::default() };
        assert!(bad.validate().unwrap_err().contains("contest_energy"));
    }
}
