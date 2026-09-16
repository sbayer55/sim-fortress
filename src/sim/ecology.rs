//! Daily vegetation, moisture, drought and regrowth dynamics (FR2–FR4).

use crate::sim::creatures::DeathTallies;
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::params::{EcologyParams, Rainfall};
use crate::sim::rng::Rng;
use crate::sim::stats::{Census, Sample, Series};
use crate::sim::time::Time;
use crate::sim::world::{Terrain, World};

type RegionRect = (String, usize, usize, usize, usize);

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

/// Index and moisture of every shallow-water cell in `r`, in row-major order.
fn shallow_water_candidates(world: &World, r: &RegionRect, w: usize) -> Vec<(usize, f32)> {
    let mut out = Vec::new();
    for y in r.2..r.4 {
        for x in r.1..r.3 {
            let idx = y * w + x;
            if world.cells[idx].terrain == Terrain::ShallowWater {
                out.push((idx, world.cells[idx].moisture));
            }
        }
    }
    out
}

/// Refill up to `limit` dried-from cells in `r` to shallow water, row-major.
fn refill_dried_cells(world: &mut World, r: &RegionRect, w: usize, limit: usize) {
    let mut done = 0usize;
    'refill: for y in r.2..r.4 {
        for x in r.1..r.3 {
            let idx = y * w + x;
            if world.cells[idx].dried_from == Some(Terrain::ShallowWater) {
                world.cells[idx].terrain = Terrain::ShallowWater;
                world.cells[idx].dried_from = None;
                world.cells[idx].vegetation = 0.0;
                done += 1;
                if done >= limit {
                    break 'refill;
                }
            }
        }
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
    rainfall: Rainfall,
    c: &Census,
    deaths: &DeathTallies,
    disease: &crate::sim::disease::DiseaseState,
) {
    let season = time.season();
    let (w, h) = (world.width, world.height);
    let regions = world.regions.clone();

    let rain_chance = ecology.rain_chance_per_day.get(&rainfall).copied().unwrap_or(0.25);
    let season_cap = ecology.season_cap.get(&season).copied().unwrap_or(1.0);
    let season_regrowth = ecology.season_regrowth.get(&season).copied().unwrap_or(1.0);
    let season_evap = ecology.season_evaporation.get(&season).copied().unwrap_or(1.0);
    step_rain(world, rng, &regions, rain_chance, ecology, w);
    step_water_adjacency(world, w, h);
    step_vegetation(world, ecology, season_cap, season_regrowth, w, h);
    step_evaporate_clamp(world, ecology, season_evap);
    step_drought(world, time, events, &regions, drought, drought_days_below, ecology);
    step_water_sand(world, &regions, *drought, ecology, w);
    world.refresh_shore();
    step_regrowth_sites(world, rng, time, events, &regions, ecology, w, h);
    series.push(sample_series(world, time, *drought, &regions, c, deaths, disease));
}

/// Step 1: one rain roll per region; on success every cell in it is moistened.
fn step_rain(world: &mut World, rng: &mut Rng, regions: &[RegionRect], rain_chance: f32, ecology: &EcologyParams, w: usize) {
    for r in regions {
        if rng.chance(rain_chance) {
            for y in r.2..r.4 {
                for x in r.1..r.3 {
                    world.cells[y * w + x].moisture += ecology.rain_amount;
                }
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
fn step_vegetation(world: &mut World, ecology: &EcologyParams, season_cap: f32, season_regrowth: f32, w: usize, h: usize) {
    let seed_grid = seed_mask(world);
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let (terrain, moisture, v) = {
                let c = &world.cells[idx];
                (c.terrain, c.moisture, c.vegetation)
            };
            if terrain.is_water() {
                continue;
            }
            let max_v = ecology.max_vegetation.get(&terrain).copied().unwrap_or(0.0);
            if max_v <= 0.0 {
                continue;
            }
            let target = max_v * season_cap * (moisture / 0.5).min(1.0);
            let new_v = if v < target {
                let rate = if seed_grid[idx] { 2.0 * ecology.growth_k } else { ecology.growth_k };
                v + rate * ecology.regrowth_rate * season_regrowth * (target - v)
            } else {
                v - ecology.dieback_k * (v - target)
            };
            world.cells[idx].vegetation = new_v;
        }
    }
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
    regions: &[RegionRect],
    drought: &mut [bool; 8],
    drought_days_below: &mut [u32; 8],
    ecology: &EcologyParams,
) {
    for ri in 0..regions.len() {
        let mean = region_land_moisture_mean(world, &regions[ri]);
        if drought[ri] {
            if mean > ecology.drought_moisture + ecology.drought_recover_margin {
                drought[ri] = false;
                drought_days_below[ri] = 0;
                events.push(region_event(time, EventKind::DroughtEased, &regions[ri], format!("Drought eases in {}", regions[ri].0)));
            }
        } else if mean < ecology.drought_moisture {
            drought_days_below[ri] += 1;
            if drought_days_below[ri] >= ecology.drought_days {
                drought[ri] = true;
                let n = count_shallow_water(world, &regions[ri]);
                events.push(region_event(time, EventKind::Drought, &regions[ri], format!("Drought grips {}; {} water cells at risk", regions[ri].0, n)));
            }
        } else {
            drought_days_below[ri] = 0;
        }
    }
}

/// Step 7: dry or refill shallow water depending on regional moisture.
fn step_water_sand(world: &mut World, regions: &[RegionRect], drought: [bool; 8], ecology: &EcologyParams, w: usize) {
    for ri in 0..regions.len() {
        let r = &regions[ri];
        let mean = region_land_moisture_mean(world, r);
        if drought[ri] && mean < ecology.water_dry_region_moisture {
            // Dry the lowest-moisture shallow water first (ties by row-major index).
            let mut candidates = shallow_water_candidates(world, r, w);
            candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
            for (idx, _) in candidates.into_iter().take(ecology.water_changes_per_region_per_day) {
                world.cells[idx].terrain = Terrain::Sand;
                world.cells[idx].dried_from = Some(Terrain::ShallowWater);
                world.cells[idx].vegetation = 0.0;
            }
        }
        if mean > ecology.water_refill_region_moisture {
            // Refill dried-from cells in row-major order.
            refill_dried_cells(world, r, w, ecology.water_changes_per_region_per_day);
        }
    }
}

/// Step 8: sprout new regrowth sites and retire ones that have recovered.
fn step_regrowth_sites(world: &mut World, rng: &mut Rng, time: &Time, events: &mut EventRing, regions: &[RegionRect], ecology: &EcologyParams, w: usize, h: usize) {

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
                let ri = region_index(regions, x, y);
                if !note_emitted[ri] {
                    note_emitted[ri] = true;
                    events.push(region_event(time, EventKind::Note, &regions[ri], format!("A regrowth site sprouted in {}", regions[ri].0)));
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
fn sample_series(world: &World, time: &Time, drought: [bool; 8], regions: &[RegionRect], c: &Census, deaths: &DeathTallies, disease: &crate::sim::disease::DiseaseState) -> Sample {
    let mut biomass_total = 0.0;
    let mut veg_sum = 0.0;
    let mut veg_count = 0usize;
    let mut water_cells = 0usize;
    let mut moist_sum = 0.0;
    for c in &world.cells {
        biomass_total += c.vegetation;
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
    for (ri, r) in regions.iter().enumerate() {
        region_veg[ri] = region_land_veg_mean(world, r);
        region_moist[ri] = region_display_moisture_mean(world, r);
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

fn region_event(time: &Time, kind: EventKind, r: &RegionRect, text: String) -> Event {
    Event {
        year: time.year(),
        day: time.day_of_year(),
        hour: time.hour(),
        kind,
        species: None,
        subject: None,
        text,
        pos: Some(((r.1 + r.3).div_euclid(2), (r.2 + r.4).div_euclid(2))),
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

fn count_shallow_water(world: &World, r: &RegionRect) -> usize {
    let mut n = 0;
    for y in r.2..r.4 {
        for x in r.1..r.3 {
            if world.cells[y * world.width + x].terrain == Terrain::ShallowWater {
                n += 1;
            }
        }
    }
    n
}

fn region_index(regions: &[RegionRect], x: usize, y: usize) -> usize {
    regions
        .iter()
        .position(|(_, x0, y0, x1, y1)| x >= *x0 && x < *x1 && y >= *y0 && y < *y1)
        .unwrap_or(0)
}

/// Mean vegetation over land cells (water excluded, rock included).
pub fn region_land_veg_mean(world: &World, r: &RegionRect) -> f32 {
    let (mut sum, mut n) = (0.0f32, 0usize);
    for y in r.2..r.4 {
        for x in r.1..r.3 {
            let c = &world.cells[y * world.width + x];
            if !c.terrain.is_water() {
                sum += c.vegetation;
                n += 1;
            }
        }
    }
    if n > 0 { sum / crate::cast!(n => f32) } else { 0.0 }
}

/// Mean stored moisture over land cells (water excluded, rock included) — the
/// drought detector's view.
pub fn region_land_moisture_mean(world: &World, r: &RegionRect) -> f32 {
    let (mut sum, mut n) = (0.0f32, 0usize);
    for y in r.2..r.4 {
        for x in r.1..r.3 {
            let c = &world.cells[y * world.width + x];
            if !c.terrain.is_water() {
                sum += c.moisture;
                n += 1;
            }
        }
    }
    if n > 0 { sum / crate::cast!(n => f32) } else { 0.0 }
}

/// Mean moisture over all cells, water treated as 1.0 (the display convention).
pub fn region_display_moisture_mean(world: &World, r: &RegionRect) -> f32 {
    let mut sum = 0.0f32;
    let mut n = 0usize;
    for y in r.2..r.4 {
        for x in r.1..r.3 {
            let c = &world.cells[y * world.width + x];
            sum += if c.terrain.is_water() { 1.0 } else { c.moisture };
            n += 1;
        }
    }
    if n > 0 { sum / crate::cast!(n => f32) } else { 0.0 }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {

    use super::*;
    use crate::sim::{Cell, Params, Sim};

    fn run_days(sim: &mut Sim, days: u64) {
        for _ in 0..(days * 24) {
            sim.step();
        }
    }

    #[test]
    fn growth_saturates_at_season_target() {
        let mut sim = Sim::new(42, Params::default());
        run_days(&mut sim, 720);
        let eco = &sim.params.ecology;
        for c in &sim.world.cells {
            if c.terrain.is_water() {
                assert_eq!(c.vegetation, 0.0);
                continue;
            }
            let max_v = eco.max_vegetation.get(&c.terrain).copied().unwrap_or(0.0);
            assert!(c.vegetation <= max_v + 0.001, "{:?} veg {} exceeds cap {}", c.terrain, c.vegetation, max_v);
        }
        assert!(sim.series.last().unwrap().veg_mean > 0.1, "vegetation did not grow");
    }

    #[test]
    fn winter_lowers_equilibrium() {
        let mut sim = Sim::new(42, Params::default());
        run_days(&mut sim, 720);
        let samples = sim.series.samples();
        let summer: f32 = samples.iter().filter(|s| (450..540).contains(&s.day)).map(|s| s.veg_mean).sum::<f32>() / 90.0;
        let winter: f32 = samples.iter().filter(|s| (630..720).contains(&s.day)).map(|s| s.veg_mean).sum::<f32>() / 90.0;
        assert!(winter < summer * 0.8, "winter {winter} not much below summer {summer}");
    }

    #[test]
    fn moisture_equilibria_by_rainfall() {
        let means: Vec<f32> = [Rainfall::Dry, Rainfall::Normal, Rainfall::Wet]
            .iter()
            .map(|&rf| {
                let mut p = Params::default();
                p.world.rainfall = rf;
                p.species.clear_initial_counts(); // pure ecology test, no grazing
                let mut sim = Sim::new(42, p);
                run_days(&mut sim, 720);
                let (s, n) = sim
                    .world
                    .cells
                    .iter()
                    .filter(|c| !c.terrain.is_water())
                    .fold((0.0f32, 0usize), |(s, n), c| (s + c.moisture, n + 1));
                s / crate::cast!(n => f32)
            })
            .collect();
        assert!(means[0] < means[1], "dry {} not < normal {}", means[0], means[1]);
        assert!(means[1] < means[2], "normal {} not < wet {}", means[1], means[2]);
        // The rain sequence follows the ecology RNG stream, which regrowth-site
        // sampling advances; C4's `growth_k` retune moved the dry mean from ~0.48 to ~0.56.
        assert!(means[0] < 0.6, "dry too wet: {}", means[0]);
        assert!(means[2] > 0.8, "wet too dry: {}", means[2]);
    }

    #[test]
    fn drought_flags_after_n_days_and_recovers() {
        let mut sim = Sim::new(42, Params::default());
        for c in &mut sim.world.cells {
            if !c.terrain.is_water() {
                c.moisture = 0.0;
            }
        }
        run_days(&mut sim, 8);
        assert!(sim.drought.iter().any(|&f| f), "expected at least one region flagged");
        for c in &mut sim.world.cells {
            if !c.terrain.is_water() {
                c.moisture = 1.0;
            }
        }
        run_days(&mut sim, 1);
        assert!(sim.drought.iter().all(|&f| !f), "expected drought to clear");
    }

    #[test]
    fn water_dries_and_refills_with_marker() {
        // Ecology only: creatures (grazing, and since C5 predation) perturb the
        // vegetation the rain stream is drawn against, so keep the world empty.
        let mut p = Params::default();
        p.species.clear_initial_counts();
        let mut sim = Sim::new(42, p);
        for c in &mut sim.world.cells {
            if !c.terrain.is_water() {
                c.moisture = 0.0;
            }
        }
        // Flag a drought, then keep running so water dries to sand.
        run_days(&mut sim, 20);
        let dried = sim.world.cells.iter().filter(|c| c.dried_from == Some(Terrain::ShallowWater)).count();
        assert!(dried > 0, "expected some shallow water to dry to sand");
        // Refill.
        for c in &mut sim.world.cells {
            if !c.terrain.is_water() {
                c.moisture = 1.0;
            }
        }
        // Refill is rate-limited to `water_changes_per_region_per_day` cells/day.
        run_days(&mut sim, 30);
        let still_dried = sim.world.cells.iter().filter(|c| c.dried_from == Some(Terrain::ShallowWater)).count();
        assert_eq!(still_dried, 0, "expected dried cells to refill");
    }

    #[test]
    fn seeds_sprout_and_clear_on_dirt() {
        let mut p = Params::default();
        p.species.clear_initial_counts(); // pure ecology test, no grazing
        let mut sim = Sim::new(42, p);
        // Sites sprout on bare dirt and sparse grass; strip both so every
        // seed's world offers plenty of candidates.
        for c in &mut sim.world.cells {
            if matches!(c.terrain, Terrain::Dirt | Terrain::GrassSparse) {
                c.vegetation = 0.0;
            }
        }
        run_days(&mut sim, 10);
        assert!(!sim.world.seeds.is_empty(), "expected regrowth sites to sprout");
        // A site whose vegetation reaches the clear threshold is removed.
        let (sx, sy) = sim.world.seeds[0];
        sim.world.cells[sy * sim.world.width + sx].vegetation = 0.5;
        run_days(&mut sim, 1);
        assert!(!sim.world.seeds.contains(&(sx, sy)), "expected the site to clear");
    }

    #[test]
    fn one_seed_note_per_region_per_day() {
        let mut sim = Sim::new(42, Params::default());
        for c in &mut sim.world.cells {
            if matches!(c.terrain, Terrain::Dirt | Terrain::GrassSparse) {
                c.vegetation = 0.0;
            }
        }
        let before = sim.events.len();
        run_days(&mut sim, 1);
        let notes: Vec<&Event> = sim.events.iter().skip(before).filter(|e| e.kind == EventKind::Note).collect();
        assert!(!notes.is_empty());
        assert!(notes.len() <= 8, "{} notes in one day", notes.len());
        let mut texts: Vec<&str> = notes.iter().map(|e| e.text.as_str()).collect();
        texts.sort_unstable();
        texts.dedup();
        assert_eq!(texts.len(), notes.len(), "duplicate region notes in one day");
    }

    #[test]
    fn region_means_exclude_water() {
        let cells = vec![
            Cell { terrain: Terrain::ShallowWater, elevation: 0.5, moisture: 0.5, vegetation: 0.9, prey_pressure: 0.0, pred_pressure: 0.0, dried_from: None, parasite_load: 0.0 },
            Cell { terrain: Terrain::Grass, elevation: 0.5, moisture: 0.5, vegetation: 0.1, prey_pressure: 0.0, pred_pressure: 0.0, dried_from: None, parasite_load: 0.0 },
            Cell { terrain: Terrain::Grass, elevation: 0.5, moisture: 0.5, vegetation: 0.3, prey_pressure: 0.0, pred_pressure: 0.0, dried_from: None, parasite_load: 0.0 },
        ];
        let world = World {
            cells,
            width: 3,
            height: 1,
            dens: vec![],
            carcasses: vec![],
            seeds: vec![],
            regions: vec![("R".to_string(), 0, 0, 3, 1)],
            water_cells_at_generation: 1,
            shore: vec![],
        };
        let r = ("R".to_string(), 0usize, 0usize, 3usize, 1usize);
        let mean = region_land_veg_mean(&world, &r);
        assert!((mean - 0.2).abs() < 1e-6, "land veg mean {mean} should exclude the water cell");
    }

    #[test]
    fn drought_event_has_region_centre_pos() {
        let mut sim = Sim::new(42, Params::default());
        for c in &mut sim.world.cells {
            if !c.terrain.is_water() {
                c.moisture = 0.0;
            }
        }
        run_days(&mut sim, 8);
        let droughts: Vec<&Event> = sim.events.iter().filter(|e| e.kind == EventKind::Drought).collect();
        assert!(!droughts.is_empty());
        for e in droughts {
            let pos = e.pos.expect("drought event should carry a position");
            let is_centre = sim.world.regions.iter().any(|r| ((r.1 + r.3).div_euclid(2), (r.2 + r.4).div_euclid(2)) == pos);
            assert!(is_centre, "pos {pos:?} is not a region centre");
        }
    }
}
