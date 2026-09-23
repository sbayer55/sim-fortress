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

use crate::sim::creatures::{Creature, CreatureId, Mutation, Sex};
use crate::sim::params::Roster;
use crate::sim::species::{Kind, SpeciesId};
use crate::sim::Sim;

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
    /// The player's name for the line (`Sim::rename_dynasty`); `None` shows
    /// `<Founder> line`. Display only (save 20).
    pub name: Option<String>,
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
            name: None,
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

    /// Set or clear the player's name for the line rooted at `root`; false
    /// when no such line is kept.
    pub fn rename(&mut self, root: CreatureId, name: Option<String>) -> bool {
        let Some(line) = self.lines.get_mut(&root) else { return false };
        line.name = name;
        true
    }

    /// Re-label the founder after the player renamed it (`Name tag`).
    pub fn relabel_founder(&mut self, root: CreatureId, label: String) {
        if let Some(line) = self.lines.get_mut(&root) {
            line.founder = label;
        }
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

/// Cells held per creature for every predator species: one pass per block.
fn held_cells(sim: &Sim) -> BTreeMap<CreatureId, u32> {
    let hold_min = sim.params.territory.hold_min;
    sim.roster().predator_ids().flat_map(|sp| sim.world.held_cells_by_holder(sp, hold_min)).collect()
}

/// A living predator's six stats plus its territory cells and age, and the
/// root of its line.
fn member_tally(sim: &Sim, held: &BTreeMap<CreatureId, u32>, c: &Creature) -> Option<(CreatureId, Tally)> {
    if sim.roster().kind(c.species) != Kind::Predator {
        return None;
    }
    let root = sim.lineage.get(c.id).map(|n| n.root)?;
    let mut t = Tally::totals_of(c);
    t.terr = held.get(&c.id).copied().unwrap_or(0);
    t.age = c.age_days(sim.time.day_index());
    Some((root, t))
}

/// Per root, the six stats of its living members: one pass over the living
/// plus one scent pass per predator species. No RNG, no events.
pub fn living_totals(sim: &Sim) -> BTreeMap<CreatureId, Tally> {
    let held = held_cells(sim);
    let mut out: BTreeMap<CreatureId, Tally> = BTreeMap::new();
    for c in sim.creatures.living() {
        if let Some((root, t)) = member_tally(sim, &held, c) {
            out.entry(root).or_default().add(t);
        }
    }
    out
}

/// The five survival counters of one animal, for the sidebar's Survival lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Survival {
    pub infections: u8,
    pub escapes: u32,
    pub contests_won: u16,
    pub droughts: u16,
    pub winters: u16,
}

/// A living member of a dynasty, as the screen shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct DynastyMember {
    pub id: CreatureId,
    /// `Name tag`.
    pub label: String,
    pub sex: Sex,
    pub generation: u32,
    /// Where it stands now.
    pub region: u8,
    /// Its own six stats: `age` in days, `terr` in cells.
    pub stats: Tally,
    pub contests: (u16, u16),
    pub survival: Survival,
    pub mutations: Vec<Mutation>,
    /// Kills per prey species (roster order), for the sidebar's Kills by prey.
    pub kills_by_species: Vec<u32>,
}

/// A dynasty with its living members, as the screen ranks it.
#[derive(Clone, Debug, PartialEq)]
pub struct DynastyView {
    pub root: CreatureId,
    pub species: SpeciesId,
    pub founder: String,
    /// The player's name for the line, if any.
    pub name: Option<String>,
    pub founded_day: i32,
    pub generations: u32,
    pub members_ever: u32,
    pub members_living: u32,
    /// The region holding the plurality of living members (lowest index on
    /// ties); `None` when nobody is alive.
    pub region: Option<u8>,
    /// Dead plus living, as of now.
    pub totals: Tally,
    /// Closed years, oldest first.
    pub years: Vec<YearRow>,
    /// Living members, kills descending then id.
    pub members: Vec<DynastyMember>,
}

fn member_view(sim: &Sim, c: &Creature, stats: Tally) -> DynastyMember {
    DynastyMember {
        id: c.id,
        label: c.label(sim.roster()),
        sex: c.sex,
        generation: c.generation,
        region: crate::cast!(sim.world.region_index(c.x, c.y).min(7) => u8),
        stats,
        contests: (c.contests_won, c.contests_lost),
        survival: Survival { infections: c.infections_survived, escapes: c.escaped, contests_won: c.contests_won, droughts: c.droughts_survived, winters: c.winters_survived },
        mutations: c.mutations.clone(),
        kills_by_species: c.kills_by_species.clone(),
    }
}

/// The region most of `members` stand in, lowest index on ties.
fn plurality_region(members: &[DynastyMember]) -> Option<u8> {
    let mut counts = [0u32; 8];
    for m in members {
        if let Some(n) = counts.get_mut(usize::from(m.region)) {
            *n += 1;
        }
    }
    let best = counts.iter().copied().max().filter(|&n| n > 0)?;
    counts.iter().position(|&n| n == best).map(|i| crate::cast!(i => u8))
}

/// Every dynasty with its living members, ranked for the screen.
///
/// Kills descending, then young, then the oldest founding, then root id. One
/// pass over the living plus one scent pass per predator species; the screen
/// caches it per day.
pub fn rank(sim: &Sim) -> Vec<DynastyView> {
    let held = held_cells(sim);
    let mut members: BTreeMap<CreatureId, Vec<DynastyMember>> = BTreeMap::new();
    for c in sim.creatures.living() {
        if let Some((root, t)) = member_tally(sim, &held, c) {
            members.entry(root).or_default().push(member_view(sim, c, t));
        }
    }
    let mut out: Vec<DynastyView> = sim
        .lineage
        .dynasties()
        .iter()
        .map(|d| {
            let mut ms = members.remove(&d.root).unwrap_or_default();
            ms.sort_by(|a, b| b.stats.kills.cmp(&a.stats.kills).then(a.id.cmp(&b.id)));
            let totals = ms.iter().fold(d.dead, |acc, m| acc.plus(m.stats));
            DynastyView {
                root: d.root,
                species: d.species,
                founder: d.founder.clone(),
                name: d.name.clone(),
                founded_day: d.founded_day,
                generations: d.generations(),
                members_ever: d.members_ever,
                members_living: crate::cast!(ms.len() => u32),
                region: plurality_region(&ms),
                totals,
                years: d.years.clone(),
                members: ms,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        b.totals.kills.cmp(&a.totals.kills).then(b.totals.young.cmp(&a.totals.young)).then(a.founded_day.cmp(&b.founded_day)).then(a.root.cmp(&b.root))
    });
    out
}
