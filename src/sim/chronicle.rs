//! The chronicle (C9): one paragraph per season, kept on `Sim` as a decorative
//! table the step never reads (ai-requirements R3, R11).
//!
//! This file is pure: it slices the event ring by season, builds a per-species
//! season census and writes the deterministic *template* entry that is the
//! fallback (and the seed) for the model-written one. Nothing here knows a
//! model exists.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::sim::{Event, EventKind, Season, Sim};

/// Who wrote the entry's text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    Template,
    Model,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChronicleEntry {
    pub year: u32,
    pub season: Season,
    pub text: String,
    pub source: Source,
}

/// Per-species figures for one season.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeasonCensus {
    pub name: String,
    pub plural: String,
    /// Population on the season's first day (`None` when no sample covers it).
    pub start: Option<u32>,
    /// Population now (the season just ended).
    pub end: u32,
    pub births: u32,
    pub deaths: u32,
    pub extinct: bool,
}

/// Which season a day of the year (1-based) falls in.
pub fn season_of_day(day_of_year: u32, season_days: u32) -> Season {
    let idx = day_of_year.saturating_sub(1).checked_div(season_days.max(1)).unwrap_or(0);
    Season::ALL[crate::cast!(idx.min(3) => usize)]
}

/// The season's slot in the year, 0..4.
const fn season_index(season: Season) -> u32 {
    match season {
        Season::Spring => 0,
        Season::Summer => 1,
        Season::Autumn => 2,
        Season::Winter => 3,
    }
}

/// Every event of `(year, season)` still in the ring, oldest first.
pub fn season_events(sim: &Sim, year: u32, season: Season) -> Vec<&Event> {
    let sd = sim.params.time.season_days;
    sim.events.iter().filter(|e| e.year == year && season_of_day(e.day, sd) == season).collect()
}

/// The per-species census for `(year, season)` from the daily series and the
/// season's events.
pub fn season_census(sim: &Sim, year: u32, season: Season, events: &[&Event]) -> Vec<SeasonCensus> {
    let sd = sim.params.time.season_days;
    let start_day = (year.saturating_sub(1) * 4 + season_index(season)) * sd;
    let start_sample = sim.series.samples().iter().find(|s| s.day >= start_day);
    let roster = sim.roster();
    roster
        .ids()
        .map(|id| {
            let i = id.index();
            let births = events.iter().filter(|e| e.kind == EventKind::Birth && e.species == Some(id)).count();
            let deaths = events.iter().filter(|e| e.kind.is_death() && e.species == Some(id)).count();
            SeasonCensus {
                name: roster.name(id).to_string(),
                plural: roster.plural(id).to_string(),
                start: start_sample.and_then(|s| s.population.get(i).copied()),
                end: sim.species.get(i).map_or(0, |s| s.count),
                births: crate::cast!(births => u32),
                deaths: crate::cast!(deaths => u32),
                extinct: sim.extinct.get(i).copied().unwrap_or(false),
            }
        })
        .collect()
}

/// The deterministic fallback entry: counts and the season's notable events.
pub fn template_entry(sim: &Sim, year: u32, season: Season) -> ChronicleEntry {
    let events = season_events(sim, year, season);
    let census = season_census(sim, year, season, &events);
    let mut text = String::new();
    let counts: Vec<String> = census
        .iter()
        .map(|c| match c.start {
            Some(s) => format!("{} {}{}{}", c.plural, s, crate::glyphs::RIGHT, c.end),
            None => format!("{} {}", c.plural, c.end),
        })
        .collect();
    let _ = writeln!(text, "{}.", counts.join(", "));
    let births: u32 = census.iter().map(|c| c.births).sum();
    let deaths: u32 = census.iter().map(|c| c.deaths).sum();
    let _ = write!(text, "{births} births, {deaths} deaths");
    let by_cause = death_causes(&events);
    if !by_cause.is_empty() {
        let _ = write!(text, " ({by_cause})");
    }
    text.push('.');
    for e in notable(&events) {
        let _ = write!(text, "\n{}.", e.text.trim_end_matches('.'));
    }
    ChronicleEntry { year, season, text, source: Source::Template }
}

/// `predation 12, starved 4, ...` in a fixed cause order.
fn death_causes(events: &[&Event]) -> String {
    let causes = [EventKind::DeathPredation, EventKind::DeathStarved, EventKind::DeathThirst, EventKind::DeathDisease, EventKind::DeathAge];
    causes
        .iter()
        .filter_map(|k| {
            let n = events.iter().filter(|e| e.kind == *k).count();
            (n > 0).then(|| format!("{} {n}", k.label()))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The events worth a sentence of their own, oldest first, at most eight.
///
/// Extinctions (global and the local-extinction notes), epidemics, droughts
/// and migrations. Routine notes (regrowth sites, soft-cap crossings) are not.
pub fn notable<'a>(events: &[&'a Event]) -> Vec<&'a Event> {
    events
        .iter()
        .copied()
        .filter(|e| {
            matches!(
                e.kind,
                EventKind::Extinction
                    | EventKind::Epidemic
                    | EventKind::EpidemicOver
                    | EventKind::Outbreak
                    | EventKind::Spillover
                    | EventKind::Drought
                    | EventKind::DroughtEased
                    | EventKind::Migration
            ) || (e.kind == EventKind::Note && e.text.contains("extinct"))
        })
        .take(8)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Params;

    #[test]
    fn season_of_day_covers_the_year() {
        assert_eq!(season_of_day(1, 90), Season::Spring);
        assert_eq!(season_of_day(90, 90), Season::Spring);
        assert_eq!(season_of_day(91, 90), Season::Summer);
        assert_eq!(season_of_day(360, 90), Season::Winter);
        assert_eq!(season_of_day(999, 90), Season::Winter, "clamped");
    }

    #[test]
    fn template_entry_is_deterministic_and_counts_the_season() {
        let mut a = Sim::new(7, Params::default());
        let mut b = Sim::new(7, Params::default());
        for _ in 0..(91 * 24) {
            a.step();
            b.step();
        }
        assert_eq!(a.time.season(), Season::Summer);
        let ea = template_entry(&a, 1, Season::Spring);
        let eb = template_entry(&b, 1, Season::Spring);
        assert_eq!(ea, eb);
        assert_eq!(ea.source, Source::Template);
        assert!(ea.text.starts_with("Voles "), "{}", ea.text);
        assert!(ea.text.contains("births"), "{}", ea.text);
        let events = season_events(&a, 1, Season::Spring);
        assert!(events.iter().all(|e| e.year == 1 && e.day <= 90));
        assert!(season_events(&a, 1, Season::Autumn).is_empty());
        let census = season_census(&a, 1, Season::Spring, &events);
        assert_eq!(census.len(), a.roster().len());
        assert!(census.iter().any(|c| c.births > 0 || c.deaths > 0));
    }
}
