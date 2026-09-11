//! Living creatures: the entity record, slot storage and founder placement.
//!
//! Determinism rules (FR9): creatures are stored in slot order, ids are handed
//! out monotonically and never reused, and all per-tick updates iterate slots in
//! ascending order. Only ordered collections (`BTreeMap`/`Vec`) are used here.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::sim::params::CreaturesParams;
use crate::sim::rng::Rng;
use crate::sim::species::{names, Genome, SpeciesId};
use crate::sim::world::World;

/// Stable creature identifier, handed out monotonically and never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CreatureId(pub u32);

/// Index into the species' name list (`species::names`).
pub type NameId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sex {
    Male,
    Female,
}

/// The creature's current objective. `Display` yields the fixture strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Goal {
    Graze,
    Drink,
    Rest,
    Wander,
    Mate,
    Flee,
    Hunt,
    Scavenge,
    Migrate,
    Patrol,
}

/// Why a creature is resting (FR5 distinguishes three wake conditions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestReason {
    /// Entered because energy dropped below the rest threshold; wake at 0.9.
    Energy,
    /// Entered because it is night (diurnal); wake at sunrise.
    Night,
    /// Forced at energy ≤ 0; wake at 0.3.
    Forced,
}

/// Daily death tallies, reset after each census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeathTallies {
    pub starved: u32,
    pub thirst: u32,
    pub age: u32,
    pub predation: u32,
}

impl Goal {
    /// "resting in den" when resting inside a den, else the plain label.
    pub fn label(self, in_den: bool) -> &'static str {
        match self {
            Goal::Rest if in_den => "resting in den",
            _ => self.plain(),
        }
    }

    pub fn plain(self) -> &'static str {
        match self {
            Goal::Graze => "grazing",
            Goal::Drink => "seeking water",
            Goal::Rest => "resting",
            Goal::Wander => "wandering",
            Goal::Mate => "seeking mate",
            Goal::Flee => "fleeing",
            Goal::Hunt => "hunting",
            Goal::Scavenge => "scavenging",
            Goal::Migrate => "migrating",
            Goal::Patrol => "patrolling",
        }
    }
}

impl fmt::Display for Goal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.plain())
    }
}

/// Why a creature died. `Injury` is unused until predation lands in C5.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cause {
    Starved,
    Thirst,
    Age,
    Injury,
    Predation,
}

impl Cause {
    pub fn label(self) -> &'static str {
        match self {
            Cause::Starved => "starved",
            Cause::Thirst => "thirst",
            Cause::Age => "old age",
            Cause::Injury => "injury",
            Cause::Predation => "predation",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Death {
    pub cause: Cause,
    /// Absolute 0-based day index at which the creature died.
    pub day: u32,
    pub killer: Option<CreatureId>,
    pub chase_ticks: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mutation {
    pub trait_idx: usize,
    pub delta: f32,
    pub generation: u32,
}

/// The full creature record (FR2). C4/C5 fields are declared now but unused.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Creature {
    pub id: CreatureId,
    pub species: SpeciesId,
    pub name: NameId,
    pub sex: Sex,
    pub x: usize,
    pub y: usize,
    /// Negative for founders; S08 shows `founder`.
    pub born_day: i32,
    /// Founders = 1.
    pub generation: u32,
    pub parents: Option<(CreatureId, CreatureId)>,
    pub genome: Genome,
    pub hp: f32,
    pub hunger: f32,
    pub thirst: f32,
    pub energy: f32,
    pub adult: bool,
    pub goal: Goal,
    pub target: Option<(usize, usize)>,
    pub replan_at: u64,
    pub trail: Vec<(usize, usize)>,
    pub alive: bool,
    pub death: Option<Death>,
    pub decay: f32,
    pub mutations: Vec<Mutation>,
    /// Remembered drinking spot, set on every successful drink (FR5).
    pub last_water: Option<(usize, usize)>,
    /// Fractional movement accumulator (FR6).
    pub move_budget: f32,
    /// Present while resting, and the reason (FR5).
    pub rest_reason: Option<RestReason>,
    // ---- C4/C5 fields declared now, unused in C3 ----
    pub pregnant_due: Option<u64>,
    pub cooldown_until: u64,
    pub mother: Option<CreatureId>,
    pub offspring: u32,
    pub kills: u32,
    pub attempts: u32,
    pub chased: u32,
    pub escaped: u32,
    pub threats_by_species: [u32; 6],
    pub kills_by_species: [u32; 6],
    pub last_kill: Option<(CreatureId, u32)>,
    pub chase_stats: (u32, u32),
}

impl Creature {
    pub fn tag(&self) -> String {
        format!("{}#{:03}", self.species.glyph(), self.id.0)
    }

    /// Display name, resolved from the species name list.
    pub fn name_str(&self) -> &'static str {
        crate::sim::species::name_for(self.species, self.name)
    }

    /// Current age in days (derived from `born_day` and the day index).
    pub fn age_days(&self, day_index: u64) -> u32 {
        (day_index as i64 - self.born_day as i64).max(0) as u32
    }

    /// Maximum lifespan in days, from longevity and the C3 params.
    pub fn max_age_days(&self, params: &CreaturesParams) -> u32 {
        params.max_age_base + (self.genome.longevity() * params.max_age_per_longevity as f32) as u32
    }
}

/// Slot storage: `Vec<Option<Creature>>` with a free list for reusable slots and
/// a monotonically increasing `CreatureId` that is never reused (FR Scope).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreatureStore {
    slots: Vec<Option<Creature>>,
    free: Vec<u32>,
    id_to_slot: BTreeMap<CreatureId, u32>,
    next_id: u32,
}

impl Default for CreatureStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CreatureStore {
    pub fn new() -> Self {
        CreatureStore { slots: Vec::new(), free: Vec::new(), id_to_slot: BTreeMap::new(), next_id: 1 }
    }

    /// Insert a creature, assigning a fresh id (ignoring any id on the input).
    pub fn insert(&mut self, mut c: Creature) -> CreatureId {
        let id = CreatureId(self.next_id);
        self.next_id += 1;
        c.id = id;
        let slot = match self.free.pop() {
            Some(i) => i,
            None => {
                self.slots.push(None);
                (self.slots.len() - 1) as u32
            }
        };
        self.slots[slot as usize] = Some(c);
        self.id_to_slot.insert(id, slot);
        id
    }

    pub fn get(&self, id: CreatureId) -> Option<&Creature> {
        let slot = *self.id_to_slot.get(&id)? as usize;
        self.slots[slot].as_ref()
    }

    pub fn get_mut(&mut self, id: CreatureId) -> Option<&mut Creature> {
        let slot = *self.id_to_slot.get(&id)? as usize;
        self.slots[slot].as_mut()
    }

    /// Free a creature's slot entirely (carcass fully decayed).
    pub fn remove(&mut self, id: CreatureId) -> Option<Creature> {
        let slot = self.id_to_slot.remove(&id)? as usize;
        let out = self.slots[slot].take();
        self.free.push(slot as u32);
        out
    }

    /// Living creatures in slot order (FR9).
    pub fn living(&self) -> impl Iterator<Item = &Creature> {
        self.slots.iter().filter_map(|s| s.as_ref()).filter(|c| c.alive)
    }

    /// Living creatures in slot order, mutable.
    pub fn living_mut(&mut self) -> impl Iterator<Item = &mut Creature> {
        self.slots.iter_mut().filter_map(|s| s.as_mut()).filter(|c| c.alive)
    }

    /// Dead-but-not-yet-freed creatures (carcasses) in slot order.
    pub fn carcasses(&self) -> impl Iterator<Item = &Creature> {
        self.slots.iter().filter_map(|s| s.as_ref()).filter(|c| !c.alive)
    }

    pub fn carcasses_mut(&mut self) -> impl Iterator<Item = &mut Creature> {
        self.slots.iter_mut().filter_map(|s| s.as_mut()).filter(|c| !c.alive)
    }

    pub fn len_living(&self) -> usize {
        self.slots.iter().filter(|s| s.as_ref().map_or(false, |c| c.alive)).count()
    }

    /// Living ids in ascending order (deterministic iteration).
    pub fn living_ids(&self) -> Vec<CreatureId> {
        let mut v: Vec<CreatureId> = self.living().map(|c| c.id).collect();
        v.sort_unstable();
        v
    }
}

/// Place founders per FR3. Deterministic: iterates `SpeciesId::ALL` order and
/// draws from `rng` in a fixed sequence.
pub fn place_founders(world: &World, params: &CreaturesParams, rng: &mut Rng) -> Vec<Creature> {
    let mut out = Vec::new();
    for species in SpeciesId::ALL {
        let n = params.initial_counts.get(&species).copied().unwrap_or(0);
        if n == 0 {
            continue;
        }
        let base = species.base_genome();
        let adult_age = params.adult_age(species);
        let name_pool_len = names(species).len();
        let mut placed = 0u32;
        let mut tries = 0u32;
        while placed < n && tries < 100_000 {
            tries += 1;
            let x = rng.below(world.width);
            let y = rng.below(world.height);
            let cell = world.cell(x, y);
            if !cell.terrain.walkable() || cell.terrain.is_water() {
                continue;
            }
            // Prey prefer vegetation (FR3 acceptance probability).
            if !rng.chance(0.3 + 0.7 * cell.vegetation) {
                continue;
            }

            let mut genome = base;
            for v in genome.0.iter_mut() {
                *v = (*v + rng.gauss(0.0, 0.12)).clamp(0.02, 0.98);
            }
            let max_age = params.max_age_base + (genome.longevity() * params.max_age_per_longevity as f32) as u32;
            let adult = rng.chance(0.7);
            let age_days = if adult {
                let upper = adult_age.max((0.75 * max_age as f32) as u32);
                adult_age + rng.below((upper - adult_age + 1) as usize) as u32
            } else {
                rng.below(adult_age.max(1) as usize) as u32
            };
            let sex = if rng.chance(0.5) { Sex::Male } else { Sex::Female };
            let name = rng.below(name_pool_len) as NameId;

            out.push(Creature {
                id: CreatureId(0),
                species,
                name,
                sex,
                x,
                y,
                born_day: -(age_days as i32),
                generation: 1,
                parents: None,
                genome,
                hp: 1.0,
                hunger: rng.f32() * 0.4,
                thirst: rng.f32() * 0.4,
                energy: 0.6 + rng.f32() * 0.4,
                adult: age_days >= adult_age,
                goal: Goal::Wander,
                target: None,
                replan_at: 0,
                trail: Vec::new(),
                alive: true,
                death: None,
                decay: 0.0,
                mutations: Vec::new(),
                last_water: None,
                move_budget: 0.0,
                rest_reason: None,
                pregnant_due: None,
                cooldown_until: 0,
                mother: None,
                offspring: 0,
                kills: 0,
                attempts: 0,
                chased: 0,
                escaped: 0,
                threats_by_species: [0; 6],
                kills_by_species: [0; 6],
                last_kill: None,
                chase_stats: (0, 0),
            });
            placed += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::params::CreaturesParams;
    use crate::sim::world::{Terrain, World};

    fn world() -> World {
        World::generate(7, &crate::sim::params::WorldParams::default())
    }

    #[test]
    fn placement_respects_terrain() {
        let w = world();
        let p = CreaturesParams::default();
        let mut rng = Rng::new(42);
        let creatures = place_founders(&w, &p, &mut rng);
        assert!(!creatures.is_empty(), "expected founders to be placed");
        for c in &creatures {
            let cell = w.cell(c.x, c.y);
            assert!(cell.terrain.walkable(), "creature on impassable {:?}", cell.terrain);
            assert!(!cell.terrain.is_water(), "creature on water");
        }
        // Counts match the requested initial_counts (within the retry budget).
        for id in SpeciesId::ALL {
            let want = p.initial_counts.get(&id).copied().unwrap_or(0);
            let got = creatures.iter().filter(|c| c.species == id).count() as u32;
            assert_eq!(got, want, "{:?}: placed {got}, want {want}", id);
        }
    }

    #[test]
    fn initial_ages_below_max_age() {
        let w = world();
        let p = CreaturesParams::default();
        let mut rng = Rng::new(42);
        for c in place_founders(&w, &p, &mut rng) {
            let age = c.age_days(0);
            assert!(age < c.max_age_days(&p), "age {age} >= max {}", c.max_age_days(&p));
        }
    }

    #[test]
    fn slot_storage_free_list_and_stable_ids() {
        let mut store = CreatureStore::new();
        let mut c = place_founders(&world(), &CreaturesParams::default(), &mut Rng::new(1)).remove(0);
        c.id = CreatureId(0);
        let a = store.insert(c.clone());
        let b = store.insert(c.clone());
        assert_ne!(a, b);
        assert_eq!(a, CreatureId(1));
        assert_eq!(b, CreatureId(2));
        assert_eq!(store.len_living(), 2);
        store.remove(a);
        assert_eq!(store.len_living(), 1);
        // The freed slot is reused, but the id is never reused.
        let d = store.insert(c);
        assert_eq!(d, CreatureId(3));
        assert!(store.get(a).is_none());
        assert!(store.get(d).is_some());
    }

    #[test]
    fn goal_labels() {
        assert_eq!(Goal::Rest.label(false), "resting");
        assert_eq!(Goal::Rest.label(true), "resting in den");
        assert_eq!(Goal::Drink.plain(), "seeking water");
        assert_eq!(Goal::Graze.to_string(), "grazing");
    }

    #[test]
    fn terrain_never_water() {
        // Regression guard: water cells must not be walkable placement targets.
        assert!(Terrain::DeepWater.is_water());
        assert!(Terrain::ShallowWater.is_water());
        assert!(!Terrain::Grass.is_water());
    }
}
