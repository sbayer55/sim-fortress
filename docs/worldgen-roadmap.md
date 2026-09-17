# World generation roadmap

**Goal.** Make the initial world read as a real place, the way a Dwarf Fortress
world does: coherent climate, biomes with edges, rivers that behave like rivers,
and a landscape that shows its history. Phases are ordered by payoff and each one
ships on its own. Later phases build on the fields earlier ones introduce.

**Starting point** (Sept 2026, `src/sim/world/`): `relief.rs` builds a tectonic
height field and ages it with `age` erosion epochs; `flow.rs` routes drainage
(D8, priority flood); `classify.rs` picks ocean, lake, river, then rock, sand,
forest and four grass bands by quantile from a moisture field that mixes a free
noise rain field, water distance and altitude. That already covers DF's elevation
and drainage stages. What is missing is temperature, wind, coherent biomes, river
morphology, and a legible history.

**Standing constraints for every phase.**

- S09 live preview: 200×60 stays under ~16 ms in the dev profile.
- Determinism: all new randomness comes from the generation `Rng`; changed cells
  re-baseline `checksum_is_fnv_stable` and nothing else.
- Any new `WorldParams` field or new `Cell` field bumps save `VERSION` (postcard
  is not self-describing).
- The percentage targets (`water_pct`, `forest_pct`, `rock_pct`) keep holding on
  every seed; `target_percentages_20_seeds` stays green.
- CP437 only, fixed 155×45 viewport: every new terrain needs a glyph and a colour
  that reads at a glance in `theme.rs` / `glyphs.rs`.

---

## Phase 1 — Climate physics: temperature and orographic rain

**Status: done (Sept 2026).** `src/sim/world/climate.rs` holds the wind draw,
the parcel sweep and the temperature field; `Relief` carries `rain`,
`temperature` and `wind`; `Cell.temperature` and `World.wind` are saved
(format version 6). The wind is drawn from the seed rather than exposed as a
parameter and is shown in the S09 preview hint; the look cursor shows a cell's
temperature. Dev-profile generation of 200×60 measured ~15.5 ms, up from
~14 ms. Tests: `rain_shadows_lie_downwind`, `temperature_falls_with_height_and_latitude`,
`rainfall_setting_scales_moisture`, plus the ignored diagnostic
`print_climate_bands` for retuning.

**Payoff.** Highest. Every later phase keys off these two fields, and they turn
"noise that happens to be dry" into "dry because it is behind that ridge".

**Deliverables.**

1. `Relief.temperature: Vec<f32>` (0 cold .. 1 hot). Latitude gradient down the
   rows (north cold, south warm, or reversed per seed) minus a lapse rate with
   height, plus a small seeded wobble so the isotherms are not straight lines.
2. Replace the free fbm rain field with an orographic sweep. Pick a prevailing
   wind per seed (one of the four cardinals, biased west→east). March air parcels
   across the grid in wind order carrying a moisture budget: pick up moisture
   over ocean and lakes, rain out proportionally when the surface rises, leave a
   shadow behind ridges. Blend with a little fbm so plains are not uniform.
3. `moisture()` in `classify.rs` uses the new rain field; the `Rainfall`
   dry/normal/wet bias becomes a scale on the parcel budget rather than a flat
   offset, so a dry world still has a wet windward coast.
4. Expose `wind` (or derive it from the seed) and show it in the S09 preview
   header.

**Acceptance.** Rain shadows lie on the lee side of the highest ridges on ≥ 18 of
20 seeds (test: mean rain on lee cells of the top-slope ridge < windward mean).
Temperature falls with height (correlation test). Preview budget unchanged.

**Files.** `relief.rs` (tectonics, new `climate` pass), `classify.rs`, `params/
world.rs` if wind becomes a param, `tests.rs`.

---

## Phase 2 — Wetlands, riparian corridors and shore types

**Status: done (Sept 2026).** `Terrain::Marsh` (code 9, glyph `≡`) is cut in
`classify::land_terrain` from the bottom slope quintile of the land with moisture
≥ 0.55 and either a catchment of ≥ 4 cells or a lake rim, capped at 5% of cells
(3–5% in practice). `classify::riparian` adds a moisture bonus one cell out from
rivers draining ≥ 24 cells (two cells out for trunks ≥ 80), so the forest and
grass quantiles follow the banks. Sand is ocean shore only. Marsh is a drinking
spot (`World::compute_shore`), costs `creatures.marsh_step_cost` extra move
budget per step, has cover 1.0 and vegetation cap 0.9. `vegetation()` now scales
by moisture and temperature, centred so the mid-range world is unchanged. Save
format version 7. Dev-profile 200×60 generation still ~15.5 ms. Tests:
`marsh_lies_on_flat_wet_land`, `rivers_carry_riparian_bands`. Ecology tests that
strip bare cells did not need Marsh added.

**Payoff.** High and cheap: everything needed is already computed (slope, acc,
moisture, water kind). This is the phase that makes rivers *matter* to the map.

**Deliverables.**

1. New terrain `Marsh`: land cells with low slope, high accumulation and high
   moisture, and the flat rims of lakes. Marsh is drinkable shore, slow to cross,
   high vegetation, and a natural refuge (see the C5 balance findings on fox/vole
   refuges).
2. Riparian corridors: cells within 1–2 of a river with accumulation above a
   threshold are promoted one moisture band (GrassSparse → Grass, Grass →
   GrassDense) and count toward `forest_pct` first, so dry basins get gallery
   forest along the trunk river.
3. Shore types: ocean shore keeps sand; lake shore becomes mud/marsh instead of
   sand, so `SAND_SHARE` applies to ocean only.
4. Vegetation seed: `vegetation()` initial biomass reads moisture and
   temperature, not just terrain and noise.

**Acceptance.** Marsh appears only on land with slope in the bottom quintile.
Rivers on a dry seed are flanked by a visibly greener band. `forest_pct` and
`water_pct` targets still hold; ecology tests that need bare `Dirt` still pass
(strip Marsh too where they strip GrassSparse).

**Files.** `world.rs` (`Terrain`, glyph/colour), `classify.rs`, `ecology.rs`
(movement/drink rules for Marsh), `creatures.rs` placement, save `VERSION`.

---

## Phase 3 — Biomes as regions

**Status: done (Sept 2026).** `src/sim/world/biome.rs` holds the Whittaker table
(`Biome::classify`, cuts on the land climate deciles: `COLD` 0.18, `HOT` 0.50),
two 3×3 majority passes and a small-patch pass that absorbs any 4-connected
patch under `MIN_PATCH` (4) cells. `Cell.biome` is saved (format version 8);
the forest quantile skips tundra, steppe and desert (falling back to the
wettest remainder so `forest_pct` still holds) and `Biome::vegetation_scale`
(0.7–1.0) multiplies the initial biomass and the ecology cap. `src/sim/world/
regions.rs` replaces the fixed rectangles: every cell's flow-tree root is a
basin (ocean cells join the nearest land basin), basins merge smallest-first
into the neighbour sharing the longest border until eight remain, a neighbour
over `GROWTH_CAP` (25 % of the world) only takes basins with nowhere else to
go, and each region is named `<Northern|Southern|Eastern|Western|Central>
<biome noun>` (≤ 16 cells, `Coast` when mostly water, `Upper`/`Lower` or a
numeral to break ties). `World.region_map` carries the per-cell index;
`RegionRect` is now a bounding box and every consumer goes through
`region_index`, `region_cells`, `region_centre`, `regions_adjacent` and
`region_size`. Rendering: `map::terrain_cell` blends land colours toward
`theme::BIOME` (0.24 fg, 0.12 bg); S09 shows the region names on the preview
and a `biomes` share line. Dev-profile 200×60 generation measured ~17.6 ms
(relief 12.6, classify 3.0 of which the biome pass is 1.1, regions 1.6–2.7),
about 1.5 ms over the 16 ms note; the preview still feels live. Tests:
`regions_cover_world` (eight regions, boxes bound their cells, centres lie
inside), `regions_are_contiguous_and_named`, `biomes_follow_climate_in_patches`,
plus the ignored diagnostic `print_biomes_and_regions` (shares, sizes, names,
climate deciles, stage timings). Checksum re-baselined to
`0xb4b5_fbec_d0ad_4ff3`.

**Payoff.** High visual payoff: the map stops being a per-cell speckle and reads as
"the northern taiga", "the dry steppe", with edges you can point at.

**Deliverables.**

1. A Whittaker-style table `biome(temperature, moisture) -> Biome` with roughly
   eight entries: tundra, taiga, temperate forest, temperate grassland, steppe,
   savanna, desert, wetland. Biome is stored per cell (`Cell.biome`) and drives
   which terrain the quantile passes may pick (taiga forest vs temperate forest
   share the `Forest` terrain but differ in colour and vegetation cap).
2. Coherence: a majority filter over the biome labels (two passes, radius 1)
   removes single-cell islands, then reassign `World.regions` from watersheds
   in the flow tree instead of the fixed rect grid, so region boundaries follow
   ridgelines.
3. Biome-tinted rendering: terrain glyph stays, colour is shaded by biome (cold
   forest bluer, savanna grass yellower). Keep the tint subtle enough that
   terrain is still legible.
4. Biome shares in the S09 summary line.

**Acceptance.** No biome patch smaller than 4 cells after filtering. Region count
stays within the range the stats screens expect (`regions_cover_world`). The
S09 preview still under budget with the two filter passes (measure; they are
O(n) with a 3×3 kernel).

**Files.** new `world/biome.rs`, `classify.rs`, `world.rs`, `theme.rs`,
`stats/` region series, `s09_worldgen.rs`.

---

## Phase 4 — River morphology

**Status: done (Sept 2026).** `classify::water_bodies` now returns a
`Bodies` layout (water kind, `Channel`, `Delta`, the fall slope cut) and
`Relief` carries `recv` (each cell's receiver) so a channel knows which way
it flows. Rivers are cut largest drainage first: `flow::Tiers` sets the
brook / river / trunk cuts per world (trunk ≥ 9 % of the cells' drainage
or the eighth-largest channel drainage, whichever is less, so every seed has
a trunk of ≥ `TRUNK_MIN_CELLS` (8) cells; river ≥ 4.5 % of the cells or
half the trunk cut); a river takes one bank cell
across the flow (the lower side), a trunk two, and a trunk's channel is
`DeepWater`. Banks are paid from the river budget, which rose from 15 % to
22 % of the water target (ocean 12 % → 10 %). `DELTA_SHARE` (15 %) of the
budget is held back for deltas: where a trunk's receiver is ocean or a lake
of ≥ 12 cells and the land within (2, 1) of the mouth has mean slope in the
bottom quintile, land within `FAN_RISE` (0.015) of the mouth becomes shallow
`Channel::Fan`, land up to `BAR_RISE` (0.03) a sand bar, and flat land up to
`PLAIN_RISE` (0.05) within (4, 2) is floodplain that the marsh pass takes
first. Deltas form on about half of the default seeds (fans of 4–11 cells).
Falls: channel cells in the top 5 % of channel slopes (`FALL_SLOPE_QUANTILE`)
keep `Rock` and are listed in `World.falls` (sorted `(x, y)`; `is_fall`,
`terrain_name` → "waterfall"), drawn by `map::world_cell` as `≡` on the
deep-water blue (`FALLS_FG`/`FALLS_BG`), in the legend and help; 8–12 per
seed. Washes: a brook whose rain-only climate (moisture without the
water-proximity term) is desert or steppe is `Channel::Wash`: generated as
`Sand` with `dried_from = Some(ShallowWater)`, free of the river budget, and
not a moisture source, so the country around it stays visibly arid; the
existing re-wet rule refills them (30–180 cells on a Dry seed, 10–105 on
Normal, all running again well inside the first year). Save format version
9 (`World.falls`). Dev-profile 200×60 generation measured ~18.7 ms (classify
3.8 ms, up ~0.8 ms for the channel walk and delta pass), 150×40 ~9.5 ms. Tests:
`trunks_are_deep_banked_and_continuous`, `deltas_fan_where_trunks_meet_the_sea`,
`falls_sit_on_the_steepest_channels`, `washes_are_dry_brooks_that_rewet_within_the_year`,
plus the ignored diagnostic `print_river_morphology`. Checksum re-baselined
to `0xb6e9_09b8_a4c6_2f17`. Two notes against the
acceptance below: the trunk is the largest-drainage reach, and that is what
the test pins; "longest connected water path" is not an invariant of this
generator (lakes break paths and a long thin basin can out-run a broad one),
and it was dropped rather than forced. With the current ecology constants
a Dry region's mean moisture sits above the refill threshold for most of the
year, so washes refill early and rarely dry again; making the dry-out
seasonal is ecology work, not worldgen.

**Payoff.** Medium-high. Rivers already route correctly; this phase makes them
look and behave like rivers rather than one-cell channels.

**Deliverables.**

1. Width from accumulation: three tiers by drainage area — brook (shallow, no
   widening), river (shallow, one cell), trunk (deep centre, shallow banks). All
   still drawn from `RIVER_SHARE` of the water budget so `water_pct` holds.
2. Deltas and floodplains: where a trunk river reaches ocean or a large lake with
   slope in the bottom decile, fan the last few cells into shallow water and sand
   bars; the surrounding low cells become Marsh (Phase 2).
3. Seasonal washes: brooks in dry biomes are generated as `Dirt`/`Sand` with
   `dried_from = Some(ShallowWater)` so the existing drought/re-wet logic in
   `ecology.rs` refills them in the wet season.
4. Gorges and falls: a river crossing a `Rock` cell with slope in the top decile
   is marked as a fall (cosmetic glyph, e.g. `≡` on deep blue) and the rock is
   kept rather than displaced.

**Acceptance.** Trunk rivers are the longest connected water paths on every seed.
`water_forms_bodies_not_speckle` stays green. Wash cells re-wet in the first wet
season on a Dry seed (test on a fixed seed).

**Files.** `classify.rs` (`water_bodies`, `water_terrain`), `flow.rs` (strahler
or area tiers), `ecology.rs`, `glyphs.rs`.

---

## Phase 5 — History: aging regimes and geological events

**Status: done (Sept 2026).** `relief::AgeRegime` (`for_age`, `name`) replaces
the flat erosion constants: young (age ≤ 3) cuts at `incision_k` 0.11 with
diffusion 0.06, mature (4–11) keeps today's 0.09 / 0.12, old (age ≥ 12) cuts at
0.06, diffuses at 0.20 and every fourth epoch silts each cell halfway up to the
depression-filled surface (`silt`), so basins become plains. The regime name
sits in the S09 preview hint beside the wind. `src/sim/world/history.rs` draws
one to three events per world before the first epoch (count, kind with at most
one glaciation, epoch `0..=age`, centre at least 6 columns and 3 rows inside
the map, scarp half-length `w/6..w/3` over the best of six candidate lines by
land crossed, dome radius 3–6, one warp noise) and strikes each at the top of
its epoch (`epoch == age` strikes after the loop, so an age-0 world still has
events). Fault scarp: a tanh ramp of 0.12 over width 1.5 across a warped line,
tapering past the half-length. Volcanic dome: a cone of 0.15 whose inner half
radius is `Relief.bedrock`. Glaciation: the top quarter of the cold half's
land by height is iced, smoothed four passes, valley floors draining ≥ 8 cells
scoured 0.02, ice-margin cells banked 0.012 as moraine, the top 2 % bared.
`classify::land_terrain` cuts bedrock land as `Rock` first, inside the
`rock_pct` budget, so a dome's core stays rock after thirty epochs and the
rock target still holds. `World.history: Vec<HistoryEvent>` (kind, epoch, x,
y, extent, angle) is saved, format version 11 (10 went to the Mutability trait on main); `HistoryEvent::describe` gives
the sentence Phase 6 will log and `examples/dump_world.rs` prints. Ecological
pre-history: `ecology::warm_up` runs `ecology.warm_up_days` (60) days of
vegetation growth with no rain, evaporation or draws from `Sim::new`, using
the starting season, so founders land on biomass at carrying capacity; the
S09 preview never pays for it. Two latent generator faults surfaced and were
fixed: basins from 8-neighbour routing could hang together by a diagonal, so
`regions::seed_labels` now splits every label into 4-connected pieces; a lone
channel head on the map edge could be a fall with no river beside it, so a
fall needs a water neighbour. Dev-profile 200×60 generation measured 19.7 ms
at age 8 (from 18.5; the glaciation's route and smoothing passes and the
region split), 14.4 ms at age 2 and 36 ms at age 20 (twenty epochs plus five
silt fills); the default-age preview still feels live, an old world less so.
Tests: `age_erodes_the_relief` extended (age 2 vs 14 on the same seed, both
with history), `events_leave_traces` (1–3 events in epoch order on 20 seeds,
bedrock is rock, rock share rises inside every dome, every glaciation bares
something, a scarp struck in the second half of the epochs still steps ≥ 0.02
between its sides), the `history::tests` unit checks on synthetic surfaces
(dome cone and core, scarp step across not along, glaciation scours and dams,
smoothing flattens the mask), plus the ignored diagnostic `print_history`
(events per seed, generation time per age). Checksum re-baselined to
`0xd0e3_ee1a_c665_f531`. `ecology::tests::moisture_equilibria_by_rainfall`'s
seed-42 dry cut widened from 0.60 to 0.65 (measured 0.6016; the ordering
checks are untouched). Deviations from the acceptance below: an early scarp
is deliberately worn faint by the epochs after it, so only late scarps are
pinned; the warm-up is final-generation only.

**Payoff.** Medium. This is DF's "years of history" step. It makes `age` a story
rather than an epoch count and gives young and old worlds distinct characters.

**Deliverables.**

1. Age regimes: young (age ≤ 3) runs incision with reduced diffusion, keeps sharp
   ridges and few lakes; mature runs as today; old (age ≥ 12) raises diffusion,
   lowers `INCISION_K`, and fills more depressions so peneplains and broad
   valleys appear. Constants move into an `AgeRegime` table.
2. Events drawn during the epoch loop, one to three per world, each recorded in
   a `Vec<HistoryEvent>` on `World`: fault scarp (a warped line of uplift),
   volcanic dome (a radial bump that becomes rock), glaciation (cold-quadrant
   scouring: widen high-altitude valleys, deposit moraine lakes, bare rock). Each
   event happens at a random epoch so later epochs partly erode it.
3. Ecological pre-history: after cells exist, run a short vegetation-only warm
   up (a few hundred ticks of growth and drying with no creatures) so initial
   biomass reflects real carrying capacity. Must fit the preview budget or run
   only on final generation, not in the live preview.
4. Save `HistoryEvent` list with the world.

**Acceptance.** `age_erodes_the_relief` extended to show old worlds have lower
mean slope than young ones on the same seed. Every event leaves a measurable
trace (e.g. rock share rises inside a volcanic dome's radius). Determinism test
unchanged.

**Files.** `relief.rs` (regime table, events), new `world/history.rs`,
`world.rs`, `save.rs`, `tests.rs`.

---

## Phase 6 — Naming, generation log and world summary

**Status: done (Sept 2026).** `src/sim/world/names.rs` (with `names/lexicon.rs`
and `names/chronicle.rs`) names the ocean (one feature over every ocean cell),
the 8-connected lakes of ≥ `LAKE_MIN_CELLS` (6) cells (largest first, at most
`LAKE_CAP` 8), the rivers (from every trunk or river-tier mouth, the stem
climbs the largest-drainage channel donor; river-tier donors that join a stem
are tributaries, walked one level deep, largest first, up to `RIVER_CAP` 12
named rivers in all; a trunk is always named, a river-tier stem or tributary
needs `STEM_MIN_CELLS` 4; banks and fans borrow their channel's name) and the
rock ranges (8-connected `Rock` clusters of ≥ `RANGE_MIN_CELLS` 8, at most
`RANGE_CAP` 6; `Fells` when tundra/taiga hold a majority of the cluster,
`Mountains` when its mean height is at or above the mean of all rock,
`Hills` otherwise). `classify::cells` now returns the `Bodies` layout and
`flow::Donors` inverts `recv` so the tree can be walked upstream. Names are
drawn from their own stream (`seed ^ NAME_SALT`), so the checksum is unchanged
(`0xd0e3_ee1a_c665_f531`) and names do not move when an earlier stage changes
its draws: one of three syllable styles per world (hard northern, soft
southern, western with double letters), stems of 2–3 syllables and 3–8 letters
(after a coda the next onset is empty or simple), unique per world, templates
`the <Stem> Sea` / `Lake <Stem>` / `River <Stem>` or `the <Stem>water` /
`<Stem> Brook` or `<Stem> Water` / `the <Stem> Hills|Mountains|Fells`, every
name ≤ `NAME_FULL_MAX` (22) chars. `World.names: Names { style, features,
map }` carries each `Feature` (kind, name, cells, anchor, source, mouth,
parent) and a per-cell feature index; save format version 12.
`World::feature_at`, `feature_near` (the cell, else a water neighbour, else a
range), `place_name` ("by Lake Ulmar" / "in the Tavos Fells" / "in Northern
Taiga"). Region names are unchanged (compass + biome noun; the 16-cell
sidebar columns keep working). `chronicle(world, age)` derives ≤ 7 lines of
≤ 85 chars: wind and age regime, one per history event naming the range (for
ice, also the lake) with the most cells in the event's window else the region,
the largest river's course from its source region to the sea, lake or edge it
reaches (or "winds through" when it sinks into a hollow too small to name), a
delta line when ≥ 3 marsh cells lie within (4, 2) of that mouth, and the
largest lake; `summary(world)` gives coast length (land 8-adjacent to the
ocean), the longest named river, the highest cell and its range or region, and
the feature counts. Both sit under the biome line on S09 (`World` and
`Chronicle` sections, no scrolling). Positioned events use `place_name`
(deaths, births, den sites); region-level events keep the region. The S01c
look sidebar names the feature under or beside the cursor with its kind and
size (and now shows the biome); the S01e goal line of a drinking creature names
the water it heads for; the S02e region overlay draws feature labels in plain
text under the region names (`widgets/map/labels.rs`), skipping any that would
cross a region label. `examples/dump_world.rs` prints the features and the
chronicle. Naming costs ~0.3 ms at 200×60; dev-profile generation measured
~21 ms at age 8 (release 12 ms). Tests: `names_are_stable_per_seed`,
`every_trunk_and_big_lake_is_named`, `names_fit_their_caps`,
`chronicle_fits_the_layout`, `summary_is_consistent`,
`place_names_prefer_water_beside_a_cell`, the lexicon unit checks, the
ignored diagnostic `print_names`, an S09 render test, a look-sidebar test, an
overlay-label test and `widgets::map::labels` tests. Deviations from the
deliverables below: regions keep their descriptive names (the user's call, so
every 16-wide column and the region tests stand); no drink event was added
(drinking is silent and would flood the log), so "a fox drinks at Lake Ulmar"
is carried by the follow sidebar and by the place names in the existing
events; the log is narrated at the end of generation rather than stage by
stage, because the preview regenerates whole in one frame.

**Payoff.** Medium for realism, high for feel. DF's legends make the world feel
inhabited before anything moves. Depends on Phases 3–5 for things to name.

**Deliverables.**

1. Name generator seeded from the world seed: syllable tables, one style per
   world. Names for the ocean, each lake above a size threshold, each trunk
   river and its main tributaries (walk the flow tree), mountain ranges
   (connected rock clusters), and biome regions.
2. A generation log the S09 screen narrates stage by stage: "The Greywater cuts
   the eastern hills", "Ice scours the northern range", "Marshes form where the
   Greywater meets the sea". Entries come from `HistoryEvent` and the naming
   pass.
3. World summary card: coast length, longest river, highest peak, biome shares,
   named features. Shown at the end of S09 and on the inspector when hovering a
   named feature.
4. Ticker and inspector reference features by name ("a fox drinks at Lake
   Ulmar") where a name exists.

**Acceptance.** Names are stable per seed. Every trunk river and every lake over
the threshold has a name. Log fits the S09 layout without scrolling.

**Files.** new `world/names.rs`, `world.rs`, `s09_worldgen.rs`, inspector and
ticker screens, `save.rs`.

---

## Later / optional

- Salinity: brackish estuaries and salt marsh at deltas; freshwater preference
  for drinking.
- Snow line: seasonal snow cover on cold high cells as a render tint tied to
  temperature and `Season`.
- Frozen shallow water in winter in cold biomes, blocking drinking.
- Karst and springs: rivers that start from a spring cell on rock rather than
  from accumulated rain.
- Preset expansion in S09 (`PRESETS` ordering is load-bearing): "young alpine",
  "old floodplain", "cold taiga", each a bundle of age, rainfall, wind and
  latitude direction.

## Sequencing summary

| Phase | Theme | Depends on | New fields | Save bump |
|---|---|---|---|---|
| 1 | Temperature + orographic rain (done) | — | `Cell.temperature`, `World.wind` | yes (v6) |
| 2 | Marsh, riparian, shore types (done) | 1 | `Terrain::Marsh`, `creatures.marsh_step_cost` | yes (v7) |
| 3 | Biomes as regions (done) | 1, 2 | `Cell.biome`, `World.region_map` | yes (v8) |
| 4 | River morphology (done) | 2 | `World.falls` | yes (v9) |
| 5 | Age regimes + events (done) | — (better after 3) | `World.history`, `ecology.warm_up_days` | yes (v11) |
| 6 | Names, log, summary (done) | 3, 4, 5 | `World.names` | yes (v12) |

Every phase has shipped; what remains is the optional list above.
