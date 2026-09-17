//! The chronicle prompt (C9, downstream).
//!
//! Sends exactly: year, season, region names, the season's events as (day,
//! label, text) capped at `MAX_EVENTS` with notable kinds kept first, the
//! per-species season census, and the previous entry's text for continuity.
//! Never params, paths or config.

use std::fmt::Write as _;

use crate::ai::Message;
use crate::sim::chronicle::{self, SeasonCensus};
use crate::sim::{EventKind, Season, Sim};

/// Events per request; the ring holds 5 000, a busy season far more than this.
pub const MAX_EVENTS: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventLine {
    pub day: u32,
    pub label: &'static str,
    pub text: String,
}

/// Plain data the prompt is built from; extracted once, then no `&Sim`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChronicleInput {
    pub year: u32,
    pub season: Season,
    pub regions: Vec<String>,
    /// `MAX_EVENTS` at most, notable kinds first, oldest first within a group.
    pub events: Vec<EventLine>,
    /// How many events the season produced before capping.
    pub total_events: usize,
    pub census: Vec<SeasonCensus>,
    pub previous: Option<String>,
}

/// Extract the season's data from the sim.
pub fn from_sim(sim: &Sim, year: u32, season: Season, previous: Option<&str>) -> ChronicleInput {
    let all = chronicle::season_events(sim, year, season);
    let census = chronicle::season_census(sim, year, season, &all);
    let line = |e: &&crate::sim::Event| EventLine { day: e.day, label: e.kind.label(), text: e.text.clone() };
    let notable = |k: EventKind| !matches!(k, EventKind::Birth | EventKind::Season) && !k.is_death();
    let mut events: Vec<EventLine> = all.iter().filter(|e| notable(e.kind)).map(line).collect();
    events.extend(all.iter().filter(|e| e.kind.is_death()).map(line));
    events.extend(all.iter().filter(|e| e.kind == EventKind::Birth).map(line));
    events.truncate(MAX_EVENTS);
    ChronicleInput {
        year,
        season,
        regions: sim.world.regions.iter().map(|r| r.0.clone()).collect(),
        events,
        total_events: all.len(),
        census,
        previous: previous.map(str::to_string),
    }
}

/// The two messages sent for one season.
pub fn messages(input: &ChronicleInput) -> Vec<Message> {
    let system = "You are the chronicler of a wild valley in a nature simulation, writing its legends. \
Write ONE paragraph of three to six sentences for the season described, in the past tense, in the \
style of a terse historical chronicle. Use only the facts given: never invent species, places, \
names or numbers. Refer to places by the region names listed. Round numbers freely. Prefer the \
notable events (extinctions, epidemics, droughts, migrations) over routine births and deaths. \
Use plain ASCII punctuation only. Output the paragraph and nothing else.";
    let mut user = String::new();
    let _ = writeln!(user, "Year {}, {}.", input.year, input.season.name());
    let _ = writeln!(user, "Regions: {}.", input.regions.join(", "));
    let _ = writeln!(user, "Species (population at season start -> end, births, deaths):");
    for c in &input.census {
        let start = c.start.map_or_else(|| "?".to_string(), |s| s.to_string());
        let extinct = if c.extinct { ", EXTINCT" } else { "" };
        let _ = writeln!(user, "- {}: {} -> {}, {} births, {} deaths{extinct}", c.plural, start, c.end, c.births, c.deaths);
    }
    let _ = writeln!(user, "Events ({} of {} this season):", input.events.len(), input.total_events);
    for e in &input.events {
        let _ = writeln!(user, "- day {} [{}] {}", e.day, e.label, e.text);
    }
    if let Some(prev) = &input.previous {
        let _ = writeln!(user, "The previous entry, for continuity (do not repeat it):\n{prev}");
    }
    let _ = write!(user, "Write the chronicle entry for this season.");
    vec![Message::system(system), Message::user(user)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Params;

    fn one_season() -> Sim {
        let mut sim = Sim::new(7, Params::default());
        for _ in 0..(91 * 24) {
            sim.step();
        }
        sim
    }

    /// R9: the message holds the documented list and nothing else.
    #[test]
    fn chronicle_prompt_sends_exactly_the_documented_data() {
        let sim = one_season();
        let input = from_sim(&sim, 1, Season::Spring, Some("Before the thaw."));
        assert!(input.events.len() <= MAX_EVENTS);
        assert!(input.total_events >= input.events.len());
        let msgs = messages(&input);
        assert_eq!(msgs.len(), 2);
        let user = &msgs[1].content;
        assert!(user.starts_with("Year 1, Spring."), "{user}");
        for r in &sim.world.regions {
            assert!(user.contains(&r.0), "region {} missing", r.0);
        }
        for id in sim.roster().ids() {
            assert!(user.contains(sim.roster().plural(id)));
        }
        assert!(user.contains("Before the thaw."));
        let first = input.events.first().unwrap();
        assert!(user.contains(&first.text));
        // Never params, paths or config.
        for forbidden in ["base_url", "hunger_base", "[species]", "ui.toml", "/Users", "width ="] {
            assert!(!user.contains(forbidden), "{forbidden} leaked into the prompt");
        }
    }

    #[test]
    fn notable_events_come_before_routine_ones() {
        let sim = one_season();
        let input = from_sim(&sim, 1, Season::Spring, None);
        let first_routine = input.events.iter().position(|e| e.label == "birth" || EventKind::DeathPredation.label() == e.label);
        let last_notable = input.events.iter().rposition(|e| e.label != "birth" && !e.label.contains("starved") && e.label != "predation" && e.label != "old age" && e.label != "thirst" && e.label != "disease");
        if let (Some(r), Some(n)) = (first_routine, last_notable) {
            assert!(n < r, "notable at {n} after routine at {r}");
        }
    }
}
