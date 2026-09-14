//! Core disease types: pathogen/stage/infection/outbreak records and the disease state.

use serde::{Deserialize, Serialize};
use crate::sim::creatures::CreatureId;
use crate::sim::params::{DiseaseParams, PathogenParams};
use crate::sim::species::SpeciesId;

/// Width of the per-creature immunity table: roster plus strains, at most 8.
pub const MAX_PATHOGENS: usize = 8;

/// Index into `DiseaseState::pathogens` (never into the params roster).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PathogenId(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Stage {
    Incubating,
    Infectious,
}

/// One contagious infection (a creature carries at most one at a time, FR3).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Infection {
    pub pathogen: PathogenId,
    pub stage: Stage,
    pub since_day: u32,
    /// Day of the next stage transition (resistance-adjusted at onset).
    pub ends_day: u32,
    /// `pathogen.severity × (1 − 0.5 × resistance)`; drives the effects.
    pub severity: f32,
    pub source: Option<CreatureId>,
    /// Absolute outbreak index (see `DiseaseState::outbreak`).
    pub outbreak: u16,
}

/// A live pathogen: the roster record plus strain bookkeeping (`FR8b`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pathogen {
    pub params: PathogenParams,
    /// The pathogen this strain mutated from (`None` for the roster).
    pub parent: Option<PathogenId>,
    pub born_day: Option<u32>,
    /// A strain whose last case is gone; its slot may be reused.
    pub extinct: bool,
}

impl Pathogen {
    pub fn name(&self) -> &str {
        &self.params.name
    }

    pub fn host(&self, s: SpeciesId) -> f32 {
        self.params.hosts.get(&s).copied().unwrap_or(0.0)
    }

    pub const fn is_strain(&self) -> bool {
        self.parent.is_some()
    }
}

/// One outbreak of one pathogen, from index case to burn-out (FR8).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Outbreak {
    pub pathogen: PathogenId,
    pub started_day: u32,
    pub ended_day: Option<u32>,
    pub origin_region: u8,
    pub index_case: CreatureId,
    /// Creatures that reached the infectious stage.
    pub cases: u32,
    pub deaths: u32,
    pub recovered: u32,
    pub peak_active: u32,
    pub peak_day: u32,
    pub species_cases: [u32; 6],
    pub species_deaths: [u32; 6],
    pub epidemic: bool,
    /// Per-species mean Resistance on the start day and at burn-out.
    pub resist_at_start: [f32; 6],
    pub resist_at_end: [f32; 6],
    /// Daily active count, refreshed by `daily_update`.
    pub active: u32,
    /// New infectious cases today (for the sidebar's `+N` line).
    pub cases_today: u32,
}

impl Outbreak {
    pub fn duration_days(&self, today: u32) -> u32 {
        self.ended_day.unwrap_or(today).saturating_sub(self.started_day)
    }
}

/// Per-pathogen running statistics, refreshed daily.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathogenStats {
    pub outbreaks: u32,
    pub total_cases: u32,
    pub total_deaths: u32,
    pub active: u32,
    pub active_by_species: [u32; 6],
    pub peak_active: u32,
    pub immune: u32,
}

/// Everything the disease system owns on the `Sim`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiseaseState {
    pub pathogens: Vec<Pathogen>,
    /// Day of the last case per pathogen slot (`u32::MAX` = never).
    pub last_case_day: [u32; MAX_PATHOGENS],
    pub stats: [PathogenStats; MAX_PATHOGENS],
    /// Outbreak history, bounded to `OUTBREAKS_MAX`; `first_index` keeps the
    /// absolute indices stored on infections stable when the oldest is dropped.
    pub outbreaks: Vec<Outbreak>,
    pub first_index: u16,
    /// Spillovers that found no free slot (diagnostics only).
    pub failed_spillovers: u32,
}

pub const OUTBREAKS_MAX: usize = 64;

impl DiseaseState {
    pub fn new(dp: &DiseaseParams) -> Self {
        let pathogens = dp
            .pathogens
            .iter()
            .take(dp.max_pathogens())
            .map(|p| Pathogen { params: p.clone(), parent: None, born_day: None, extinct: false })
            .collect();
        Self {
            pathogens,
            last_case_day: [u32::MAX; MAX_PATHOGENS],
            stats: [PathogenStats::default(); MAX_PATHOGENS],
            outbreaks: Vec::new(),
            first_index: 0,
            failed_spillovers: 0,
        }
    }

    pub fn pathogen(&self, id: PathogenId) -> Option<&Pathogen> {
        self.pathogens.get(crate::cast!(id.0 => usize))
    }

    /// The name of a pathogen slot, or `?` when the slot is empty.
    pub fn name(&self, id: PathogenId) -> &str {
        self.pathogen(id).map_or("?", Pathogen::name)
    }

    /// The roster ancestor of a strain (itself for roster pathogens).
    pub fn root(&self, id: PathogenId) -> PathogenId {
        let mut cur = id;
        for _ in 0..MAX_PATHOGENS {
            match self.pathogen(cur).and_then(|p| p.parent) {
                Some(p) => cur = p,
                None => return cur,
            }
        }
        cur
    }

    pub fn outbreak(&self, index: u16) -> Option<&Outbreak> {
        let i = crate::cast!(index.checked_sub(self.first_index)? => usize);
        self.outbreaks.get(i)
    }

    pub fn outbreak_mut(&mut self, index: u16) -> Option<&mut Outbreak> {
        let i = crate::cast!(index.checked_sub(self.first_index)? => usize);
        self.outbreaks.get_mut(i)
    }

    /// The most recent still-open outbreak of `id`, if any.
    pub fn open_outbreak(&self, id: PathogenId) -> Option<(u16, &Outbreak)> {
        self.outbreaks
            .iter()
            .enumerate()
            .rev()
            .find(|(_, o)| o.pathogen == id && o.ended_day.is_none())
            .map(|(i, o)| (self.first_index + crate::cast!(i => u16), o))
    }

    pub(super) fn push_outbreak(&mut self, o: Outbreak) -> u16 {
        if self.outbreaks.len() >= OUTBREAKS_MAX {
            self.outbreaks.remove(0);
            self.first_index += 1;
        }
        self.outbreaks.push(o);
        self.first_index + crate::cast!((self.outbreaks.len() - 1) => u16)
    }

    /// Living hosts of a pathogen (host multiplier > 0), all species.
    pub fn hosts_living(&self, id: PathogenId, population: &[u32; 6]) -> u32 {
        let Some(p) = self.pathogen(id) else { return 0 };
        SpeciesId::ALL.iter().filter(|s| p.host(**s) > 0.0).map(|s| population[s.index()]).sum()
    }
}
