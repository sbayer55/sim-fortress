//! Living creatures: the entity record, slot storage and founder placement.
//!
//! Determinism rules (FR9): creatures are stored in slot order, ids are handed
//! out monotonically and never reused, and all per-tick updates iterate slots in
//! ascending order. Only ordered collections (`BTreeMap`/`Vec`) are used here.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::sim::disease::{Infection, PathogenId};
use crate::sim::params::{CreaturesParams, GeneticsParams, Roster, SpeciesParams};
use crate::sim::rng::Rng;
use crate::sim::species::{Genome, SpeciesId, IDX_RESISTANCE};
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
    /// C5 `FR5b`: the low-exertion avoidance tier — a predator that is not
    /// currently a danger is nearby and the prey is giving it room.
    Wary,
}

/// The phase of a predator's current hunt (C5 FR4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum HuntPhase {
    Stalk,
    Chase,
    Eat,
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

/// The retained death record of a species' most recent death (C5 FR8): the
/// S12 modal names the last individual from it even after the carcass slot is
/// freed, and it is cleared when the alert is dismissed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtinctionRecord {
    pub species: SpeciesId,
    pub last: CreatureId,
    pub name: String,
    pub tag: String,
    pub cause: Cause,
    pub day: u32,
    pub age: u32,
    pub region: String,
    pub pos: (usize, usize),
}

/// Daily death tallies, reset after each census. The C5 fields (`hunt_*`,
/// `last_death`) are cumulative and survive the daily reset (see `Sim::step`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeathTallies {
    pub starved: u32,
    pub thirst: u32,
    pub age: u32,
    pub predation: u32,
    /// Disease deaths today (C7), total and per species.
    pub disease: u32,
    pub disease_by_species: Vec<u32>,
    /// Births per species today (C4 FR5), roster order.
    pub births: Vec<u32>,
    /// Deaths per species today (C4 FR5), roster order.
    pub deaths: Vec<u32>,
    /// Cumulative hunt attempts per predator species (C5; survives carcass freeing).
    pub hunt_attempts: Vec<u32>,
    /// Cumulative kills per predator species (C5).
    pub hunt_kills: Vec<u32>,
    /// The most recent death per species (C5 FR8).
    pub last_death: Vec<Option<ExtinctionRecord>>,
    /// C5 `FR5b`: wary encounters today, keyed `(region index, prey species,
    /// predator species)`. Flushed into one Wary event per region at the day
    /// boundary and cleared by `next_day`.
    pub wary_today: BTreeMap<(u8, SpeciesId, SpeciesId), u32>,
}

impl DeathTallies {
    /// Empty tallies for a roster of `n_species`.
    pub fn new(n_species: usize) -> Self {
        Self {
            disease_by_species: vec![0; n_species],
            births: vec![0; n_species],
            deaths: vec![0; n_species],
            hunt_attempts: vec![0; n_species],
            hunt_kills: vec![0; n_species],
            last_death: vec![None; n_species],
            ..Self::default()
        }
    }

    /// A fresh daily tally that keeps the cumulative C5 fields.
    #[must_use]
    pub fn next_day(&self) -> Self {
        Self {
            hunt_attempts: self.hunt_attempts.clone(),
            hunt_kills: self.hunt_kills.clone(),
            last_death: self.last_death.clone(),
            ..Self::new(self.births.len())
        }
    }
}

impl Goal {
    /// "resting in den" when resting inside a den, else the plain label.
    pub const fn label(self, in_den: bool) -> &'static str {
        match self {
            Self::Rest if in_den => "resting in den",
            _ => self.plain(),
        }
    }

    pub const fn plain(self) -> &'static str {
        match self {
            Self::Graze => "grazing",
            Self::Drink => "seeking water",
            Self::Rest => "resting",
            Self::Wander => "wandering",
            Self::Mate => "seeking mate",
            Self::Flee => "fleeing",
            Self::Hunt => "hunting",
            Self::Scavenge => "scavenging",
            Self::Migrate => "migrating",
            Self::Patrol => "patrolling",
            Self::Wary => "wary",
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
    /// C7: a pathogen or a parasite load (FR5/FR6).
    Disease,
}

impl Cause {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Starved => "starved",
            Self::Thirst => "thirst",
            Self::Age => "old age",
            Self::Injury => "injury",
            Self::Predation => "predation",
            Self::Disease => "disease",
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

/// The full creature record (FR2). C5 fields are declared now but unused.
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
    /// Born sterile (a high-Mutability cost): never eligible to mate.
    pub sterile: bool,
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
    /// Tick of the last meal (graze, kill, shared kill or scavenge); S01e / S03 "last ate".
    pub last_ate: Option<u64>,
    /// Tick of the last drink; S01e / S03 "last drank".
    pub last_drank: Option<u64>,
    /// Tick of the last hour spent resting; S01e / S03 "last slept".
    pub last_slept: Option<u64>,
    /// Fractional movement accumulator (FR6).
    pub move_budget: f32,
    /// Remaining planned steps (next step last) when the greedy step stalled
    /// at an obstacle and a bounded path search took over.
    pub path: Vec<(usize, usize)>,
    /// The target `path` was searched for; a different target invalidates it.
    pub path_for: Option<(usize, usize)>,
    /// Present while resting, and the reason (FR5).
    pub rest_reason: Option<RestReason>,
    // ---- C4 reproduction / C5 predation fields ----
    pub pregnant_due: Option<u64>,
    pub cooldown_until: u64,
    /// Intended partner while the goal is `Mate`; after mating the female keeps
    /// the father's id (S03 "mate"), the male's is cleared.
    pub mate_id: Option<CreatureId>,
    pub mother: Option<CreatureId>,
    pub offspring: u32,
    pub kills: u32,
    pub attempts: u32,
    pub chased: u32,
    pub escaped: u32,
    pub threats_by_species: Vec<u32>,
    pub kills_by_species: Vec<u32>,
    /// Last kill: `(victim id, day index, region index)` (C5 FR4).
    pub last_kill: Option<(CreatureId, u32, u8)>,
    /// `(sum of chase ticks, longest chase ticks)`; longest year kept beside it.
    pub chase_stats: (u32, u32),
    pub chase_longest_year: u32,
    // ---- C5 hunt / flee / scavenge / migration state ----
    pub hunt_phase: HuntPhase,
    /// The prey being hunted (cleared when the hunt ends).
    pub hunt_target: Option<CreatureId>,
    /// Tick at which the chase clock started (Stalk → Chase transition).
    pub chase_start_tick: Option<u64>,
    pub hunt_cooldown_until: u64,
    /// Tick at which the Eat phase ends; the carcass is `target`.
    pub eat_until: Option<u64>,
    /// The prey carcass being scavenged.
    pub scavenge_target: Option<CreatureId>,
    /// Tick at which Flee ends.
    pub flee_until: u64,
    /// Nearest threatening predator `(x, y, species)` driving the away-vector (per tick).
    pub threatened_by: Option<(usize, usize, SpeciesId)>,
    /// `min(1, 0.5 × pred_pressure + 0.5 × predators_in_range / 3)` (S03 Condition).
    pub predation_risk: f32,
    // ---- C5 `FR5b` wary (the second, low-exertion avoidance tier) ----
    /// Tick at which the wary state ends.
    pub wary_until: u64,
    /// The non-danger predator `(x, y, species)` driving the wary away-vector.
    pub wary_by: Option<(usize, usize, SpeciesId)>,
    /// Times this creature has turned wary (S03).
    pub wary_count: u32,
    /// C8: same-species neighbours seen at the last replan (S03 "kin nearby").
    pub kin_nearby: u8,
    pub migrate_until: u64,
    pub migrate_target: Option<(usize, usize)>,
    // ---- C7 disease / parasites ----
    /// The current contagious infection, at most one at a time (FR3).
    pub infection: Option<Infection>,
    /// Immunity per pathogen slot as a day index (`0` none, `u32::MAX` lifelong).
    pub immune_until: [u32; 8],
    /// Continuous parasite ("gut worm") load 0..=1.
    pub parasite_load: f32,
    pub infections_survived: u8,
    /// Set at death when the creature was infectious: carcass transmission and S03c.
    pub died_infected: Option<PathogenId>,
}

impl Creature {
    /// `v#001`: the species glyph and the creature id.
    pub fn tag(&self, roster: &Roster) -> String {
        format!("{}#{:03}", roster.get(self.species).glyph, self.id.0)
    }

    /// Display name, resolved from the species' name pool.
    pub fn name_str<'r>(&self, roster: &'r Roster) -> &'r str {
        roster.name_for(self.species, self.name)
    }

    /// `Clover v#001`: name and tag, as event text refers to a creature.
    pub fn label(&self, roster: &Roster) -> String {
        format!("{} {}", self.name_str(roster), self.tag(roster))
    }

    /// Current age in days (derived from `born_day` and the day index).
    pub fn age_days(&self, day_index: u64) -> u32 {
        crate::cast!((crate::cast!(day_index => i64) - i64::from(self.born_day)).max(0) => u32)
    }

    /// Maximum lifespan in days, from longevity, the C3 params and the maturity
    /// trait (a slow life history lives longer).
    pub fn max_age_days(&self, params: &CreaturesParams, gp: &GeneticsParams) -> u32 {
        max_age_days(&self.genome, params, gp)
    }
}

/// Maximum lifespan in days for a genome; `Creature::max_age_days` delegates here
/// so founder placement can use it before the `Creature` exists.
pub fn max_age_days(genome: &Genome, params: &CreaturesParams, gp: &GeneticsParams) -> u32 {
    let base = params.max_age_base + crate::cast!((genome.longevity() * crate::cast!(params.max_age_per_longevity => f32)) => u32);
    crate::cast!((crate::cast!(base => f32) * GeneticsParams::maturity_factor(genome.maturity(), gp.maturity_lifespan_span)).round() => u32)
}

/// Days to adulthood: the species' base age scaled by the individual's maturity
/// trait, rounded to whole days. Maturity 0.5 reproduces `adult_age_days` exactly.
pub fn adult_age_days(sp: &SpeciesParams, genome: &Genome, gp: &GeneticsParams) -> u32 {
    let base = crate::cast!(sp.adult_age_days => f32);
    crate::cast!((base * GeneticsParams::maturity_factor(genome.maturity(), gp.maturity_age_span)).round() => u32)
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
    pub const fn new() -> Self {
        Self { slots: Vec::new(), free: Vec::new(), id_to_slot: BTreeMap::new(), next_id: 1 }
    }

    /// Insert a creature, assigning a fresh id (ignoring any id on the input).
    pub fn insert(&mut self, mut c: Creature) -> CreatureId {
        let id = CreatureId(self.next_id);
        self.next_id += 1;
        c.id = id;
        let slot = if let Some(i) = self.free.pop() { i } else {
            self.slots.push(None);
            crate::cast!((self.slots.len() - 1) => u32)
        };
        self.slots[crate::cast!(slot => usize)] = Some(c);
        self.id_to_slot.insert(id, slot);
        id
    }

    pub fn get(&self, id: CreatureId) -> Option<&Creature> {
        let slot = crate::cast!(*self.id_to_slot.get(&id)? => usize);
        self.slots[slot].as_ref()
    }

    pub fn get_mut(&mut self, id: CreatureId) -> Option<&mut Creature> {
        let slot = crate::cast!(*self.id_to_slot.get(&id)? => usize);
        self.slots[slot].as_mut()
    }

    /// Free a creature's slot entirely (carcass fully decayed).
    pub fn remove(&mut self, id: CreatureId) -> Option<Creature> {
        let slot = crate::cast!(self.id_to_slot.remove(&id)? => usize);
        let out = self.slots[slot].take();
        self.free.push(crate::cast!(slot => u32));
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
        self.slots.iter().filter(|s| s.as_ref().is_some_and(|c| c.alive)).count()
    }

    /// Living ids in ascending order (deterministic iteration).
    pub fn living_ids(&self) -> Vec<CreatureId> {
        let mut v: Vec<CreatureId> = self.living().map(|c| c.id).collect();
        v.sort_unstable();
        v
    }
}

/// Build one founder creature (all per-founder state at its defaults).
#[allow(clippy::too_many_arguments)]
fn founder(species: SpeciesId, n_species: usize, name: NameId, sex: Sex, pos: (usize, usize), age_days: u32, genome: Genome, adult_age: u32, sterile: bool, rng: &mut Rng) -> Creature {
    let (x, y) = pos;
    Creature {
                id: CreatureId(0),
                species,
                name,
                sex,
                x,
                y,
                born_day: -(crate::cast!(age_days => i32)),
                generation: 1,
                parents: None,
                genome,
                hp: 1.0,
                hunger: rng.f32() * 0.4,
                thirst: rng.f32() * 0.4,
                energy: 0.6 + rng.f32() * 0.4,
                adult: age_days >= adult_age,
                sterile,
                goal: Goal::Wander,
                target: None,
                replan_at: 0,
                trail: Vec::new(),
                alive: true,
                death: None,
                decay: 0.0,
                mutations: Vec::new(),
                last_water: None,
                last_ate: None,
                last_drank: None,
                last_slept: None,
                move_budget: 0.0,
                path: Vec::new(),
                rest_reason: None,
                pregnant_due: None,
                cooldown_until: 0,
                mate_id: None,
                mother: None,
                offspring: 0,
                kills: 0,
                attempts: 0,
                chased: 0,
                escaped: 0,
                threats_by_species: vec![0; n_species],
                kills_by_species: vec![0; n_species],
                last_kill: None,
                chase_stats: (0, 0),
                chase_longest_year: 0,
                hunt_phase: HuntPhase::Stalk,
                hunt_target: None,
                chase_start_tick: None,
                hunt_cooldown_until: 0,
                eat_until: None,
                scavenge_target: None,
                flee_until: 0,
                threatened_by: None,
                predation_risk: 0.0,
                wary_until: 0,
                wary_by: None,
                wary_count: 0,
                kin_nearby: 0,
                migrate_until: 0,
                // ---- C7 disease / parasites
                infection: None,
                immune_until: [0; 8],
                parasite_load: 0.0,
                infections_survived: 0,
                died_infected: None,
                migrate_target: None,
                path_for: None,
            }
}

/// Place founders per FR3.
///
/// Deterministic: iterates the roster in order and draws from `rng` in a fixed
/// sequence. Adult age and lifespan are read from each founder's own genome, so
/// a slow-maturing individual starts older.
pub fn place_founders(world: &World, roster: &Roster, params: &CreaturesParams, gp: &GeneticsParams, resistance_sd: f32, rng: &mut Rng) -> Vec<Creature> {
    let mut out = Vec::new();
    for species in roster.ids() {
        let sp = roster.get(species);
        let n = sp.initial_count;
        if n == 0 {
            continue;
        }
        let base = sp.base_genome.genome();
        let name_pool_len = sp.name_pool_len();
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
            for (t, v) in genome.0.iter_mut().enumerate() {
                // C7: Resistance starts with a wider spread (`resistance_sd`).
                let sd = if t == IDX_RESISTANCE { resistance_sd } else { 0.12 };
                *v = Genome::clamp_trait(*v + rng.gauss(0.0, sd));
            }
            // Maturity moves both ends of the life history for this individual.
            let adult_age = adult_age_days(sp, &genome, gp);
            let max_age = max_age_days(&genome, params, gp);
            let adult = rng.chance(0.7);
            let age_days = if adult {
                let upper = adult_age.max(crate::cast!((0.75 * crate::cast!(max_age => f32)) => u32));
                adult_age + crate::cast!(rng.below(crate::cast!((upper - adult_age + 1) => usize)) => u32)
            } else {
                crate::cast!(rng.below(crate::cast!(adult_age.max(1) => usize)) => u32)
            };
            let sex = if rng.chance(0.5) { Sex::Male } else { Sex::Female };
            let name = crate::cast!(rng.below(name_pool_len) => NameId);
            // The same sterility roll a newborn gets, so the rule has no exceptions.
            let sterile = rng.chance(gp.sterility_chance(genome.mutability()));

            out.push(founder(species, roster.len(), name, sex, (x, y), age_days, genome, adult_age, sterile, rng));
            placed += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::species::testing::*;
    use crate::sim::params::{CreaturesParams, GeneticsParams, Roster};
    use crate::sim::world::{Terrain, World};

    fn world() -> World {
        World::generate(7, &crate::sim::params::WorldParams::default())
    }

    #[test]
    fn placement_respects_terrain() {
        let w = world();
        let p = CreaturesParams::default();
        let mut rng = Rng::new(42);
        let roster = Roster::default();
        let creatures = place_founders(&w, &roster, &p, &GeneticsParams::default(), 0.20, &mut rng);
        assert!(!creatures.is_empty(), "expected founders to be placed");
        for c in &creatures {
            let cell = w.cell(c.x, c.y);
            assert!(cell.terrain.walkable(), "creature on impassable {:?}", cell.terrain);
            assert!(!cell.terrain.is_water(), "creature on water");
        }
        // Counts match the requested initial_count (within the retry budget).
        for id in roster.ids() {
            let want = roster.get(id).initial_count;
            let got = crate::cast!(creatures.iter().filter(|c| c.species == id).count() => u32);
            assert_eq!(got, want, "{id:?}: placed {got}, want {want}");
        }
    }

    #[test]
    fn initial_ages_below_max_age() {
        let w = world();
        let p = CreaturesParams::default();
        let mut rng = Rng::new(42);
        for c in place_founders(&w, &Roster::default(), &p, &GeneticsParams::default(), 0.20, &mut rng) {
            let age = c.age_days(0);
            assert!(age < c.max_age_days(&p, &GeneticsParams::default()), "age {age} >= max {}", c.max_age_days(&p, &GeneticsParams::default()));
        }
    }

    #[test]
    fn slot_storage_free_list_and_stable_ids() {
        let mut store = CreatureStore::new();
        let mut c = place_founders(&world(), roster(), &CreaturesParams::default(), &GeneticsParams::default(), 0.20, &mut Rng::new(1)).remove(0);
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
