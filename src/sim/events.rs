//! Event log entries and a fixed-capacity ring buffer.

use serde::{Deserialize, Serialize};

use crate::sim::creatures::CreatureId;
use crate::sim::species::SpeciesId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    Birth,
    DeathStarved,
    DeathThirst,
    DeathPredation,
    DeathAge,
    /// C7: died of a pathogen or parasite load.
    DeathDisease,
    Mutation,
    Migration,
    Extinction,
    Drought,
    DroughtEased,
    Season,
    Note,
    // ---- C7 disease
    Outbreak,
    Spillover,
    Epidemic,
    EpidemicOver,
    Recovery,
    /// C5 `FR5b`: prey avoidance of predators that are not currently hunting.
    /// Aggregated to at most one event per region per day.
    Wary,
}

impl EventKind {
    pub const fn is_death(self) -> bool {
        matches!(
            self,
            Self::DeathStarved | Self::DeathThirst | Self::DeathPredation | Self::DeathAge | Self::DeathDisease
        )
    }

    /// Text label, kept in `sim` (no presentation deps) for headless output.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Birth => "birth",
            Self::DeathStarved => "starved",
            Self::DeathThirst => "thirst",
            Self::DeathPredation => "predation",
            Self::DeathAge => "old age",
            Self::DeathDisease => "disease",
            Self::Mutation => "mutation",
            Self::Migration => "migration",
            Self::Extinction => "EXTINCTION",
            Self::Drought => "drought",
            Self::DroughtEased => "eases",
            Self::Season => "season",
            Self::Note => "note",
            Self::Outbreak => "outbreak",
            Self::Spillover => "spillover",
            Self::Epidemic => "EPIDEMIC",
            Self::EpidemicOver => "burnt out",
            Self::Recovery => "recovery",
            Self::Wary => "wary",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub year: u32,
    pub day: u32,
    pub hour: u32,
    pub kind: EventKind,
    pub species: Option<SpeciesId>,
    /// The creature this event is about (deaths, den notes, etc.).
    pub subject: Option<CreatureId>,
    pub text: String,
    pub pos: Option<(usize, usize)>,
    pub detail: String,
}

/// Fixed-capacity ring buffer of events (Vec-backed, so iteration order is stable).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRing {
    cap: usize,
    buf: Vec<Event>,
    /// Absolute count of events ever pushed (1-based index of the last event;
    /// used as the extinction alert's `event_index`).
    total: u64,
}

impl EventRing {
    pub const fn new(cap: usize) -> Self {
        Self { cap, buf: Vec::new(), total: 0 }
    }

    pub fn push(&mut self, e: Event) {
        if self.cap == 0 {
            return;
        }
        self.buf.push(e);
        self.total += 1;
        let excess = self.buf.len().saturating_sub(self.cap);
        if excess > 0 {
            self.buf.drain(0..excess);
        }
    }

    /// Absolute (1-based) index of the most recently pushed event.
    pub const fn total(&self) -> u64 {
        self.total
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Event> {
        self.buf.iter()
    }

    pub fn last(&self) -> Option<&Event> {
        self.buf.last()
    }

    /// The last `n` events, oldest first.
    pub fn tail(&self, n: usize) -> Vec<&Event> {
        let start = self.buf.len().saturating_sub(n);
        self.buf.iter().skip(start).collect()
    }
}
