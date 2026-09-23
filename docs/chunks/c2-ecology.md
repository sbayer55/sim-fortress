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

### FR12 Succession and trampling (2026-09-22)
Plan: [succession-plan.md](../succession-plan.md). Animals change the map: prey traffic
lowers the vegetation target (**trampling**) and a land cell's terrain climbs or wears one
rung of the ladder `Dirt → GrassSparse → Grass → GrassDense → Forest` (**succession**).
Sand, Marsh, Rock and water are never on the ladder; a `dried_from` cell is Sand and so is
excluded; Dirt is the floor; Forest is the ceiling only where `cell.biome.allows_forest()`
(tundra, steppe and desert stop at meadow). No worldgen, grazing, movement, cover or diet
code changes: every consumer of `Terrain` reads it fresh.

1. **Trampling** (step 3): `target = max_vegetation[terrain] × biome.vegetation_scale() ×
   season_cap × min(1, moisture / 0.5) × (1 − trample_w × prey_pressure)`, clamped. Pure
   arithmetic, no draw. `warm_up` sees no creature and is unchanged. Predator traffic is not
   counted.
2. **Two day counters per cell**, `Cell.thrive_days` and `Cell.wear_days` (`u16`, zero at
   generation, not fed to the checksum). Every on-ladder cell is classified once a day in
   sub-step **8b**, after the regrowth sites and before the series sample, with `target` the
   untrampled target step 3 computed and `trampled = target × (1 − trample_w × pressure)`:
   - **thriving** when the cell has a rung to climb into, `moisture ≥ climb_moisture[next]`,
     `vegetation ≥ climb_veg × trampled` and `prey_pressure < trample_low`:
     `thrive_days += 1`, `wear_days = 0`;
   - **worn** when `vegetation < wear_veg × target` and `prey_pressure ≥ trample_high`:
     `wear_days += 1`, `thrive_days = 0`;
   - otherwise both counters fall by `relax_per_day` toward zero.
   Wear needs pressure, so drought alone never wears a cell.
3. **The flip**, in row-major order: a cell with `thrive_days ≥ climb_days[next]` and
   `moisture ≥ climb_moisture[next]` rolls `rng.chance(flip_chance)` on the ecology stream and
   on success climbs one rung; a cell above Dirt with `wear_days ≥ wear_days_needed` rolls the
   same chance and drops one rung. On any flip both counters reset and
   `vegetation = min(vegetation, max_vegetation[new])`; nothing else on the cell changes. The
   roll is skipped entirely while `flip_chance == 0`, so the neutral overlay makes no draw.
4. **Events.** One `Note` per region per day per direction: `Scrub is closing over <region>`
   on the first climb, `Grazing wears <region> back` on the first drop. No new chip.
5. **Series and CSV.** `Sample.forest_cells` and `Sample.bare_cells` (Dirt + Sand), the CSV
   columns `forest_cells,bare_cells` at the end, `--summary` columns `forest_pct` and
   `bare_pct` (shares of the land cells at the end of the run). `bench_c5` takes
   `trample_w`, `flip_chance`, `climb_days_forest`, `wear_days` and `succession=off`.
6. **Screens.** S02k succession overlay (a `Succession` base row on S14, see
   [S02](../screens/s02-map-overlay.md)); the S06 Terrain composition table already lists
   every terrain and starts moving; S01 look mode names the flipped cell.
7. **`[succession]`** (`SuccessionParams`, after `territory`; save `VERSION = 21`):

   | key | default | role |
   |---|---:|---|
   | `trample_w` | 0.25 | weight of `prey_pressure` against the vegetation target (the plan said 0.5; see the result) |
   | `climb_veg` | 0.9 | vegetation ≥ this × today's trampled target counts as thriving |
   | `wear_veg` | 0.5 | vegetation < this × the untrampled target counts as worn |
   | `trample_low` | 0.10 | pressure below this allows thriving |
   | `trample_high` | 0.30 | pressure at or above this allows wear |
   | `relax_per_day` | 1 | counter decay on a day that is neither |
   | `climb_days` | Sparse 60, Grass 90, Dense 120, Forest 360 | thriving days needed to climb *into* each rung |
   | `climb_moisture` | Sparse 0.15, Grass 0.25, Dense 0.35, Forest 0.40 | cell moisture needed to climb *into* each rung |
   | `wear_days_needed` | 45 | worn days needed to drop one rung |
   | `flip_chance` | 0.10 | daily roll once a cell is ripe |

   `trample_w = 0` and `flip_chance = 0` is the **neutral control**: the target is unchanged
   to the bit and no draw is made, so the run is the pre-succession run.
   `ecology::tests::neutral_succession_reproduces_the_old_checksum` pins the pre-FR12 value
   `0xf9ea_eb02_3e27_c085` under it; the territory tripwire now applies both neutral
   overlays and keeps pinning the pre-territory value. The default checksum was re-baselined
   to `0x10cd_7594_03e3_d96c`.

**Result (2026-09-22).** Six seeds, one, two and five years, the planned `trample_w = 0.5`
(`on`) against the neutral overlay (`off`), plus `trample_w = 0.25` (`w25`), which became
the default. End counts vole, hare, deer, fox, wolf, lynx; extinctions; forest and
bare shares of the land.

| seed | run | y1 counts | y1 ext | y1 forest / bare | y2 counts | y2 ext | y2 forest / bare | y5 counts | y5 ext | y5 forest / bare |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | on | 29, 18, 4, 5, 3, 0 | 1 | 19.1 / 18.5 | 1, 11, 1, 0, 0, 0 | 3 | 25.2 / 5.7 | 0, 62, 0, 0, 0, 0 | 5 | 52.0 / 5.5 |
| 1 | w25 | 0, 48, 24, 6, 2, 1 | 1 | 19.1 / 19.8 | | | | 0, 108, 177, 0, 0, 0 | 4 | 32.0 / 10.1 |
| 1 | off | 0, 65, 47, 15, 6, 0 | 2 | 19.1 / 27.9 | 0, 2, 41, 0, 0, 0 | 4 | 19.1 / 27.9 | 0, 0, 256, 0, 0, 0 | 5 | 19.1 / 27.9 |
| 2 | on | 31, 22, 21, 0, 0, 0 | 3 | 18.8 / 17.8 | 0, 1, 10, 0, 0, 0 | 4 | 27.7 / 6.2 | 0, 0, 0, 0, 0, 0 | 6 | 59.5 / 5.2 |
| 2 | w25 | 10, 1, 31, 0, 0, 0 | 3 | 19.1 / 12.4 | | | | 0, 0, 104, 0, 0, 0 | 5 | 45.7 / 7.5 |
| 2 | off | 80, 101, 59, 0, 0, 0 | 3 | 19.1 / 20.6 | 16, 67, 65, 0, 0, 0 | 3 | 19.1 / 20.6 | 0, 13, 134, 0, 0, 0 | 4 | 19.1 / 20.6 |
| 3 | on | 9, 202, 41, 11, 4, 3 | 0 | 17.8 / 23.6 | 0, 190, 22, 8, 1, 0 | 2 | 17.8 / 20.6 | 0, 206, 8, 0, 0, 0 | 4 | 19.9 / 19.5 |
| 3 | w25 | 13, 129, 41, 14, 1, 2 | 0 | 18.5 / 19.9 | | | | 0, 43, 269, 0, 0, 0 | 4 | 16.0 / 21.1 |
| 3 | off | 36, 189, 68, 8, 2, 1 | 0 | 19.0 / 21.2 | 0, 214, 100, 3, 1, 0 | 2 | 19.0 / 21.2 | 0, 162, 175, 0, 0, 0 | 4 | 19.0 / 21.2 |
| 4 | on | 1, 163, 29, 24, 7, 3 | 0 | 18.0 / 29.8 | 0, 31, 45, 6, 0, 0 | 3 | 18.0 / 14.2 | 0, 0, 246, 0, 0, 0 | 5 | 19.4 / 13.8 |
| 4 | w25 | 0, 227, 58, 19, 4, 3 | 1 | 16.6 / 33.8 | | | | 0, 96, 120, 0, 0, 0 | 4 | 16.0 / 27.1 |
| 4 | off | 2, 185, 60, 28, 9, 3 | 0 | 19.1 / 25.2 | 0, 0, 49, 0, 8, 0 | 4 | 19.1 / 25.2 | 0, 0, 311, 0, 0, 0 | 5 | 19.1 / 25.2 |
| 5 | on | 36, 87, 4, 2, 4, 3 | 0 | 19.3 / 21.6 | 20, 103, 0, 1, 1, 1 | 1 | 22.4 / 16.5 | 0, 270, 0, 0, 0, 0 | 5 | 30.7 / 19.0 |
| 5 | w25 | 0, 80, 10, 12, 3, 2 | 1 | 19.1 / 21.4 | | | | 0, 0, 0, 0, 0, 0 | 6 | 62.9 / 5.0 |
| 5 | off | 2, 128, 15, 19, 7, 3 | 0 | 19.4 / 22.6 | 0, 0, 1, 0, 0, 0 | 5 | 19.4 / 22.6 | 0, 0, 0, 0, 0, 0 | 6 | 19.4 / 22.6 |
| 6 | on | 0, 12, 8, 32, 10, 4 | 1 | 15.2 / 46.4 | 0, 0, 0, 0, 0, 0 | 6 | 15.2 / 5.0 | 0, 0, 0, 0, 0, 0 | 6 | 50.7 / 5.0 |
| 6 | w25 | 0, 24, 44, 39, 7, 4 | 1 | 14.8 / 41.7 | | | | 0, 0, 296, 0, 0, 0 | 5 | 9.7 / 25.6 |
| 6 | off | 0, 12, 32, 41, 10, 5 | 1 | 18.9 / 28.0 | 0, 0, 4, 0, 0, 0 | 5 | 18.9 / 28.0 | 0, 0, 227, 0, 0, 0 | 5 | 18.9 / 28.0 |

- **The neutral run is today's run.** The tripwire holds the pre-FR12 checksum exactly, and
  the `off` rows' terrain shares never move from generation (drought's sand aside).
- **The map moves.** Claim met on 6 of 6 seeds: by year five forest or bare share differs
  from the neutral run by at least two points of the land everywhere, and by far more where
  the grazers are gone (forest 50–60 % on seeds 1, 2 and 6, bare down to 5 %). An emptied
  world reforests in about two years from meadow and about four from dirt (the `climb_days`
  ladder sums to 630 thriving days). Seed 42 differs from its neutral twin on 863 cells
  after one year and 2 482 after two.
- **Trampling hits the herds.** Deer are lower at year one on every seed under defaults
  (4 vs 47, 21 vs 59, 41 vs 68, 29 vs 60, 4 vs 15, 8 vs 32), and by year five they are gone
  on seeds 1, 2 and 6 where the neutral run keeps 134–256. Hares are lower on four seeds at
  year one. The cause is the C8 herd-grazing rule: a herd stacks on one cell, `prey_pressure`
  saturates within a few ticks, and the herd's own cells lose half their cap. On seed 1 the
  year's starvation deaths are 345 with trampling against 131 without (102 with succession
  but no trampling); on seed 3 they are 18 against 377, because fewer deer are born. Claim 3
  (starvation not above the neutral run by more than the seed-to-seed spread of 246) holds,
  but the deer decline was the balance cost of the planned weight. At `trample_w = 0.25`
  the deer loss halves at year one (24, 31, 41, 58, 10, 44) and the herds survive to year
  five on five of six seeds (177, 104, 269, 120, 0, 296) with year-five extinctions 28
  against 31 at 0.5 and 29 neutral, while the map still moves (forest 32 %, 46 %, 63 % on
  the emptied seeds, 10–16 % where the deer stayed and grazed it back). **The default was
  set to 0.25** on that read; it does nothing for voles.
- **Voles: the hoped-for result, on some seeds.** Year-one voles 29 vs 0 (seed 1) and 36
  vs 2 (seed 5), worse on seeds 2 and 3 (31 vs 80, 9 vs 36), flat on 4 and 6. Year-two
  extinctions 19 with succession against 23 without; year-five 31 against 29. The C5
  collapse is unchanged: every predator is gone by year five under both conditions.
- **Cost.** Ecology step 0.0196 s per 1 000 ticks against 0.0162 (seed 1, 30 000 ticks),
  under 1 % of the step either way; not recorded in `PERFORMANCE.md`.
- **Follow-ups.** A herd that rotates off trampled ground (the graze score already prefers
  the best cell in sense range, but the C8 herd rule overrides it); `climb_days[Forest]`
  if reforestation should take longer than two years; the rest of the plant brainstorm.

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
- FR12: `sim::params::succession::tests::*`, `sim::world::tests::succession_ladder_walks_both_ways_and_stops_off_it`,
  `sim::ecology::tests::{trampled_cell_grows_toward_a_lower_target, thriving_meadow_climbs_after_climb_days,
  worn_grass_drops_after_wear_days, drought_alone_never_wears, relax_decays_both_counters,
  forest_needs_a_wooded_biome, climb_needs_moisture, sand_marsh_rock_and_water_never_flip,
  one_note_per_region_per_day_per_direction, neutral_succession_reproduces_the_old_checksum}`,
  `ui::screens::s01_map::tests::s02k_succession_overlay_render`,
  `tests/ecology.rs::{ungrazed_meadows_close_into_forest, grazed_ground_wears_and_recovers}`

## Decisions made here
- Vegetation is continuous 0..1 per cell with a seasonal target; the map glyph still comes
  from the terrain type, the overlay from the value.
- ~~Terrain changes only through drought (shallow water ⇄ sand); grazing does not change
  terrain (C3 may revisit).~~ *Revisited 2026-09-22: FR12 lets grazing pressure and its
  absence move a land cell one rung along the dirt-to-forest ladder; drought's rule is
  unchanged and the two never touch the same cell.*
- Rain is per region per day. Within-region moisture therefore homogenises after year 1
  (the S02c overlay becomes eight flat blocks plus a shore halo); if the user wants texture at
  the checkpoint, the agreed extension is `Cell.rain_factor = 0.5 + base_moisture` multiplying
  `rain_amount`, added as a parameterised option in C3.

## Risks
- The constants above were derived by hand; the acceptance ratios decide whether they
  hold. Budget time for tuning inside this chunk.
- `growth_k` and `season_cap` are the levers C3 will pull to feed herbivores.
