//! Synthetic event log for the prototype viewer.

use crate::sim::rng::Rng;
use crate::sim::species::SpeciesId;
use crate::sim::{Event, EventKind, TRAIT_NAMES};

use super::creatures::Creature;

pub fn generate(creatures: &[Creature]) -> Vec<Event> {
    let mut rng = Rng::new(0xE7E7);
    let mut out = Vec::new();
    let alive: Vec<&Creature> = creatures.iter().filter(|c| c.alive).collect();
    let mut year = 11;
    let mut day = 340;
    let mut hour = 3;
    let push = |out: &mut Vec<Event>, year, day, hour, kind, species, text: String, pos, detail: String| {
        out.push(Event { year, day, hour, kind, species, subject: None, text, pos, detail });
    };
    push(&mut out, 11, 331, 6, EventKind::Season, None, "Winter settles over the valley; vegetation regrowth halves".into(), None, "Seasonal modifier: vegetation regrowth ×0.5, water evaporation ×0.6, metabolism cost ×1.3 for all species.".into());
    push(&mut out, 11, 338, 12, EventKind::Extinction, Some(SpeciesId::Lynx), "The Lynx line of Sunfall Coast is extinct (last: Gloam l#088)".into(), Some((128, 7)), "Local population fell below 2 for 20 consecutive days. Lynx survive elsewhere: 5 remain in the Long Meadow and Fenlands.".into());
    for _ in 0..70 {
        hour = (hour + 1 + rng.below(7) as u32) % 24;
        if rng.chance(0.30) {
            day += 1;
            if day > 360 {
                day = 1;
                year += 1;
            }
        }
        let c = *rng.pick(&alive);
        let r = rng.f32();
        let (kind, text, detail) = if r < 0.30 {
            (
                EventKind::Birth,
                format!("{} {} gave birth to {} young in {}", c.name, c.tag(), 1 + rng.below(4), region_at(c)),
                format!("Mother fertility {:.2}; litter inherits mean of parents ±0.05 per trait.", c.genome.fertility()),
            )
        } else if r < 0.45 {
            (
                EventKind::DeathPredation,
                format!("{} {} was killed by a {} near {}", c.name, c.tag(), rng.pick(&["fox", "wolf", "lynx"]), region_at(c)),
                format!("Chase lasted {} ticks over {} cells. Victim speed {:.2} vs hunter speed {:.2}.", 6 + rng.below(30), 3 + rng.below(12), c.genome.speed(), 0.6 + rng.f32() * 0.35),
            )
        } else if r < 0.55 {
            (
                EventKind::DeathStarved,
                format!("{} {} starved in {}", c.name, c.tag(), region_at(c)),
                format!("Hunger reached 1.00 after {} days below the forage threshold. Local vegetation {:.2}.", 4 + rng.below(9), rng.f32() * 0.3),
            )
        } else if r < 0.62 {
            (
                EventKind::DeathAge,
                format!("{} {} died of old age at {} days", c.name, c.tag(), c.max_age_days),
                format!("Longevity {:.2}; produced {} offspring across {} seasons.", c.genome.longevity(), c.offspring, 1 + c.max_age_days / 90),
            )
        } else if r < 0.78 {
            let t = rng.below(8);
            let delta = rng.gauss(0.0, 0.06);
            (
                EventKind::Mutation,
                format!("{} {} born with {} {:+.2}", c.name, c.tag(), TRAIT_NAMES[t], delta),
                format!("Mutation rate 0.04/trait/birth. Parent mean {:.2} → offspring {:.2}.", c.genome.0[t], (c.genome.0[t] + delta).clamp(0.0, 1.0)),
            )
        } else if r < 0.90 {
            (
                EventKind::Migration,
                format!("{} {} {} migrated from {} toward {}", c.species.name(), rng.pick(&["herd", "pack", "family", "pair"]), c.tag(), region_at(c), rng.pick(&["Lakeshore", "Northmarch", "Fenlands", "The Long Meadow", "Reedwater Vale"])),
                "Trigger: local vegetation below 0.25 for 6 days, or predator pressure above 0.7.".into(),
            )
        } else if r < 0.95 {
            (
                EventKind::Drought,
                "Drought deepens in Ashen Ridge; 3 water cells dried up".to_string(),
                "Moisture in region fell to 0.18. Water cells revert to sand while moisture < 0.2.".into(),
            )
        } else {
            (
                EventKind::Note,
                format!("{} {} discovered a new den site in {}", c.name, c.tag(), region_at(c)),
                "Dens shelter juveniles from predation (−60% detection) and cold.".into(),
            )
        };
        push(&mut out, year, day, hour, kind, Some(c.species), text, Some((c.x, c.y)), detail);
    }
    // Random events must not run past "now" (Year 12, Day 3).
    for e in out.iter_mut() {
        if e.year == 12 && e.day > 3 {
            e.day = 3;
        }
    }
    out.sort_by_key(|e| (e.year, e.day, e.hour));
    // Final headline events at "now".
    push(&mut out, 12, 4, 13, EventKind::DeathPredation, Some(SpeciesId::Deer), "Thistle d#133 was killed by Ashfang w#042 in the Fenlands".into(), Some((80, 33)), "Chase lasted 41 ticks over 17 cells. Ashfang's 61st kill.".into());
    push(&mut out, 12, 4, 14, EventKind::Note, Some(SpeciesId::Wolf), "Ashfang w#042 is stalking Bramble h#217".into(), None, "Distance 11 cells; Bramble has not yet detected the wolf (camouflage 0.19 vs sense 0.71).".into());
    out
}

fn region_at(c: &Creature) -> &'static str {
    super::get_region(c.x, c.y)
}
