//! Daily vegetation, moisture, drought and regrowth dynamics (FR2–FR4), and
//! the succession and trampling step (FR12, in `succession`).

use crate::sim::creatures::DeathTallies;
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::params::{EcologyParams, Rainfall, SuccessionParams};
use crate::sim::rng::Rng;
use crate::sim::stats::{Census, Sample, Series};
use crate::sim::time::{Season, Time};
use crate::sim::world::{Terrain, World};

/// True when any of the eight neighbours of `(x, y)` is a water cell.
fn has_adjacent_water(world: &World, x: usize, y: usize, w: usize, h: usize) -> bool {
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let (nx, ny) = (crate::cast!(x => i32) + dx, crate::cast!(y => i32) + dy);
            if nx >= 0
                && ny >= 0
                && (crate::cast!(nx => usize)) < w
                && (crate::cast!(ny => usize)) < h
                && world.cells[crate::cast!(ny => usize) * w + crate::cast!(nx => usize)].terrain.is_water()
            {
                return true;
            }
        }
    }
    false
}

/// Index and moisture of every shallow-water cell in region `ri`, row-major.
fn shallow_water_candidates(world: &World, ri: usize, w: usize) -> Vec<(usize, f32)> {
    world
        .region_cells(ri)
        .map(|(x, y)| y * w + x)
        .filter(|&idx| world.cells[idx].terrain == Terrain::ShallowWater)
        .map(|idx| (idx, world.cells[idx].moisture))
        .collect()
}

/// Refill up to `limit` dried-from cells in region `ri` to shallow water, row-major.
fn refill_dried_cells(world: &mut World, ri: usize, w: usize, limit: usize) {
    let dried: Vec<usize> = world
        .region_cells(ri)
        .map(|(x, y)| y * w + x)
        .filter(|&idx| world.cells[idx].dried_from == Some(Terrain::ShallowWater))
        .take(limit)
        .collect();
    for idx in dried {
        world.cells[idx].terrain = Terrain::ShallowWater;
        world.cells[idx].dried_from = None;
        world.cells[idx].vegetation = 0.0;
    }
}

/// Ecological pre-history run once when a world is made.
///
/// `days` of vegetation growth and die-back with no rain, evaporation,
/// creatures or draws, so a fresh world's biomass sits at the carrying
/// capacity its moisture, biome and `season` allow.
pub fn warm_up(world: &mut World, ecology: &EcologyParams, season: Season, days: u32) {
    let (w, h) = (world.width, world.height);
    let season_cap = ecology.season_cap.get(&season).copied().unwrap_or(1.0);
    let season_regrowth = ecology.season_regrowth.get(&season).copied().unwrap_or(1.0);
    for _ in 0..days {
        step_vegetation(world, ecology, None, season_cap, season_regrowth, w, h);
    }
    for cell in &mut world.cells {
        cell.vegetation = cell.vegetation.clamp(0.0, 1.0);
    }
}

/// Run one day's ecology update. Called from `Sim::step` when `hour() == 0`.
///
/// The order of the sub-steps is fixed for determinism (FR2).
#[allow(clippy::too_many_arguments)]
pub fn daily_update(
    world: &mut World,
    rng: &mut Rng,
    time: &Time,
    events: &mut EventRing,
    series: &mut Series,
    drought: &mut [bool; 8],
    drought_days_below: &mut [u32; 8],
    ecology: &EcologyParams,
    succession: &SuccessionParams,
    rainfall: Rainfall,
    c: &Census,
    deaths: &DeathTallies,
    disease: &crate::sim::disease::DiseaseState,
) {
    let season = time.season();
    let (w, h) = (world.width, world.height);
    let n_regions = world.regions.len().min(8);

    let rain_chance = ecology.rain_chance_per_day.get(&rainfall).copied().unwrap_or(0.25);
    let season_cap = ecology.season_cap.get(&season).copied().unwrap_or(1.0);
    let season_regrowth = ecology.season_regrowth.get(&season).copied().unwrap_or(1.0);
    let season_evap = ecology.season_evaporation.get(&season).copied().unwrap_or(1.0);
    step_rain(world, rng, n_regions, rain_chance, ecology, w);
    step_water_adjacency(world, w, h);
    let targets = step_vegetation(world, ecology, Some(succession), season_cap, season_regrowth, w, h);
    step_evaporate_clamp(world, ecology, season_evap);
    step_drought(world, time, events, n_regions, drought, drought_days_below, ecology);
    step_water_sand(world, n_regions, *drought, ecology, w);
    world.refresh_shore();
    step_regrowth_sites(world, rng, time, events, ecology, w, h);
    succession::step_succession(world, rng, time, events, ecology, succession, &targets, w, h);
    series.push(sample_series(world, time, *drought, n_regions, c, deaths, disease));
}

/// Step 1: one rain roll per region; on success every cell in it is moistened.
fn step_rain(world: &mut World, rng: &mut Rng, n_regions: usize, rain_chance: f32, ecology: &EcologyParams, w: usize) {
    for ri in 0..n_regions {
        if rng.chance(rain_chance) {
            let cells: Vec<usize> = world.region_cells(ri).map(|(x, y)| y * w + x).collect();
            for idx in cells {
                world.cells[idx].moisture += ecology.rain_amount;
            }
        }
    }
}

/// Step 2: land cells touching water gain a little moisture.
fn step_water_adjacency(world: &mut World, w: usize, h: usize) {
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if world.cells[idx].terrain.is_water() {
                continue;
            }
            if has_adjacent_water(world, x, y, w, h) {
                world.cells[idx].moisture += 0.01;
            }
        }
    }
}

/// Step 3: vegetation grows toward, or decays toward, its seasonal target.
///
/// FR12: with `trample` set, prey traffic scales the target down by its
/// trampling factor. Returns the *untrampled* target of every cell (0 for
/// water and rock) for the succession step to read.
fn step_vegetation(world: &mut World, ecology: &EcologyParams, trample: Option<&SuccessionParams>, season_cap: f32, season_regrowth: f32, w: usize, h: usize) -> Vec<f32> {
    let seed_grid = seed_mask(world);
    let mut targets = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let (terrain, biome, moisture, v, pressure) = {
                let c = &world.cells[idx];
                (c.terrain, c.biome, c.moisture, c.vegetation, c.prey_pressure)
            };
            if terrain.is_water() {
                continue;
            }
            let max_v = ecology.max_vegetation.get(&terrain).copied().unwrap_or(0.0);
            if max_v <= 0.0 {
                continue;
            }
            let untrampled = max_v * biome.vegetation_scale() * season_cap * (moisture / 0.5).min(1.0);
            targets[idx] = untrampled;
            let target = trample.map_or(untrampled, |sp| untrampled * sp.trample_factor(pressure));
            let new_v = if v < target {
                let rate = if seed_grid[idx] { 2.0 * ecology.growth_k } else { ecology.growth_k };
                v + rate * ecology.regrowth_rate * season_regrowth * (target - v)
            } else {
                v - ecology.dieback_k * (v - target)
            };
            world.cells[idx].vegetation = new_v;
        }
    }
    targets
}

/// Steps 4 and 5: evaporate, then clamp moisture and vegetation to 0..1.
fn step_evaporate_clamp(world: &mut World, ecology: &EcologyParams, season_evap: f32) {
    for cell in &mut world.cells {
        cell.moisture -= ecology.evap_k * season_evap * cell.moisture;
    }

    // 5. Clamp moisture and vegetation to 0..1.
    for cell in &mut world.cells {
        cell.moisture = cell.moisture.clamp(0.0, 1.0);
        cell.vegetation = cell.vegetation.clamp(0.0, 1.0);
    }
}

/// Step 6: per-region drought detection on land-cell stored moisture.
fn step_drought(
    world: &World,
    time: &Time,
    events: &mut EventRing,
    n_regions: usize,
    drought: &mut [bool; 8],
    drought_days_below: &mut [u32; 8],
    ecology: &EcologyParams,
) {
    for ri in 0..n_regions {
        let mean = region_land_moisture_mean(world, ri);
        let name = &world.regions[ri].0;
        if drought[ri] {
            if mean > ecology.drought_moisture + ecology.drought_recover_margin {
                drought[ri] = false;
                drought_days_below[ri] = 0;
                events.push(region_event(time, EventKind::DroughtEased, world, ri, format!("Drought eases in {name}")));
            }
        } else if mean < ecology.drought_moisture {
            drought_days_below[ri] += 1;
            if drought_days_below[ri] >= ecology.drought_days {
                drought[ri] = true;
                let n = count_shallow_water(world, ri);
                events.push(region_event(time, EventKind::Drought, world, ri, format!("Drought grips {name}; {n} water cells at risk")));
            }
        } else {
            drought_days_below[ri] = 0;
        }
    }
}

/// Step 7: dry or refill shallow water depending on regional moisture.
fn step_water_sand(world: &mut World, n_regions: usize, drought: [bool; 8], ecology: &EcologyParams, w: usize) {
    for ri in 0..n_regions {
        let mean = region_land_moisture_mean(world, ri);
        if drought[ri] && mean < ecology.water_dry_region_moisture {
            // Dry the lowest-moisture shallow water first (ties by row-major index).
            let mut candidates = shallow_water_candidates(world, ri, w);
            candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
            for (idx, _) in candidates.into_iter().take(ecology.water_changes_per_region_per_day) {
                world.cells[idx].terrain = Terrain::Sand;
                world.cells[idx].dried_from = Some(Terrain::ShallowWater);
                world.cells[idx].vegetation = 0.0;
            }
        }
        if mean > ecology.water_refill_region_moisture {
            // Refill dried-from cells in row-major order.
            refill_dried_cells(world, ri, w, ecology.water_changes_per_region_per_day);
        }
    }
}

/// Step 8: sprout new regrowth sites and retire ones that have recovered.
fn step_regrowth_sites(world: &mut World, rng: &mut Rng, time: &Time, events: &mut EventRing, ecology: &EcologyParams, w: usize, h: usize) {

    // 8. Regrowth sites.
    let mut note_emitted = [false; 8];
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let (terrain, v) = (world.cells[idx].terrain, world.cells[idx].vegetation);
            if (terrain == Terrain::Dirt || terrain == Terrain::GrassSparse)
                && v < 0.1
                && rng.chance(ecology.seed_sprout_chance_per_day)
                && !world.seeds.contains(&(x, y))
            {
                world.seeds.push((x, y));
                let ri = world.region_index(x, y).min(7);
                if !note_emitted[ri] {
                    note_emitted[ri] = true;
                    events.push(region_event(time, EventKind::Note, world, ri, format!("A regrowth site sprouted in {}", world.regions[ri].0)));
                }
            }
        }
    }
    // Remove sites that have grown back to 0.8 × their terrain cap.
    world.seeds.retain(|&(sx, sy)| {
        let c = &world.cells[sy * w + sx];
        let max_v = ecology.max_vegetation.get(&c.terrain).copied().unwrap_or(0.0);
        c.vegetation < 0.8 * max_v
    });
}

/// Step 9: build the daily series sample.
fn sample_series(world: &World, time: &Time, drought: [bool; 8], n_regions: usize, c: &Census, deaths: &DeathTallies, disease: &crate::sim::disease::DiseaseState) -> Sample {
    let mut biomass_total = 0.0;
    let mut veg_sum = 0.0;
    let mut veg_count = 0usize;
    let mut water_cells = 0usize;
    let mut moist_sum = 0.0;
    let (mut forest_cells, mut bare_cells) = (0usize, 0usize);
    for c in &world.cells {
        biomass_total += c.vegetation;
        match c.terrain {
            Terrain::Forest => forest_cells += 1,
            Terrain::Dirt | Terrain::Sand => bare_cells += 1,
            _ => {}
        }
        if c.terrain.is_water() {
            water_cells += 1;
            moist_sum += 1.0;
        } else {
            veg_sum += c.vegetation;
            veg_count += 1;
            moist_sum += c.moisture;
        }
    }
    let veg_mean = if veg_count > 0 { veg_sum / crate::cast!(veg_count => f32) } else { 0.0 };
    let moisture_mean = moist_sum / crate::cast!(world.cells.len().max(1) => f32);
    let water_level = if world.water_cells_at_generation > 0 {
        crate::cast!(water_cells => f32) / crate::cast!(world.water_cells_at_generation => f32)
    } else {
        0.0
    };
    let mut region_veg = [0.0f32; 8];
    let mut region_moist = [0.0f32; 8];
    for ri in 0..n_regions.min(8) {
        region_veg[ri] = region_land_veg_mean(world, ri);
        region_moist[ri] = region_display_moisture_mean(world, ri);
    }
    Sample {
        day: crate::cast!(time.day_index() => u32),
        biomass_total,
        veg_mean,
        water_cells,
        water_level,
        moisture_mean,
        seeds: world.seeds.len(),
        dens: world.dens.len(),
        carcasses: world.carcasses.len(),
        drought_regions: drought.iter().filter(|&&f| f).count(),
        drought_flags: drought,
        forest_cells,
        bare_cells,
        region_veg,
        region_moist,
        population: c.population.clone(),
        adults: c.adults.clone(),
        juveniles: c.juveniles.clone(),
        deaths_starved: deaths.starved,
        deaths_thirst: deaths.thirst,
        deaths_age: deaths.age,
        genome_mean: c.genome_mean.clone(),
        genome_min: c.genome_min.clone(),
        genome_max: c.genome_max.clone(),
        births: deaths.births.clone(),
        deaths: deaths.deaths.clone(),
        generation_mean: (0..c.population.len()).map(|i| c.generation_mean(i)).collect(),
        generation_max: c.max_generation.clone(),
        infected: c.infected.clone(),
        immune: c.immune.clone(),
        deaths_disease: deaths.disease,
        parasite_mean: (0..c.population.len()).map(|i| c.parasite_mean(i)).collect(),
        active_by_pathogen: std::array::from_fn(|i| disease.stats[i].active),
    }
}

/// An event anchored on region `ri`'s centre cell.
pub(super) fn region_event(time: &Time, kind: EventKind, world: &World, ri: usize, text: String) -> Event {
    Event {
        year: time.year(),
        day: time.day_of_year(),
        hour: time.hour(),
        kind,
        species: None,
        subject: None,
        text,
        pos: Some(world.region_centre(ri)),
        detail: String::new(),
    }
}

fn seed_mask(world: &World) -> Vec<bool> {
    let mut m = vec![false; world.cells.len()];
    for &(x, y) in &world.seeds {
        m[y * world.width + x] = true;
    }
    m
}

fn count_shallow_water(world: &World, ri: usize) -> usize {
    world.region_cells(ri).filter(|&(x, y)| world.cell(x, y).terrain == Terrain::ShallowWater).count()
}

/// Mean of `f` over the land cells of region `ri` (water excluded, rock included).
fn region_land_mean(world: &World, ri: usize, f: impl Fn(&crate::sim::world::Cell) -> f32) -> f32 {
    let (mut sum, mut n) = (0.0f32, 0usize);
    for (x, y) in world.region_cells(ri) {
        let c = world.cell(x, y);
        if !c.terrain.is_water() {
            sum += f(c);
            n += 1;
        }
    }
    if n > 0 { sum / crate::cast!(n => f32) } else { 0.0 }
}

/// Mean vegetation over land cells (water excluded, rock included).
pub fn region_land_veg_mean(world: &World, ri: usize) -> f32 {
    region_land_mean(world, ri, |c| c.vegetation)
}

/// Mean stored moisture over land cells (water excluded, rock included) — the
/// drought detector's view.
pub fn region_land_moisture_mean(world: &World, ri: usize) -> f32 {
    region_land_mean(world, ri, |c| c.moisture)
}

/// Mean moisture over all cells, water treated as 1.0 (the display convention).
pub fn region_display_moisture_mean(world: &World, ri: usize) -> f32 {
    let mut sum = 0.0f32;
    let mut n = 0usize;
    for (x, y) in world.region_cells(ri) {
        let c = world.cell(x, y);
        sum += if c.terrain.is_water() { 1.0 } else { c.moisture };
        n += 1;
    }
    if n > 0 { sum / crate::cast!(n => f32) } else { 0.0 }
}

mod succession;

#[cfg(test)]
mod tests;
