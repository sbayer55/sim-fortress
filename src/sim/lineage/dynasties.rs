//! Dynasties (S16 Top Dynasties): a founder and every animal whose mother line
//! leads back to it, with six stats over the life of the line.
//!
//! A dynasty is keyed by its root, the `LineageNode::root` of every member.
//! The store keeps what pruning would lose: the founder's label, the member
//! counts, the totals of the members that have died, and one closed row per
//! year for the race charts. Living members are read live from the store, so
//! the totals shown are `dead + living`. Predators only (D2).
//!
//! Passive: it draws no RNG and emits no events, so the checksum is unchanged.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::sim::creatures::{Creature, CreatureId};
use crate::sim::params::Roster;
use crate::sim::species::SpeciesId;

#[cfg(test)]
mod tests;

/// Closed year rows kept per dynasty (the race chart window).
pub const YEARS_KEPT: usize = 40;
/// Years an extinct dynasty is kept after its last member died.
pub const EXTINCT_KEEP_YEARS: u32 = 10;

/// The six S16 stats: running totals (kills, young, surv, muts) and levels at
/// year end (terr = cells held by the living, age = the oldest living, days).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    pub kills: u32,
    pub terr: u32,
    pub young: u32,
    pub surv: u32,
    pub age: u32,
    pub muts: u32,
}

impl Tally {
    /// The running-total part of a creature's record: kills, offspring,
    /// survival events and mutations at birth. Territory and age are levels
    /// and are filled by the caller.
    pub fn totals_of(c: &Creature) -> Self {
        Self { kills: c.kills, terr: 0, young: c.offspring, surv: c.survival_events(), age: 0, muts: crate::cast!(c.mutations.len() => u32) }
    }

    /// Totals add, territory adds, age keeps the oldest.
    pub fn add(&mut self, o: Self) {
        self.kills += o.kills;
        self.terr += o.terr;
        self.young += o.young;
        self.surv += o.surv;
        self.age = self.age.max(o.age);
        self.muts += o.muts;
    }

    /// `self` plus `o` (see `add`).
    #[must_use]
    pub fn plus(mut self, o: Self) -> Self {
        self.add(o);
        self
    }
}

/// One closed year: the line's totals as of the year's last day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct YearRow {
    pub year: u32,
    pub stats: Tally,
}

/// A bloodline: its founder, its members and what the dead ones left behind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dynasty {
    pub root: CreatureId,
    pub species: SpeciesId,
    /// `Creature::label` at record time ("Ash w#003"); the node is pruned eventually.
    pub founder: String,
    /// Negative for founders (`Creature::born_day`).
    pub founded_day: i32,
    pub root_generation: u32,
    pub max_generation: u32,
    pub members_ever: u32,
    pub members_living: u32,
    /// Folded at death: kills, young, surv and muts of the dead members
    /// (terr and age stay 0).
    pub dead: Tally,
    /// Day the last living member died; the line never regrows after this.
    pub died_out_day: Option<u32>,
    /// Closed years, oldest first, at most `YEARS_KEPT`.
    pub years: Vec<YearRow>,
}

impl Dynasty {
    fn founded_by(c: &Creature, roster: &Roster) -> Self {
        Self {
            root: c.id,
            species: c.species,
            founder: c.label(roster),
            founded_day: c.born_day,
            root_generation: c.generation,
            max_generation: c.generation,
            members_ever: 0,
            members_living: 0,
            dead: Tally::default(),
            died_out_day: None,
            years: Vec::new(),
        }
    }

    /// Generations from the founder to the newest member.
    pub const fn generations(&self) -> u32 {
        self.max_generation.saturating_sub(self.root_generation) + 1
    }
}

/// Every predator dynasty, by root id.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dynasties {
    lines: BTreeMap<CreatureId, Dynasty>,
}

impl Dynasties {
    pub fn get(&self, root: CreatureId) -> Option<&Dynasty> {
        self.lines.get(&root)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Dynasty> {
        self.lines.values()
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// A predator was recorded: found the line when `root == c.id`, else join it.
    pub fn record_birth(&mut self, root: CreatureId, c: &Creature, roster: &Roster) {
        let line = self.lines.entry(root).or_insert_with(|| Dynasty::founded_by(c, roster));
        line.members_ever += 1;
        line.members_living += 1;
        line.max_generation = line.max_generation.max(c.generation);
    }

    /// A member died: its running totals move into `dead`.
    pub fn record_death(&mut self, root: CreatureId, c: &Creature, day: u32) {
        let Some(line) = self.lines.get_mut(&root) else { return };
        line.dead.add(Tally::totals_of(c));
        line.members_living = line.members_living.saturating_sub(1);
        if line.members_living == 0 {
            line.died_out_day = Some(day);
        }
    }

    /// Close `year` on `day`: every line pushes `dead + living` as a row
    /// (living totals per root from `living_totals`), the oldest rows beyond
    /// `YEARS_KEPT` go, and lines dead for `EXTINCT_KEEP_YEARS` are dropped.
    pub fn close_year(&mut self, year: u32, day: u32, year_days: u32, living: &BTreeMap<CreatureId, Tally>) {
        for (root, line) in &mut self.lines {
            let live = living.get(root).copied().unwrap_or_default();
            line.years.push(YearRow { year, stats: line.dead.plus(live) });
            if line.years.len() > YEARS_KEPT {
                let extra = line.years.len() - YEARS_KEPT;
                line.years.drain(..extra);
            }
        }
        let keep_days = EXTINCT_KEEP_YEARS.saturating_mul(year_days);
        self.lines.retain(|_, d| d.died_out_day.is_none_or(|dd| day.saturating_sub(dd) <= keep_days));
    }
}
