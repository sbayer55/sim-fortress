//! Daily regional migration (C5 FR7).

use crate::sim::creatures::{
    CreatureId, CreatureStore, Goal,
};
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::params::{EcologyParams, PredationParams, Roster};
use crate::sim::species::{Kind, SpeciesId};
use crate::sim::time::Time;
use crate::sim::world::World;

/// C5 FR7: daily per-(species, region) migration evaluation. A group migrates
/// when its trigger holds for `migrate_days` consecutive days and its
/// (species, region) cooldown has passed.
pub fn migration_daily(
    store: &mut CreatureStore,
    world: &World,
    events: &mut EventRing,
    time: &Time,
    roster: &Roster,
    ep: &EcologyParams,
    pp: &PredationParams,
    cooldown_until: &mut [u64],
    days_below: &mut [u32],
) {
    let n_regions = world.regions.len().min(8);
    let mut counts = vec![[0u32; 8]; roster.len()];
    let mut prey_per_region = [0u32; 8];
    // Sum of `pred_pressure` over the cells the group occupies (FR7: the prey
    // trigger uses the mean over occupied cells, not the region mean).
    let mut occupied_pressure = vec![[0.0f32; 8]; roster.len()];
    for c in store.living() {
        let ri = world.region_index(c.x, c.y).min(7);
        counts[c.species.index()][ri] += 1;
        occupied_pressure[c.species.index()][ri] += world.cell(c.x, c.y).pred_pressure;
        if roster.kind(c.species) == Kind::Prey {
            prey_per_region[ri] += 1;
        }
    }
    let season_cap = ep.season_cap.get(&time.season()).copied().unwrap_or(1.0);

    for id in roster.ids() {
        let si = id.index();
        for ri in 0..n_regions {
            let key = si * 8 + ri;
            if counts[si][ri] == 0 || time.tick < cooldown_until[key] {
                days_below[key] = 0;
                continue;
            }
            let trigger = if roster.kind(id) == Kind::Prey {
                let veg_mean = crate::sim::ecology::region_land_veg_mean(world, ri);
                let shortfall = season_cap > 0.0 && veg_mean / season_cap < pp.migrate_veg;
                let group_pressure = occupied_pressure[si][ri] / crate::cast!(counts[si][ri] => f32);
                let pressure = group_pressure > pp.migrate_pressure;
                shortfall || pressure
            } else {
                prey_per_region[ri] < pp.migrate_prey_min
            };
            if trigger {
                days_below[key] += 1;
                if days_below[key] >= pp.migrate_days {
                    days_below[key] = 0;
                    cooldown_until[key] = time.tick + u64::from(pp.migrate_cooldown_days) * u64::from(time.ticks_per_day);
                    migrate_group(store, world, events, time, roster, id, ri);
                }
            } else {
                days_below[key] = 0;
            }
        }
    }
}

/// Mean `pred_pressure` over the land cells of region `ri`.
pub(super) fn mean_pred_pressure(world: &World, ri: usize) -> f32 {
    let (mut sum, mut n) = (0.0f32, 0usize);
    for (x, y) in world.region_cells(ri) {
        let c = world.cell(x, y);
        if !c.terrain.is_water() {
            sum += c.pred_pressure;
            n += 1;
        }
    }
    if n > 0 { sum / crate::cast!(n => f32) } else { 0.0 }
}

pub(super) fn migrate_group(store: &mut CreatureStore, world: &World, events: &mut EventRing, time: &Time, roster: &Roster, id: SpeciesId, origin_ri: usize) {
    let members: Vec<CreatureId> = store
        .living()
        .filter(|c| c.species == id && world.region_index(c.x, c.y) == origin_ri)
        .map(|c| c.id)
        .collect();
    if members.is_empty() {
        return;
    }
    let n = crate::cast!(members.len() => u32);

    // Destination = adjacent region maximising the species' score.
    let mut best_dest: Option<usize> = None;
    let mut best_score = f32::NEG_INFINITY;
    for di in 0..world.regions.len() {
        if di == origin_ri || !world.regions_adjacent(origin_ri, di) {
            continue;
        }
        let score = if roster.kind(id) == Kind::Prey {
            crate::sim::ecology::region_land_veg_mean(world, di) * (1.0 - mean_pred_pressure(world, di))
        } else {
            crate::cast!(store.living().filter(|c| roster.kind(c.species) == Kind::Prey && world.region_index(c.x, c.y) == di).count() => f32)
        };
        if score > best_score {
            best_score = score;
            best_dest = Some(di);
        }
    }
    let Some(dest) = best_dest else { return };
    let Some((tx, ty)) = migration_target_cell(world, roster.kind(id), dest) else { return };

    let migrate_until = time.tick + 2 * u64::from(time.ticks_per_day);
    for mid in &members {
        if let Some(c) = store.get_mut(*mid) {
            c.goal = Goal::Migrate;
            c.migrate_target = Some((tx, ty));
            c.migrate_until = migrate_until;
            c.target = Some((tx, ty));
            c.replan_at = time.tick;
        }
    }

    let group_word = if n <= 3 { "family" } else if roster.kind(id) == Kind::Prey { "herd" } else { "pack" };
    let origin = &world.regions[origin_ri];
    let dest_name = world.regions[dest].0.clone();
    let pos = world.region_centre(origin_ri);
    events.push(Event {
        year: time.year(),
        day: time.day_of_year(),
        hour: time.hour(),
        kind: EventKind::Migration,
        species: Some(id),
        subject: None,
        text: format!("A {} of {} {} migrated from {} to {}", group_word, n, roster.plural(id), origin.0, dest_name),
        pos: Some(pos),
        // `detail` carries "origin→dest" region indices for tests and S06.
        detail: format!("{origin_ri}>{dest}"),
    });
}

/// The walkable destination cell with the highest vegetation (prey) or
/// `prey_pressure` (predators) — the group's `target_cell` (FR7).
fn migration_target_cell(world: &World, kind: Kind, dest: usize) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    let mut best_score = f32::NEG_INFINITY;
    for (x, y) in world.region_cells(dest) {
        let c = world.cell(x, y);
        if !c.terrain.walkable() || c.terrain.is_water() {
            continue;
        }
        let score = if kind == Kind::Prey { c.vegetation } else { c.prey_pressure };
        if score > best_score {
            best_score = score;
            best = Some((x, y));
        }
    }
    best
}
