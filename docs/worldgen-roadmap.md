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
| 2 | Marsh, riparian, shore types | 1 | `Terrain::Marsh` | yes |
| 3 | Biomes as regions | 1, 2 | `Cell.biome`, watershed regions | yes |
| 4 | River morphology | 2 | none | no |
| 5 | Age regimes + events | — (better after 3) | `World.history` | yes |
| 6 | Names, log, summary | 3, 4, 5 | `World.names` | yes |

Phases 4 and 5 are independent of each other and could be built in parallel
branches once 1–3 are in.
