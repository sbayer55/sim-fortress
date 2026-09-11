# C2 — Vegetation, water and seasons

Back to the [roadmap](README.md). Previous: [C1](c1-core-and-shell.md). Next: [C3 Herbivores](c3-herbivores.md).

## Goal
Make the landscape alive before anything eats it: vegetation grows toward a seasonal
target and dies back in autumn and winter, moisture reaches a rainfall-dependent
equilibrium, droughts dry shallow water to sand, and the UI shows it through the overlays,
the ecology screen, the event log and the vegetation charts.

## Checkpoint (what the user sees)
Over a simulated year at x25 the vegetation overlay pulses green in spring and summer and
browns in autumn and winter; in a dry world a drought shrinks shallow water to sand and
the event log records it and its end; the ecology screen's region table moves between
Plenty and Scarce; the charts draw the biomass line and shade drought periods.

## Scope

### In
- `sim::ecology`: daily vegetation and moisture update, per-region rain, drought
  detection and recovery, water ⇄ sand, regrowth sites.
- `sim::stats::Series`: daily ring buffer (`params.stats.series_days`, default 720) of the
  fields in FR5.
- Events: `Drought` (start), `DroughtEased` (new kind: glyph `¡` dim, label `eases`, chip
  `droughts`), `Note` (regrowth site sprouted), `Season`.
- **S09 Regrowth rate field** live (maps to `ecology.regrowth_rate`).
- Live screens: **S02a/S02b/S02c** overlays (S02b shows zeros with a `no creatures yet`
  note), **S06 Ecology**, **S07a Event Log** (chips functional, no detail pane),
  **S05a/S05c** (vegetation only), **S01 sidebar Resources section** fully live.
- Headless `--csv path`.

### Out
Creatures, population lines, S07b detail pane, S02d.

## Dependencies
- [C1](c1-core-and-shell.md) complete, in particular FR4 time getters, FR6 screen trait,
  the `lib.rs` layout (needed by `tests/ecology.rs`) and `params.world.rainfall`.
- Screen requirements: [S02](../screens/s02-map-overlay.md), [S06](../screens/s06-ecology.md),
  [S07](../screens/s07-event-log.md), [S05](../screens/s05-population-charts.md), [S01](../screens/s01-world-map.md).
- `widgets::map` overlays (`Overlay::Vegetation/Pressure/Moisture`) reused unchanged.

## Functional requirements

### FR1 Params (`[ecology]`)
```toml
regrowth_rate = 1.0          # user-facing multiplier (S09 "Regrowth rate" field maps here)
growth_k = 0.08              # approach rate toward target per day
dieback_k = 0.06             # decay rate toward target when above it
evap_k = 0.05                # proportional evaporation per day
rain_amount = 0.15
seed_sprout_chance_per_day = 0.02
drought_moisture = 0.20      # region mean below this …
drought_days = 6             # … for this many days flags a drought
drought_recover_margin = 0.10
water_dry_region_moisture = 0.15
water_refill_region_moisture = 0.35
water_changes_per_region_per_day = 3
max_vegetation = { sand = 0.10, dirt = 0.30, grass_sparse = 0.50, grass = 0.80, grass_dense = 1.00, forest = 0.85 }  # water, rock = 0
season_cap        = { spring = 1.0, summer = 1.0, autumn = 0.7, winter = 0.45 }
season_regrowth   = { spring = 1.3, summer = 1.0, autumn = 0.7, winter = 0.5 }
season_evaporation= { spring = 0.8, summer = 1.2, autumn = 0.9, winter = 0.6 }
season_metabolism = { spring = 1.0, summer = 1.0, autumn = 1.1, winter = 1.3 }   # used by C3; shown in S06 now
rain_chance_per_day = { dry = 0.10, normal = 0.25, wet = 0.45 }
[ui.scarcity_thresholds]  scarce = 0.365  strained = 0.40  plenty = 0.45  plenty_min_prey = 8  crowded_prey_per_veg = 30
```
Expected moisture equilibria (rain − evaporation = 0): ≈ 0.30 dry, ≈ 0.75 normal, ≈ 1.0 wet.
The implementer may retune these constants inside this chunk to satisfy the acceptance
criteria and must record the final values here.

### FR2 Daily update
Runs on the tick where `hour() == 0`, in this order (fixed for determinism):
1. **Rain**: for each region in index order, one `rng.chance(rain_chance[rainfall])`; on
   success add `rain_amount` to the moisture of every cell in the region.
2. **Water adjacency**: land cells with any water cell among their 8 neighbours gain 0.01.
3. **Vegetation** for every land cell:
   `target = max_vegetation[terrain] × season_cap[season] × min(1, moisture / 0.5)`;
   if `v < target`: `v += growth_k × regrowth_rate × season_regrowth[season] × (target − v)`
   (regrowth-site cells use `2 × growth_k`); else `v −= dieback_k × (v − target)`.
4. **Evaporation** for every cell including water: `moisture −= evap_k × season_evaporation[season] × moisture`.
5. Clamp moisture and vegetation to 0..1.
6. **Drought detection** per region using the mean of stored `moisture` over **land**
   cells (not water; Rock counts as land). The displayed `region_moist` (water = 1.0) is
   presentation only.
7. **Water ⇄ sand** (FR3).
8. **Regrowth sites** (FR4), one rng call per eligible cell in row-major order.
9. **Series sample** (FR5).

### FR3 Drought and water
A region whose mean moisture stays below `drought_moisture` for `drought_days`
consecutive days is flagged and emits one `Drought` event (`pos` = region centre cell,
`species = None`, text `Drought grips <region>; <n> water cells at risk`). It is unflagged,
with a `DroughtEased` event `Drought eases in <region>`, when the land-cell mean rises above
`drought_moisture + drought_recover_margin`. While flagged and the region land-cell mean is below
`water_dry_region_moisture`, up to `water_changes_per_region_per_day` `ShallowWater` cells
(lowest own moisture first, ties by row-major index) become `Sand` with
`Cell.dried_from = Some(ShallowWater)` and vegetation 0. When a region's land-cell mean exceeds
`water_refill_region_moisture`, up to the same number of `dried_from` cells per day (row-major
order) revert to `ShallowWater` (vegetation 0, marker cleared). `checksum` includes `dried_from` and the
drought flags. `World.water_cells_at_generation` is stored for the water-level series.

### FR4 Regrowth sites
Dirt/GrassSparse cells with vegetation < 0.1 sprout with `seed_sprout_chance_per_day`
into `world.seeds`; a site is removed when its vegetation reaches `0.8 × max_vegetation[terrain]`
(so Dirt sites, capped at 0.30, do clear). At most one
`Note` `A regrowth site sprouted in <region>` per region per day.

### FR5 Series (daily)
`biomass_total`, `veg_mean` (land cells), `water_cells`, `water_level = water_cells /
water_cells_at_generation`, `moisture_mean`, `seeds`, `dens`, `carcasses` (0 until C3),
`drought_regions` (count flagged), `drought_flags[8]` (per-region bool), `region_veg[8]` (land cells), `region_moist[8]` (all
cells, water = 1.0), plus per-species population slots (zero until C3). Getters return
oldest-first slices. Drought *bands* on charts are the days with `drought_regions ≥ 2` (a single flagged region
is common in dry worlds and would shade most of the chart). "Land cells" everywhere in this
chunk = every cell that is not water (Rock included).

### FR6 Overlays (S02a/b/c)
Live cells. Sidebar region table: vegetation over land cells, moisture with water = 1.0
(matches the overlay). S02b draws the pressure overlay from the (zero) pressure fields
and replaces its second description line with `no creatures yet`. `o` cycles vegetation →
pressure → moisture → off; `4` is a no-op with the status hint. Heatmaps ignore the night
and winter tints. The "over N cells" text uses `W × H`.

### FR7 Ecology screen (S06)
All values live. Sunrise/sunset and the hour strip come from `params.time`. Modifier
table columns regrowth / evaporation / metabolism / forage from `params.ecology`; forage
word from `season_cap` (≥ 1.0 lush, ≥ 0.9 drying, ≥ 0.6 fading, else scarce). Region means
over land cells (vegetation) and all cells (moisture). Status rule: Scarce if veg <
`scarce`; Strained if veg < `strained`, or crowded (`prey / veg > crowded_prey_per_veg`)
and veg < `plenty`; Plenty if veg ≥ `plenty` and (the sum of prey `initial_counts` is 0,
or `prey ≥ plenty_min_prey`); otherwise Stable. Drought risk (land-cell means): low if no region is
below `drought_moisture + margin`, moderate if some is, high if any region is flagged.
`frost in N days` = days until Winter, or `frost now`. The `¡` line shows the latest
`Drought`/`DroughtEased` event. Keys: `r` sort (name → vegetation → moisture → status), `↑↓`
select a row, `Enter` centres the map on the region (formula in FR8), `Tab` no-op.

### FR8 Event log (S07a)
Newest first from the ring buffer. `1` = all; pressing `2`–`7` while `all` is active
selects only that kind, further chips add/remove kinds, an empty set reverts to `all`;
`f` cycles all → deaths+extinctions → migrations+droughts → all. Season/Note appear only
under `all`; `DroughtEased` belongs to the `droughts` chip. No detail pane in C2. `Enter` on an event with `pos` pops to the map and
centres the viewport: `origin = (clamp(cx − 55, 0, W − 110), clamp(cy − 20, 0, H − 40))`.

### FR9 Charts (S05a/S05c)
S05a top chart title ` Vegetation (mean biomass, % of max) `, y axis 0–100 % with five
labels, drought bands shaded; bottom chart empty with `no population data yet` centred;
sidebar shows Drought and Legend/Keys sections only; `+/-` zoom 60/240/720 days. S05c
draws axes, the dimmed species legend and the vegetation line; Composition sidebar reads
`no population data yet`.

### FR10 S01 Resources section
Vegetation bar = `veg_mean`, water bar = `water_level`, counts live; in winter, or when any
region is below `strained`, the line `! scarcity: N regions below forage line`.

### FR11 Headless CSV
Header `day,biomass_total,veg_mean,water_cells,water_level,moisture_mean,seeds,drought_regions,
veg_<region>×8,moist_<region>×8`, one row per sampled day (`--ticks 17280` → 720 rows).

## Acceptance criteria
- Seed 42, default params, 720 days headless: `veg_mean` summer maximum ÷ winter minimum
  in year 2 is between 1.4 and 3.0 (the model's structural value is ≈ 1/season_cap.winter ≈ 2.2); no cell leaves 0..1; a Dry-rainfall world has ≥ 1
  Drought event in 2 years and a Wet world has none (tested on seeds 1..=5).
- Determinism test green with ecology on; `daily_update_is_deterministic` (two sims, 720
  days, equal checksums).
- Region means exclude water for vegetation; Drought events carry the region centre.
- Screens S02a/c, S06, S07a match their prototypes in panel structure (flip with
  `--prototypes S06a`).
- Headless 720 days at 150×40 in under 2 s.

## Checkpoint demo script
1. Generate the default world, `1` vegetation overlay, x25 (one year ≈ 2.9 min). The
   overlay greens through spring/summer and browns through autumn/winter.
2. `3` moisture overlay; `y` ecology: region statuses change across seasons; `Esc`.
3. Regenerate with Rainfall = dry; within two years the ticker shows `Drought grips …`;
   `e` opens the log; select it; `Enter` centres the map; sand patches appear in the
   region's shallow water; later `Drought eases in …` appears under the `droughts` chip.
4. `g` charts: biomass line with a shaded drought band; `3` stacked view shows only the
   vegetation line.
5. `cargo run -- --headless --seed 42 --ticks 17280 --csv out.csv`: 720 data rows.

## Tests
- `sim::ecology::tests::{growth_saturates_at_season_target, winter_lowers_equilibrium,
  moisture_equilibria_by_rainfall, drought_flags_after_n_days_and_recovers,
  water_dries_and_refills_with_marker (region containing water cells), seeds_sprout_and_clear_on_dirt, one_seed_note_per_region_per_day,
  region_means_exclude_water, drought_event_has_region_centre_pos}`
- `sim::stats::tests::{series_ring_buffer, daily_sampling_fields}`
- `ui::tests::{s07_chip_sets, s06_status_rule}`
- `tests/ecology.rs::{yearly_cycle_ratio, dry_world_has_drought, wet_world_has_none,
  daily_update_is_deterministic, csv_row_count}`

## Decisions made here
- Vegetation is continuous 0..1 per cell with a seasonal target; the map glyph still comes
  from the terrain type, the overlay from the value.
- Terrain changes only through drought (shallow water ⇄ sand); grazing does not change
  terrain (C3 may revisit).
- Rain is per region per day. Within-region moisture therefore homogenises after year 1
  (the S02c overlay becomes eight flat blocks plus a shore halo); if the user wants texture at
  the checkpoint, the agreed extension is `Cell.rain_factor = 0.5 + base_moisture` multiplying
  `rain_amount`, added as a parameterised option in C3.

## Risks
- The constants above were derived by hand; the acceptance ratios decide whether they
  hold. Budget time for tuning inside this chunk.
- `growth_k` and `season_cap` are the levers C3 will pull to feed herbivores.
