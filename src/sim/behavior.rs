//! Creature behaviour: perception, goal selection (with hysteresis), fractional
//! movement, grazing/drinking/resting, mating (C4), den creation, death and the
//! day boundary.

use crate::sim::creatures::{
    adult_age_days, Cause, Creature, CreatureId, CreatureStore, Death, DeathTallies,
};
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::genetics::{self, TickView};
use crate::sim::lineage::Lineage;
use crate::sim::params::{CreaturesParams, EcologyParams, GeneticsParams, PredationParams, SocialParams};
use crate::sim::rng::Rng;
use crate::sim::spatial::SpatialIndex;
use crate::sim::species::SpeciesId;
use crate::sim::disease::{self, DiseaseState};
use crate::sim::params::DiseaseParams;
use crate::sim::time::Time;
use crate::sim::world::World;

use goals::update_one;
use vitals::{in_den, is_resting};
use threat::mark_threats;
use hunt::{hunt_contacts, scavenge_contacts};
pub use perception::{Kin, Perception};
pub use migration::migration_daily;

/// Advance every living creature one tick, in slot order (FR9), then run the
/// C4/C5 passes: hunt contacts (kill/eat), scavenging, consummation and delivery.
#[allow(clippy::too_many_arguments)]
pub fn tick_creatures(
    store: &mut CreatureStore,
    spatial: &SpatialIndex,
    world: &mut World,
    events: &mut EventRing,
    time: &Time,
    cp: &CreaturesParams,
    ep: &EcologyParams,
    gp: &GeneticsParams,
    pp: &PredationParams,
    dp: &DiseaseParams,
    sp: &SocialParams,
    rng: &mut Rng,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
    soft_cap_noted: &mut bool,
    dstate: &mut DiseaseState,
    drng: &mut Rng,
) {
    let view = TickView::build(store, time, world, gp, dp);
    // FR5: predator-first threat marking (bucket-bounded; never per-prey scans).
    mark_threats(store, spatial, world, time.tick, pp, sp);
    for c in store.living_mut() {
        update_one(c, spatial, world, events, time, cp, ep, gp, pp, dp, sp, &view, rng, tallies, lineage);
    }
    // C7 FR4: infectious-first contagion over the same spatial snapshot.
    disease::contagion_pass(store, spatial, world, time, dp, dstate, drng);
    genetics::consummate(store, time, gp, events, &view, soft_cap_noted);
    hunt_contacts(store, world, events, time, pp, dp, sp, rng, tallies, lineage, dstate, drng);
    scavenge_contacts(store, world, events, time, pp, dp, dstate, drng);
    genetics::deliver(store, world, events, time, gp, cp, dp, rng, tallies, lineage, dstate, drng);
}

/// The day-boundary step: age death, adult re-evaluation, carcass decay/free and
/// pressure decay. Runs at midnight, before the census.
pub fn day_boundary(
    store: &mut CreatureStore,
    world: &mut World,
    events: &mut EventRing,
    time: &Time,
    cp: &CreaturesParams,
    gp: &GeneticsParams,
    dp: &DiseaseParams,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
    dstate: &mut DiseaseState,
    drng: &mut Rng,
) {
    let day_index = time.day_index();

    // C5 `FR5b`: flush the day's wary encounters as one event per region.
    flush_wary(world, events, time, tallies);

    // 0. C7 FR5: infection progression, lethality, recovery, parasite clearance.
    disease::progress_daily(store, world, events, time, dp, dstate, tallies, lineage, drng);

    // 1. Age death + adult re-evaluation (FR3, FR7).
    for c in store.living_mut() {
        let age = c.age_days(day_index);
        c.adult = age >= adult_age_days(c.species, &c.genome, cp, gp);
        if age >= c.max_age_days(cp, gp) {
            kill(c, Cause::Age, world, events, time, tallies, lineage, None, 0, None);
        }
    }

    // 2. Carcass decay and slot freeing (FR7).
    let decay_step = 1.0 / crate::cast!(cp.carcass_decay_days.max(1) => f32);
    let mut to_free: Vec<CreatureId> = Vec::new();
    for c in store.carcasses_mut() {
        c.decay += decay_step;
        if c.decay >= 1.0 {
            to_free.push(c.id);
        }
    }
    for id in to_free {
        if let Some(c) = store.remove(id) {
            world.carcasses.retain(|&(x, y)| !(x == c.x && y == c.y));
        }
    }

    // 3. Pressure decay (FR8): both traffic maps decay the same way.
    for cell in &mut world.cells {
        cell.prey_pressure *= cp.pressure_decay_per_day;
        cell.pred_pressure *= cp.pressure_decay_per_day;
    }
    disease::decay_cells(world, dp);
}

/// One region's wary summary, accumulated while flushing the day's tally.
struct WaryDay {
    region: usize,
    prey: SpeciesId,
    pred: SpeciesId,
    dominant: u32,
    total: u32,
}

/// C5 `FR5b`: flush the day's wary encounters into one event per region — the most
/// common prey/predator pair names the line and the total is the day's count.
/// `detail` carries `region:prey:predator:total` for tests, like Migration's
/// `origin>dest`. The tally is a `BTreeMap`, so regions are visited in ascending
/// order and the first of any tied pair wins: deterministic event order.
fn flush_wary(world: &World, events: &mut EventRing, time: &Time, tallies: &mut DeathTallies) {
    if tallies.wary_today.is_empty() {
        return;
    }
    let mut summaries: Vec<WaryDay> = Vec::new();
    for (&(ri, prey, pred), &n) in &tallies.wary_today {
        let region = usize::from(ri);
        match summaries.last_mut() {
            Some(last) if last.region == region => {
                last.total += n;
                if n > last.dominant {
                    last.prey = prey;
                    last.pred = pred;
                    last.dominant = n;
                }
            }
            _ => summaries.push(WaryDay { region, prey, pred, dominant: n, total: n }),
        }
    }
    let (year, day, hour) = (time.year(), time.day_of_year(), time.hour());
    for s in summaries {
        let r = &world.regions[s.region];
        let pos = ((r.1 + r.3).div_euclid(2), (r.2 + r.4).div_euclid(2));
        events.push(Event {
            year,
            day,
            hour,
            kind: EventKind::Wary,
            species: Some(s.prey),
            subject: None,
            text: format!(
                "{} give {} room in {} ({} wary encounters)",
                s.prey.plural(),
                s.pred.plural().to_lowercase(),
                r.0,
                s.total
            ),
            pos: Some(pos),
            detail: format!("{}:{}:{}:{}", s.region, s.prey.index(), s.pred.index(), s.total),
        });
    }
    tallies.wary_today.clear();
}

/// Mark a creature dead, add a carcass, record the tally and emit the event.
/// `killer`/`chase_ticks`/`killer_label` are set for predation deaths.
#[allow(clippy::too_many_arguments)]
pub(crate) fn kill(
    c: &mut Creature,
    cause: Cause,
    world: &mut World,
    events: &mut EventRing,
    time: &Time,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
    killer: Option<CreatureId>,
    chase_ticks: u16,
    killer_label: Option<&str>,
) {
    c.alive = false;
    c.hp = 0.0;
    c.target = None;
    c.pregnant_due = None;
    c.death = Some(Death { cause, day: crate::cast!(time.day_index() => u32), killer, chase_ticks });
    // C7: an animal that dies while infectious leaves an infectious carcass.
    if c.died_infected.is_none() && disease::is_infectious(c) {
        c.died_infected = c.infection.map(|i| i.pathogen);
    }
    let outbreak = c.infection.map(|i| i.outbreak);
    world.carcasses.push((c.x, c.y));
    tallies.deaths[c.species.index()] += 1;
    lineage.record_death(c.id, crate::cast!(time.day_index() => u32), cause, if cause == Cause::Disease { outbreak } else { None }, c.infections_survived);
    tallies.last_death[c.species.index()] = Some(crate::sim::creatures::ExtinctionRecord {
        species: c.species,
        last: c.id,
        name: c.name_str().to_string(),
        tag: c.tag(),
        cause,
        day: crate::cast!(time.day_index() => u32),
        age: c.age_days(time.day_index()),
        region: world.region_name(c.x, c.y).to_string(),
        pos: (c.x, c.y),
    });

    let kind = match cause {
        Cause::Starved => {
            tallies.starved += 1;
            EventKind::DeathStarved
        }
        Cause::Thirst => {
            tallies.thirst += 1;
            EventKind::DeathThirst
        }
        Cause::Age => {
            tallies.age += 1;
            EventKind::DeathAge
        }
        Cause::Predation => {
            tallies.predation += 1;
            EventKind::DeathPredation
        }
        Cause::Disease => {
            tallies.disease += 1;
            tallies.disease_by_species[c.species.index()] += 1;
            EventKind::DeathDisease
        }
        Cause::Injury => return, // no event until C5
    };

    let region = world.region_name(c.x, c.y).to_string();
    let text = match cause {
        Cause::Starved => format!("{} {} starved in {}", c.name_str(), c.tag(), region),
        Cause::Thirst => format!("{} {} died of thirst in {}", c.name_str(), c.tag(), region),
        Cause::Age => format!("{} {} died of old age at {} days in {}", c.name_str(), c.tag(), c.age_days(time.day_index()), region),
        Cause::Predation => match killer_label {
            Some(k) => format!("{} {} was killed by {} in {}", c.name_str(), c.tag(), k, region),
            None => format!("{} {} was killed in {}", c.name_str(), c.tag(), region),
        },
        Cause::Disease => match killer_label {
            Some(p) => format!("{} {} died of {} in {}", c.name_str(), c.tag(), p, region),
            None => format!("{} {} died of disease in {}", c.name_str(), c.tag(), region),
        },
        Cause::Injury => unreachable!(),
    };
    let detail = format!("hunger {:.2} thirst {:.2} energy {:.2}", c.hunger, c.thirst, c.energy);

    events.push(Event {
        year: time.year(),
        day: time.day_of_year(),
        hour: time.hour(),
        kind,
        species: Some(c.species),
        subject: Some(c.id),
        text,
        pos: Some((c.x, c.y)),
        detail,
    });
}

/// The nearest walkable cell within a small radius (wander fallback).
pub(crate) fn find_walkable_near(x: usize, y: usize, world: &World) -> Option<(usize, usize)> {
    for r in 1i32..=8 {
        for dy in -r..=r {
            for dx in -r..=r {
                let (nx, ny) = (crate::cast!(x => i32) + dx, crate::cast!(y => i32) + dy);
                if world.in_bounds(nx, ny) && world.cell(crate::cast!(nx => usize), crate::cast!(ny => usize)).terrain.walkable() {
                    return Some((crate::cast!(nx => usize), crate::cast!(ny => usize)));
                }
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn needs(c: &mut Creature, world: &World, time: &Time, cp: &CreaturesParams, ep: &EcologyParams, gp: &GeneticsParams, dp: &DiseaseParams, hunger_factor: f32) {
    let season_metabolism = ep.season_metabolism.get(&time.season()).copied().unwrap_or(1.0);
    let pregnancy = if c.pregnant_due.is_some() { gp.pregnancy_hunger_factor } else { 1.0 };
    // C7 FR6: `hunger_factor` carries the resistance cost, fever and parasite tax.
    c.hunger = (c.hunger + cp.hunger_per_hour(c.genome.size(), c.genome.metabolism(), season_metabolism) * pregnancy * hunger_factor).min(2.0);
    c.thirst = (c.thirst + cp.thirst_per_hour).min(2.0);

    if is_resting(c) {
        let bonus = if in_den(c, world) { cp.den_rest_bonus } else { 1.0 };
        c.energy = (c.energy + cp.energy_rest_per_hour * bonus).min(1.0);
    } else {
        c.energy -= cp.energy_awake_per_hour;
    }

    if c.hunger >= 1.0 || c.thirst >= 1.0 {
        c.hp -= cp.hp_loss_per_hour;
    } else if c.hunger < 0.5 && c.thirst < 0.5 {
        c.hp = (c.hp + cp.hp_regen_per_hour).min(1.0);
    }
    // C7 FR6: a heavy parasite load drains hp on its own.
    if dp.enabled && c.parasite_load > dp.parasite_hp_threshold {
        c.hp -= dp.parasite_hp_loss;
    }
}

mod perception;
mod movement;
mod goals;
mod vitals;
mod death;
mod threat;
mod hunt;
mod migration;
#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests;
#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests_vitals;
#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests_death;
#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests_flee;
#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests_wary;
#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests_migration;
#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests_social;
