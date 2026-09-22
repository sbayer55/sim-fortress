//! S17 Hunt Watch: one trace per predator that has ever hunted.
//!
//! Passive bookkeeping for the screen: `behavior::hunt` opens a trace when a
//! hunter first has a target, `observe` pushes one sample per tick after
//! movement, and the resolution passes close it with the outcome. The step
//! never reads a trace, nothing here draws RNG or emits an event, and the
//! table stays out of the checksum; it is saved with the world so a loaded
//! valley shows the same ribbons.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::sim::creatures::{Cause, Creature, CreatureId, CreatureStore, Goal, HuntPhase};
use crate::sim::geom;
use crate::sim::species::SpeciesId;
use crate::sim::world::{Terrain, World};

/// Samples kept per hunt: six stalk ticks before the clock plus the 30-tick chase clock.
pub const RIBBON_LEN: usize = 36;
/// Attempts remembered per hunter, most recent first.
pub const HISTORY_LEN: u8 = 12;

/// One tick of an open hunt, taken after everyone has moved.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HuntSample {
    pub tick: u64,
    /// Chebyshev gap in cells, saturated at 255.
    pub gap: u8,
    pub phase: HuntPhase,
    pub hunter_energy: f32,
    pub prey_energy: f32,
    pub hunter_hunger: f32,
    /// The prey's goal this tick: Flee or Wary means it has noticed.
    pub prey_goal: Goal,
    /// The terrain under the prey, for the cover narration.
    pub prey_terrain: Terrain,
}

impl HuntSample {
    pub fn of(hunter: &Creature, prey: &Creature, terrain: Terrain, tick: u64) -> Self {
        Self {
            tick,
            gap: crate::cast!(geom::cheb(hunter.x, hunter.y, prey.x, prey.y).min(255) => u8),
            phase: hunter.hunt_phase,
            hunter_energy: hunter.energy,
            prey_energy: prey.energy,
            hunter_hunger: hunter.hunger,
            prey_goal: prey.goal,
            prey_terrain: terrain,
        }
    }
}

/// How a hunt ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HuntOutcome {
    /// A kill; `eat_until` lets the screen show eating and fed without a second hook.
    Kill { eat_until: u64 },
    /// The contact roll failed and the prey was forced to flee.
    Miss,
    /// The chase clock ran out.
    Timeout,
    /// The prey left the hunter's sense range or died of something else.
    Lost,
    /// The hunter's replan chose another goal mid-hunt; the sim counts no attempt.
    Dropped,
}

impl HuntOutcome {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Kill { .. } => "kill",
            Self::Miss => "miss",
            Self::Timeout => "timed out",
            Self::Lost => "lost",
            Self::Dropped => "dropped",
        }
    }

    pub const fn is_kill(self) -> bool {
        matches!(self, Self::Kill { .. })
    }

    /// Mirrors `Creature.attempts`: kills and every `fail_hunt` count, a drop does not.
    pub const fn counts_attempt(self) -> bool {
        !matches!(self, Self::Dropped)
    }
}

/// The resolution of a hunt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HuntEnd {
    pub outcome: HuntOutcome,
    pub tick: u64,
    pub chase_ticks: u16,
}

/// The first tick the hunter entered Chase, and its energy then (the legs budget).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChaseStart {
    pub tick: u64,
    pub hunter_energy: f32,
}

/// A dead hunter's trace lingers until its carcass slot is freed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HunterDeath {
    pub cause: Cause,
    pub tick: u64,
}

/// Who is hunting whom; the key `begin` and `end` are called with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HuntKey {
    pub hunter: CreatureId,
    pub species: SpeciesId,
    pub prey: CreatureId,
}

/// One predator's current or last hunt, plus its attempt history.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HuntTrace {
    hunter: CreatureId,
    species: SpeciesId,
    prey: CreatureId,
    /// The first tick this hunt was observed (Stalk).
    start_tick: u64,
    /// The first Stalk → Chase of this hunt; not reset by the six-tick re-pick.
    chase: Option<ChaseStart>,
    /// Oldest first, at most `RIBBON_LEN`.
    samples: VecDeque<HuntSample>,
    end: Option<HuntEnd>,
    /// Bit 0 is the most recent attempt; 1 = kill.
    history: u16,
    history_len: u8,
    died: Option<HunterDeath>,
}

impl HuntTrace {
    fn new(key: HuntKey, tick: u64) -> Self {
        Self {
            hunter: key.hunter,
            species: key.species,
            prey: key.prey,
            start_tick: tick,
            chase: None,
            samples: VecDeque::with_capacity(RIBBON_LEN),
            end: None,
            history: 0,
            history_len: 0,
            died: None,
        }
    }

    /// Start a new hunt on this trace: the history survives, everything else resets.
    fn restart(&mut self, prey: CreatureId, tick: u64) {
        self.prey = prey;
        self.start_tick = tick;
        self.chase = None;
        self.samples.clear();
        self.end = None;
    }

    fn push_sample(&mut self, s: HuntSample) {
        if self.samples.len() >= RIBBON_LEN {
            self.samples.pop_front();
        }
        self.samples.push_back(s);
    }

    fn close(&mut self, outcome: HuntOutcome, tick: u64, chase_ticks: u16) {
        self.end = Some(HuntEnd { outcome, tick, chase_ticks });
        if outcome.counts_attempt() {
            self.history = ((self.history << 1) | u16::from(outcome.is_kill())) & ((1 << HISTORY_LEN) - 1);
            self.history_len = (self.history_len + 1).min(HISTORY_LEN);
        }
    }

    /// Ticks since the chase clock started, saturated to `u16`.
    fn chase_ticks_at(&self, tick: u64) -> u16 {
        self.chase.map_or(0, |c| crate::cast!(tick.saturating_sub(c.tick).min(u64::from(u16::MAX)) => u16))
    }

    pub const fn hunter(&self) -> CreatureId {
        self.hunter
    }
    pub const fn species(&self) -> SpeciesId {
        self.species
    }
    pub const fn prey(&self) -> CreatureId {
        self.prey
    }
    pub const fn start_tick(&self) -> u64 {
        self.start_tick
    }
    pub const fn chase(&self) -> Option<ChaseStart> {
        self.chase
    }
    pub fn samples(&self) -> impl Iterator<Item = &HuntSample> {
        self.samples.iter()
    }
    pub fn last_sample(&self) -> Option<&HuntSample> {
        self.samples.back()
    }
    pub const fn end(&self) -> Option<HuntEnd> {
        self.end
    }
    pub const fn died(&self) -> Option<HunterDeath> {
        self.died
    }
    /// Still hunting: no resolution and the hunter is alive.
    pub const fn is_open(&self) -> bool {
        self.end.is_none() && self.died.is_none()
    }
    pub const fn history_len(&self) -> u8 {
        self.history_len
    }
    /// The remembered attempts, most recent first; `true` is a kill.
    pub fn history(&self) -> impl Iterator<Item = bool> + '_ {
        (0..self.history_len).map(move |i| (self.history >> i) & 1 == 1)
    }
}

/// Every predator that has hunted, by id.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HuntWatch {
    traces: BTreeMap<CreatureId, HuntTrace>,
}

impl HuntWatch {
    /// A hunter has a target this tick. A no-op while the same hunt is open; a
    /// switched target closes the old hunt as dropped and starts the new one.
    pub fn begin(&mut self, key: HuntKey, tick: u64) {
        match self.traces.get_mut(&key.hunter) {
            Some(t) if t.is_open() && t.prey == key.prey => {}
            Some(t) => {
                if t.is_open() {
                    let ct = t.chase_ticks_at(tick);
                    t.close(HuntOutcome::Dropped, tick, ct);
                }
                t.restart(key.prey, tick);
            }
            None => {
                self.traces.insert(key.hunter, HuntTrace::new(key, tick));
            }
        }
    }

    /// A hunt resolved. An upsert: a hunt that opens and closes on the same
    /// tick still leaves a one-sample trace with its outcome.
    pub fn end(&mut self, key: HuntKey, outcome: HuntOutcome, tick: u64, chase_ticks: u16) {
        let t = self.traces.entry(key.hunter).or_insert_with(|| HuntTrace::new(key, tick));
        if t.prey != key.prey || !t.is_open() {
            t.restart(key.prey, tick);
        }
        t.close(outcome, tick, chase_ticks);
    }

    /// One pass per tick after movement: sample every open hunt, mirror the
    /// chase start, close hunts the replan silently abandoned, note dead
    /// hunters, and forget hunters whose slot is gone.
    pub fn observe(&mut self, store: &CreatureStore, world: &World, tick: u64) {
        for t in self.traces.values_mut() {
            if let Some(c) = store.get(t.hunter) {
                Self::observe_one(t, c, store, world, tick);
            }
        }
        self.prune(|id| store.get(id).is_some());
    }

    fn observe_one(t: &mut HuntTrace, c: &Creature, store: &CreatureStore, world: &World, tick: u64) {
        if !c.alive {
            if t.died.is_none() {
                t.died = Some(HunterDeath { cause: c.death.map_or(Cause::Starved, |d| d.cause), tick });
            }
            return;
        }
        if !t.is_open() {
            return;
        }
        let hunting = c.goal == Goal::Hunt && c.hunt_phase != HuntPhase::Eat && c.hunt_target == Some(t.prey);
        if !hunting {
            let ct = t.chase_ticks_at(tick);
            t.close(HuntOutcome::Dropped, tick, ct);
            return;
        }
        if t.chase.is_none() {
            if let Some(start) = c.chase_start_tick {
                t.chase = Some(ChaseStart { tick: start, hunter_energy: c.energy });
            }
        }
        if let Some(q) = store.get(t.prey) {
            let terrain = world.cell(q.x, q.y).terrain;
            t.push_sample(HuntSample::of(c, q, terrain, tick));
        }
    }

    /// Drop the traces of hunters that no longer exist.
    pub fn prune(&mut self, alive: impl Fn(CreatureId) -> bool) {
        self.traces.retain(|&id, _| alive(id));
    }

    pub fn trace(&self, id: CreatureId) -> Option<&HuntTrace> {
        self.traces.get(&id)
    }

    /// Every trace, by hunter id.
    pub fn traces(&self) -> impl Iterator<Item = &HuntTrace> {
        self.traces.values()
    }

    pub fn len(&self) -> usize {
        self.traces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.traces.is_empty()
    }
}

#[cfg(test)]
mod tests;
