//! Vital needs and the actions that satisfy them (graze, drink, den, rest).

use crate::sim::creatures::{
    Creature, Goal,
};
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::params::{CreaturesParams, Roster};
use crate::sim::rng::Rng;
use crate::sim::disease::{self};
use crate::sim::params::DiseaseParams;
use crate::sim::time::Time;
use crate::sim::world::{Terrain, World};

pub(super) fn act(c: &mut Creature, world: &mut World, events: &mut EventRing, time: &Time, roster: &Roster, cp: &CreaturesParams, dp: &DiseaseParams, rng: &mut Rng) {
    match c.goal {
        Goal::Graze => {
            if graze(c, world, cp) {
                c.last_ate = Some(time.tick);
            }
            disease::parasite_uptake(c, world, dp);
        }
        Goal::Drink => {
            if drink(c, world, cp) {
                c.last_drank = Some(time.tick);
            }
            disease::parasite_uptake(c, world, dp);
        }
        Goal::Rest => {
            if is_resting(c) {
                c.last_slept = Some(time.tick);
            }
            maybe_make_den(c, world, events, time, roster, cp, rng);
        }
        _ => {}
    }
}

/// Eat from the cell; `true` when there was anything to eat.
pub(super) fn graze(c: &mut Creature, world: &mut World, cp: &CreaturesParams) -> bool {
    let cell = world.cell_mut(c.x, c.y);
    let g = cell.vegetation.min(cp.graze_per_hour);
    cell.vegetation -= g;
    c.hunger = (c.hunger - cp.graze_nutrition * g).max(0.0);
    g > 0.0
}

/// Drink if standing on a shore; `true` when the creature drank.
pub(super) fn drink(c: &mut Creature, world: &World, cp: &CreaturesParams) -> bool {
    if !world.is_shore(c.x, c.y) {
        return false;
    }
    c.thirst = (c.thirst - cp.drink_per_hour).max(0.0);
    c.last_water = Some((c.x, c.y));
    true
}

pub(super) fn maybe_make_den(c: &Creature, world: &mut World, events: &mut EventRing, time: &Time, roster: &Roster, cp: &CreaturesParams, rng: &mut Rng) {
    if !is_resting(c) || in_den(c, world) {
        return;
    }
    let cell = world.cell(c.x, c.y);
    if cell.vegetation >= 0.2 || !matches!(cell.terrain, Terrain::Dirt | Terrain::GrassDense | Terrain::Forest) {
        return;
    }
    let ri = world.region_index(c.x, c.y);
    let region_dens = world.dens.iter().filter(|&&(dx, dy)| world.region_index(dx, dy) == ri).count();
    if region_dens >= cp.max_dens_per_region {
        return;
    }
    if rng.chance(cp.den_create_chance_per_rest_hour) {
        world.dens.push((c.x, c.y));
        events.push(Event {
            year: time.year(),
            day: time.day_of_year(),
            hour: time.hour(),
            kind: EventKind::Note,
            species: Some(c.species),
            subject: Some(c.id),
            text: format!("{} discovered a new den site {}", c.label(roster), world.place_name(c.x, c.y)),
            pos: Some((c.x, c.y)),
            detail: String::new(),
        });
    }
}

pub(super) fn is_resting(c: &Creature) -> bool {
    c.goal == Goal::Rest && (c.target.is_none() || c.target == Some((c.x, c.y)))
}

pub(super) fn in_den(c: &Creature, world: &World) -> bool {
    world.dens.iter().any(|&(x, y)| x == c.x && y == c.y)
}
