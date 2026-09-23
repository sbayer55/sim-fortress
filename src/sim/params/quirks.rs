//! Quirks: named, `WorldBox` / CK3-style oddities a creature is born with
//! (`[quirks]`, `[[quirks.catalog]]`). Off by default; for fun, not realism.
//!
//! Every quirk is a row of multipliers on facts the sim already has (speed,
//! senses, kill and evade odds, litter, lifespan …), plus an optional code-backed
//! `special`. The catalogue merges by `name` like `[[species]]`, so an overlay can
//! retune or add a quirk without a rebuild. "Trait" already means a genome slot,
//! hence the different word.

use serde::{Deserialize, Serialize};

mod catalog;

/// How often a quirk is rolled; legendary quirks are never inherited by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuirkTier {
    Common,
    Rare,
    Legendary,
}

/// Which species kinds may carry a quirk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuirkKinds {
    Any,
    Prey,
    Predator,
}

/// A quirk effect that is more than a multiplier and needs its own code path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuirkSpecial {
    None,
    /// A starving predator may hunt juveniles of its own species.
    Cannibal,
}

/// One quirk of the catalogue. Multipliers default to 1.0 (no effect).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QuirkDef {
    pub name: String,
    pub tier: QuirkTier,
    pub kinds: QuirkKinds,
    /// Whether a parent can pass it on (legendary quirks default to false).
    pub inheritable: bool,
    /// Quirks a carrier can never also have (both directions are enforced).
    pub excludes: Vec<String>,
    pub special: QuirkSpecial,
    /// Move budget per tick.
    pub speed: f32,
    /// Effective Sense trait (perception, detection), clamped to the genome range.
    pub sense: f32,
    /// Kill chance when this creature attacks.
    pub kill_odds: f32,
    /// Chance to survive when this creature is attacked (kill chance ÷ this).
    pub evade_odds: f32,
    /// Effective Camouflage trait, clamped to the genome range.
    pub camouflage: f32,
    /// Length of a flight from a predator.
    pub flee_ticks: f32,
    /// Litter size.
    pub litter: f32,
    /// Maximum age.
    pub lifespan: f32,
    /// Days to adulthood.
    pub maturity: f32,
    /// Infection chance.
    pub susceptibility: f32,
    /// Hunger rate.
    pub hunger: f32,
    /// Effective Sociality trait (herding, packs), clamped to the genome range.
    pub sociality: f32,
}

impl Default for QuirkDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            tier: QuirkTier::Common,
            kinds: QuirkKinds::Any,
            inheritable: true,
            excludes: Vec::new(),
            special: QuirkSpecial::None,
            speed: 1.0,
            sense: 1.0,
            kill_odds: 1.0,
            evade_odds: 1.0,
            camouflage: 1.0,
            flee_ticks: 1.0,
            litter: 1.0,
            lifespan: 1.0,
            maturity: 1.0,
            susceptibility: 1.0,
            hunger: 1.0,
            sociality: 1.0,
        }
    }
}

impl QuirkDef {
    /// The multipliers in a fixed order, for validation and display.
    pub const fn multipliers(&self) -> [(&'static str, f32); 12] {
        [
            ("speed", self.speed),
            ("sense", self.sense),
            ("kill", self.kill_odds),
            ("evade", self.evade_odds),
            ("camo", self.camouflage),
            ("flee", self.flee_ticks),
            ("litter", self.litter),
            ("lifespan", self.lifespan),
            ("maturity", self.maturity),
            ("disease", self.susceptibility),
            ("hunger", self.hunger),
            ("social", self.sociality),
        ]
    }

    /// `speed +30%, hunger +10%`: the multipliers that are not 1, as percentages.
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = self
            .multipliers()
            .into_iter()
            .filter(|(_, v)| (v - 1.0).abs() > 1e-4)
            .map(|(k, v)| format!("{k} {:+.0}%", (v - 1.0) * 100.0))
            .collect();
        if self.special == QuirkSpecial::Cannibal {
            parts.insert(0, "eats own young".into());
        }
        parts.join(", ")
    }
}

/// Quirk tunables (`[quirks]`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QuirkParams {
    /// Master switch, chosen on the New World screen. Off: no quirk is ever rolled.
    pub enabled: bool,
    /// Chance a founder or newborn rolls one fresh quirk.
    pub birth_chance: f32,
    /// After a fresh quirk, the chance of rolling another (repeats up to the max).
    pub extra_chance: f32,
    /// Most quirks one creature can carry (at most 64).
    pub max_per_creature: u32,
    /// Chance each inheritable quirk of each parent passes to a pup.
    pub inherit_chance: f32,
    /// Roll weight of a common quirk.
    pub common_weight: f32,
    /// Roll weight of a rare quirk.
    pub rare_weight: f32,
    /// Roll weight of a legendary quirk.
    pub legendary_weight: f32,
    /// A Cannibal hunts its own species' juveniles at or above this hunger.
    pub cannibal_hunger: f32,
    /// At most 64 entries (a creature stores its quirks as a 64-bit mask).
    pub catalog: Vec<QuirkDef>,
}

impl Default for QuirkParams {
    fn default() -> Self {
        Self {
            enabled: false,
            birth_chance: 0.15,
            extra_chance: 0.25,
            max_per_creature: 3,
            inherit_chance: 0.5,
            common_weight: 10.0,
            rare_weight: 3.0,
            legendary_weight: 0.2,
            cannibal_hunger: 0.7,
            catalog: catalog::default_catalog(),
        }
    }
}

impl QuirkParams {
    /// Roll weight of one catalogue entry.
    pub const fn weight(&self, def: &QuirkDef) -> f32 {
        match def.tier {
            QuirkTier::Common => self.common_weight,
            QuirkTier::Rare => self.rare_weight,
            QuirkTier::Legendary => self.legendary_weight,
        }
    }

    /// Catalogue index of a quirk by name.
    pub fn position(&self, name: &str) -> Option<usize> {
        self.catalog.iter().position(|d| d.name == name)
    }

    /// Reject catalogues the sim cannot run: the error names the rule.
    pub fn validate(&self) -> Result<(), String> {
        if self.catalog.len() > 64 {
            return Err(format!("quirks.catalog has {} entries; at most 64", self.catalog.len()));
        }
        if self.max_per_creature > 64 {
            return Err("quirks.max_per_creature must be at most 64".into());
        }
        for (i, d) in self.catalog.iter().enumerate() {
            if d.name.is_empty() {
                return Err(format!("quirk #{i} has no name"));
            }
            if self.catalog.iter().take(i).any(|o| o.name == d.name) {
                return Err(format!("quirk '{}' is defined twice", d.name));
            }
            if let Some(x) = d.excludes.iter().find(|x| self.position(x).is_none()) {
                return Err(format!("quirk '{}' excludes unknown quirk '{x}'", d.name));
            }
            if let Some((k, v)) = d.multipliers().into_iter().find(|(_, v)| !(*v > 0.0 && *v <= 10.0)) {
                return Err(format!("quirk '{}': {k} = {v} is outside (0, 10]", d.name));
            }
        }
        for (k, v) in [("birth_chance", self.birth_chance), ("extra_chance", self.extra_chance), ("inherit_chance", self.inherit_chance)] {
            if !(0.0..=1.0).contains(&v) {
                return Err(format!("quirks.{k} = {v} is outside 0..=1"));
            }
        }
        Ok(())
    }
}
