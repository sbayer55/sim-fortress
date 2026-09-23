//! Quirks: named birth oddities (Giant, Albino, Undying …) in the spirit of
//! `WorldBox` and Crusader Kings 3. Off by default (`quirks.enabled`).
//!
//! A creature's quirks are a 64-bit mask of catalogue indices plus the folded
//! multipliers (`QuirkMods`) the behaviour code reads, so hot loops never touch
//! the catalogue. Rolls use a throwaway `Rng` seeded from the world seed and the
//! creature id, so no stream is threaded or saved and no existing draw moves:
//! with quirks off nothing is rolled, every mask is empty, every multiplier is 1
//! and the run is identical to one without the feature.

use serde::{Deserialize, Serialize};

use crate::sim::creatures::{CreatureId, CreatureStore};
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::lineage::Lineage;
use crate::sim::params::{QuirkKinds, QuirkParams, QuirkSpecial, QuirkTier, Roster};
use crate::sim::species::Kind;
use crate::sim::{Rng, Time};

/// Salt that separates quirk rolls from every other stream.
const QUIRK_SALT: u64 = 0x7175_6972_6B73_5F31;

/// The quirks a creature carries: bit `i` is catalogue entry `i`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuirkSet(pub u64);

impl QuirkSet {
    pub const fn has(self, i: usize) -> bool {
        i < 64 && self.0 & (1 << i) != 0
    }

    pub const fn insert(&mut self, i: usize) {
        if i < 64 {
            self.0 |= 1 << i;
        }
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }

    /// Catalogue indices, ascending.
    pub fn iter(self) -> impl Iterator<Item = usize> {
        (0..64).filter(move |i| self.has(*i))
    }
}

/// A creature's quirk multipliers folded into one record; identity when it has none.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuirkMods {
    pub speed: f32,
    pub sense: f32,
    pub kill: f32,
    pub evade: f32,
    pub camouflage: f32,
    pub flee: f32,
    pub litter: f32,
    pub lifespan: f32,
    pub maturity: f32,
    pub susceptibility: f32,
    pub hunger: f32,
    pub sociality: f32,
    /// Hunger at which a Cannibal hunts its own species' juveniles; `f32::MAX` = never.
    pub cannibal_hunger: f32,
}

impl QuirkMods {
    pub const IDENTITY: Self = Self {
        speed: 1.0,
        sense: 1.0,
        kill: 1.0,
        evade: 1.0,
        camouflage: 1.0,
        flee: 1.0,
        litter: 1.0,
        lifespan: 1.0,
        maturity: 1.0,
        susceptibility: 1.0,
        hunger: 1.0,
        sociality: 1.0,
        cannibal_hunger: f32::MAX,
    };
}

impl Default for QuirkMods {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Fold the multipliers of every quirk in `set`.
pub fn fold(set: QuirkSet, qp: &QuirkParams) -> QuirkMods {
    let mut m = QuirkMods::IDENTITY;
    for d in set.iter().filter_map(|i| qp.catalog.get(i)) {
        m.speed *= d.speed;
        m.sense *= d.sense;
        m.kill *= d.kill_odds;
        m.evade *= d.evade_odds;
        m.camouflage *= d.camouflage;
        m.flee *= d.flee_ticks;
        m.litter *= d.litter;
        m.lifespan *= d.lifespan;
        m.maturity *= d.maturity;
        m.susceptibility *= d.susceptibility;
        m.hunger *= d.hunger;
        m.sociality *= d.sociality;
        if d.special == QuirkSpecial::Cannibal {
            m.cannibal_hunger = qp.cannibal_hunger;
        }
    }
    m
}

/// A trait value scaled by a quirk multiplier, kept inside the genome range.
/// Exactly `v` when `k` is 1, so an unquirked creature reads its genome unchanged.
pub fn scaled(v: f32, k: f32) -> f32 {
    if (k - 1.0).abs() < f32::EPSILON { v } else { (v * k).clamp(0.02, 0.98) }
}

/// A tick count scaled by a quirk multiplier; exactly `base` when `k` is 1.
pub fn scale_ticks(base: u32, k: f32) -> u64 {
    if (k - 1.0).abs() < f32::EPSILON { u64::from(base) } else { crate::cast!((crate::cast!(base => f32) * k).round().max(1.0) => u64) }
}

/// Whether catalogue entry `i` may join `set` on a creature of `kind`.
fn allowed(qp: &QuirkParams, set: QuirkSet, i: usize, kind: Kind) -> bool {
    let Some(d) = qp.catalog.get(i) else { return false };
    let kind_ok = match d.kinds {
        QuirkKinds::Any => true,
        QuirkKinds::Prey => kind == Kind::Prey,
        QuirkKinds::Predator => kind == Kind::Predator,
    };
    if !kind_ok || set.has(i) {
        return false;
    }
    set.iter().filter_map(|j| qp.catalog.get(j)).all(|o| !o.excludes.contains(&d.name) && !d.excludes.contains(&o.name))
}

/// Roll a creature's quirks: inheritance from each parent first, then fresh rolls.
/// Deterministic in `(seed, id, parents)`.
pub fn roll(qp: &QuirkParams, seed: u64, id: CreatureId, kind: Kind, parents: &[QuirkSet]) -> QuirkSet {
    let mut rng = Rng::new(seed ^ QUIRK_SALT ^ u64::from(id.0).wrapping_mul(0xA24B_AED4_963E_E407));
    let max = qp.max_per_creature;
    let mut set = QuirkSet::default();
    for p in parents {
        for i in p.iter() {
            let inheritable = qp.catalog.get(i).is_some_and(|d| d.inheritable);
            if inheritable && set.len() < max && rng.chance(qp.inherit_chance) && allowed(qp, set, i, kind) {
                set.insert(i);
            }
        }
    }
    if set.len() < max && rng.chance(qp.birth_chance) {
        while let Some(i) = pick(qp, set, kind, &mut rng) {
            set.insert(i);
            if set.len() >= max || !rng.chance(qp.extra_chance) {
                break;
            }
        }
    }
    set
}

/// A weighted draw among the quirks that may still join `set`.
fn pick(qp: &QuirkParams, set: QuirkSet, kind: Kind, rng: &mut Rng) -> Option<usize> {
    let candidates: Vec<(usize, f32)> =
        (0..qp.catalog.len()).filter(|i| allowed(qp, set, *i, kind)).filter_map(|i| qp.catalog.get(i).map(|d| (i, qp.weight(d)))).filter(|(_, w)| *w > 0.0).collect();
    let total: f32 = candidates.iter().map(|(_, w)| w).sum();
    if total <= 0.0 {
        return None;
    }
    let mut r = rng.f32() * total;
    for (i, w) in &candidates {
        if r < *w {
            return Some(*i);
        }
        r -= w;
    }
    candidates.last().map(|(i, _)| *i)
}

/// Roll quirks for every living creature with id ≥ `first`.
///
/// Those are the founders, or the pups delivered this tick. The masks are
/// recorded in the lineage and rare or legendary births are announced. Does
/// nothing when quirks are off.
#[allow(clippy::too_many_arguments)]
pub fn assign_from(
    store: &mut CreatureStore,
    lineage: &mut Lineage,
    events: &mut EventRing,
    roster: &Roster,
    qp: &QuirkParams,
    seed: u64,
    time: &Time,
    first: CreatureId,
) {
    if !qp.enabled {
        return;
    }
    let ids: Vec<CreatureId> = store.living().filter(|c| c.id >= first).map(|c| c.id).collect();
    for id in ids {
        let Some(c) = store.get(id) else { continue };
        let kind = roster.get(c.species).kind;
        let parents = parent_sets(store, lineage, c.parents);
        let set = roll(qp, seed, id, kind, &parents);
        if set.is_empty() {
            continue;
        }
        let Some(c) = store.get_mut(id) else { continue };
        c.quirks = set;
        c.qm = fold(set, qp);
        let founder = c.parents.is_none();
        if founder && c.qm.lifespan < 1.0 {
            // A founder's age was drawn against its unquirked lifespan.
            c.born_day = crate::cast!((crate::cast!(c.born_day => f32) * c.qm.lifespan).round() => i32);
        }
        let label = c.label(roster);
        let (species, pos) = (c.species, (c.x, c.y));
        let legendary = set.iter().any(|i| qp.catalog.get(i).is_some_and(|d| d.tier == QuirkTier::Legendary));
        lineage.set_quirks(id, set, legendary);
        if !founder {
            announce(events, time, qp, set, &label, species, id, pos);
        }
    }
}

/// The quirk sets of a pup's parents (living, or remembered by the lineage).
fn parent_sets(store: &CreatureStore, lineage: &Lineage, parents: Option<(CreatureId, CreatureId)>) -> Vec<QuirkSet> {
    let Some((m, f)) = parents else { return Vec::new() };
    let of = |id: CreatureId| store.get(id).map(|c| c.quirks).or_else(|| lineage.get(id).map(|n| n.quirks)).unwrap_or_default();
    if m == f { vec![of(m)] } else { vec![of(m), of(f)] }
}

/// One Quirk event per pup born with a rare or legendary quirk.
#[allow(clippy::too_many_arguments)]
fn announce(events: &mut EventRing, time: &Time, qp: &QuirkParams, set: QuirkSet, label: &str, species: crate::sim::SpeciesId, id: CreatureId, pos: (usize, usize)) {
    let notable: Vec<&str> = set.iter().filter_map(|i| qp.catalog.get(i)).filter(|d| d.tier != QuirkTier::Common).map(|d| d.name.as_str()).collect();
    if notable.is_empty() {
        return;
    }
    let names: Vec<&str> = set.iter().filter_map(|i| qp.catalog.get(i)).map(|d| d.name.as_str()).collect();
    events.push(Event {
        year: time.year(),
        day: time.day_of_year(),
        hour: time.hour(),
        kind: EventKind::Quirk,
        species: Some(species),
        subject: Some(id),
        text: format!("{label} was born {}", notable.join(" and ")),
        pos: Some(pos),
        detail: format!("quirks: {}", names.join(", ")),
    });
}

/// Display names of a set, in catalogue order.
pub fn names(set: QuirkSet, qp: &QuirkParams) -> Vec<&str> {
    set.iter().filter_map(|i| qp.catalog.get(i)).map(|d| d.name.as_str()).collect()
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests;
