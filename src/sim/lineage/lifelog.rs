//! The life log (S15 Traits & Fates): the deaths of the last `LIFE_WINDOW_DAYS` days.
//!
//! Each record holds what the creature slot forgets once the carcass is freed:
//! genome, age, cause and the outcome counters. Written by `behavior::kill`,
//! saved with the lineage (save format 15), and read together with the living
//! creatures by `stats::outcomes`.
//!
//! Passive: it draws no RNG and emits no events, so the checksum is unchanged.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::sim::creatures::{Cause, Creature};
use crate::sim::species::{Genome, SpeciesId};

/// How far back the log reaches, in days (the S15 window).
pub const LIFE_WINDOW_DAYS: u32 = 240;
/// Hard cap on kept records; the oldest go first.
pub const LIFE_LOG_MAX: usize = 50_000;

/// One finished life.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LifeRecord {
    pub species: SpeciesId,
    pub genome: Genome,
    /// Negative for founders.
    pub born_day: i32,
    pub died_day: u32,
    pub cause: Cause,
    pub offspring: u32,
    pub kills: u32,
    pub attempts: u32,
    pub chased: u32,
    pub escaped: u32,
}

impl LifeRecord {
    /// Snapshot a creature at the moment it dies.
    pub const fn from_creature(c: &Creature, cause: Cause, died_day: u32) -> Self {
        Self {
            species: c.species,
            genome: c.genome,
            born_day: c.born_day,
            died_day,
            cause,
            offspring: c.offspring,
            kills: c.kills,
            attempts: c.attempts,
            chased: c.chased,
            escaped: c.escaped,
        }
    }

    /// Age at death in days.
    pub fn age_days(&self) -> u32 {
        crate::cast!((i64::from(self.died_day) - i64::from(self.born_day)).max(0) => u32)
    }
}

/// Deaths of the last `LIFE_WINDOW_DAYS` days, oldest first.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LifeLog {
    records: VecDeque<LifeRecord>,
}

impl LifeLog {
    /// Append a death (deaths arrive in day order) and drop what fell out of
    /// the window or over the cap.
    pub fn push(&mut self, rec: LifeRecord) {
        let today = rec.died_day;
        self.records.push_back(rec);
        self.trim(today);
    }

    /// Drop records older than the window as of `today`, then enforce the cap.
    pub fn trim(&mut self, today: u32) {
        while self.records.front().is_some_and(|r| r.died_day.saturating_add(LIFE_WINDOW_DAYS) < today) {
            self.records.pop_front();
        }
        while self.records.len() > LIFE_LOG_MAX {
            self.records.pop_front();
        }
    }

    pub fn records(&self) -> impl Iterator<Item = &LifeRecord> {
        self.records.iter()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[cfg(test)]
mod tests;
