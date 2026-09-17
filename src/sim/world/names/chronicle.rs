//! The generation log and the world summary, derived from a finished world
//! (never saved): a handful of one-line sentences that name the places the
//! history struck and the rivers that shaped the map, and the numbers the
//! S09 summary shows.

use super::lexicon::capitalise;
use super::{Feature, FeatureKind};
use crate::sim::world::{AgeRegime, HistoryEvent, HistoryKind, Terrain, World};

/// The chronicle never has more lines than this (one for the wind, up to
/// three events, a river, a delta, a lake), so it fits the S09 layout.
pub const CHRONICLE_LINES: usize = 7;
/// No line is longer than this: every name is bounded, so the templates
/// stay inside the preview panel.
pub const CHRONICLE_WIDTH: usize = 85;
/// Marsh cells this close to a river's mouth make it a delta line.
const DELTA_REACH: (i32, i32) = (4, 2);
const DELTA_MIN_MARSH: usize = 3;

/// The world's story in `CHRONICLE_LINES` lines at most.
pub fn chronicle(world: &World, age: u8) -> Vec<String> {
    let mut lines = vec![wind_line(world, age)];
    lines.extend(world.history.iter().take(3).map(|e| event_line(world, e)));
    let trunk = world.features_of_kind(FeatureKind::River).next();
    if let Some(r) = trunk {
        lines.push(river_line(world, r));
        if let Some(m) = marsh_line(world, r) {
            lines.push(m);
        }
    }
    if let Some(l) = world.features_of_kind(FeatureKind::Lake).next() {
        lines.push(format!("{} fills a hollow in the {}.", l.name, world.region_name(l.anchor.0, l.anchor.1)));
    }
    lines.truncate(CHRONICLE_LINES);
    lines
}

fn wind_line(world: &World, age: u8) -> String {
    let wind = capitalise(world.wind.name());
    if age == 0 {
        format!("{wind} winds sweep an unweathered land.")
    } else {
        let epochs = if age == 1 { "one epoch".to_string() } else { format!("{age} epochs") };
        format!("{wind} winds shape a {} land over {epochs}.", AgeRegime::for_age(age).name)
    }
}

fn event_line(world: &World, e: &HistoryEvent) -> String {
    let place = event_place(world, e);
    match e.kind {
        HistoryKind::FaultScarp => format!("A fault throws up a scarp across {place}."),
        HistoryKind::VolcanicDome => format!("Fire raises {place}."),
        HistoryKind::Glaciation => format!("Ice scours {place}."),
    }
}

/// The range (for ice, also the lake) with the most cells in the event's
/// window, else the region under its centre.
fn event_place(world: &World, e: &HistoryEvent) -> String {
    let (w, h) = (crate::cast!(world.width => i32), crate::cast!(world.height => i32));
    let reach = match e.kind {
        HistoryKind::Glaciation => (w.div_euclid(2), h.div_euclid(4)),
        HistoryKind::FaultScarp | HistoryKind::VolcanicDome => (crate::cast!(e.extent => i32), crate::cast!(e.extent.div_euclid(2) + 1 => i32)),
    };
    let (cx, cy) = (crate::cast!(e.x => i32), crate::cast!(e.y => i32));
    let mut tally = vec![0usize; world.names.features.len()];
    if world.names.map.len() != world.cells.len() {
        return format!("the {}", world.region_name(e.x, e.y));
    }
    for y in (cy - reach.1).max(0)..=(cy + reach.1).min(h - 1) {
        for x in (cx - reach.0).max(0)..=(cx + reach.0).min(w - 1) {
            let k = world.names.map[crate::cast!(y => usize) * world.width + crate::cast!(x => usize)];
            let Some(f) = world.names.features.get(usize::from(k)) else { continue };
            if f.kind == FeatureKind::Range || (e.kind == HistoryKind::Glaciation && f.kind == FeatureKind::Lake) {
                tally[usize::from(k)] += 1;
            }
        }
    }
    let best = (0..tally.len()).filter(|&k| tally[k] > 0).max_by_key(|&k| (tally[k], std::cmp::Reverse(k)));
    best.and_then(|k| world.names.features.get(k)).map_or_else(|| format!("the {}", world.region_name(e.x, e.y)), |f| f.name.clone())
}

fn river_line(world: &World, r: &Feature) -> String {
    let from = world.region_name(r.source.0, r.source.1);
    match destination(world, r) {
        Some(to) => format!("{} runs from the {from} to {to}.", capitalise(&r.name)),
        None => format!("{} winds through the {from}.", capitalise(&r.name)),
    }
}

/// Where a river ends: the sea or lake it drains into, or the edge of the
/// world. `None` when it sinks into a hollow too small to name.
fn destination(world: &World, r: &Feature) -> Option<String> {
    let (mx, my) = r.mouth;
    match world.feature_at(mx, my) {
        Some(f) if f.kind == FeatureKind::Ocean || f.kind == FeatureKind::Lake => Some(f.name.clone()),
        _ if mx == 0 || my == 0 || mx + 1 == world.width || my + 1 == world.height => Some("beyond the world's edge".to_string()),
        _ => None,
    }
}

fn marsh_line(world: &World, r: &Feature) -> Option<String> {
    let (w, h) = (crate::cast!(world.width => i32), crate::cast!(world.height => i32));
    let (cx, cy) = (crate::cast!(r.mouth.0 => i32), crate::cast!(r.mouth.1 => i32));
    let mut marsh = 0;
    for y in (cy - DELTA_REACH.1).max(0)..=(cy + DELTA_REACH.1).min(h - 1) {
        for x in (cx - DELTA_REACH.0).max(0)..=(cx + DELTA_REACH.0).min(w - 1) {
            if world.cell(crate::cast!(x => usize), crate::cast!(y => usize)).terrain == Terrain::Marsh {
                marsh += 1;
            }
        }
    }
    if marsh < DELTA_MIN_MARSH {
        return None;
    }
    Some(match destination(world, r) {
        Some(to) => format!("Marshes form where {} meets {to}.", r.name),
        None => format!("Marshes form at the mouth of {}.", r.name),
    })
}

/// The numbers the S09 summary shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    /// Land cells 8-adjacent to the ocean.
    pub coast: usize,
    /// The longest named river and its channel length.
    pub longest_river: Option<(String, u32)>,
    /// The highest cell, its elevation, and the range or region it stands in.
    pub peak: ((usize, usize), f32, String),
    pub lakes: usize,
    pub rivers: usize,
    pub ranges: usize,
    pub has_ocean: bool,
}

pub fn summary(world: &World) -> Summary {
    let ocean = world.names.features.iter().position(|f| f.kind == FeatureKind::Ocean);
    let coast = ocean.map_or(0, |o| {
        let o = crate::cast!(o => u16);
        (0..world.cells.len())
            .filter(|&i| !world.cells[i].terrain.is_water() && touches(world, i, o))
            .count()
    });
    let longest_river = world
        .names
        .features
        .iter()
        .filter(|f| f.kind.is_river())
        .max_by_key(|f| f.cells)
        .map(|f| (f.name.clone(), f.cells));
    let top = (0..world.cells.len()).max_by(|&a, &b| world.cells[a].elevation.total_cmp(&world.cells[b].elevation).then(b.cmp(&a))).unwrap_or(0);
    let (px, py) = (top % world.width.max(1), top.div_euclid(world.width.max(1)));
    let stands = match world.feature_at(px, py) {
        Some(f) if f.kind == FeatureKind::Range => f.name.clone(),
        _ => world.region_name(px, py).to_string(),
    };
    Summary {
        coast,
        longest_river,
        peak: ((px, py), world.cells.get(top).map_or(0.0, |c| c.elevation), stands),
        lakes: world.features_of_kind(FeatureKind::Lake).count(),
        rivers: world.names.features.iter().filter(|f| f.kind.is_river()).count(),
        ranges: world.features_of_kind(FeatureKind::Range).count(),
        has_ocean: ocean.is_some(),
    }
}

/// Is any 8-neighbour of `i` mapped to feature `f`?
fn touches(world: &World, i: usize, f: u16) -> bool {
    if world.names.map.len() != world.cells.len() {
        return false;
    }
    let (x, y) = (crate::cast!(i % world.width => i32), crate::cast!(i.div_euclid(world.width) => i32));
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            let (nx, ny) = (x + dx, y + dy);
            if (dx != 0 || dy != 0) && world.in_bounds(nx, ny) && world.names.map[crate::cast!(ny => usize) * world.width + crate::cast!(nx => usize)] == f {
                return true;
            }
        }
    }
    false
}
