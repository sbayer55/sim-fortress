# Succession and trampling — implementation plan

*Written against `main` at 21cb12d (the animal and dynasty naming merge, PR #32) on
2026-09-22. Save format `VERSION = 20`, checksum `0xf9ea_eb02_3e27_c085`, thirteen genome
slots.*

*Implemented 2026-09-22 on this branch. Deviations from the text below: the moisture gate
applies at the flip roll as well as in the classification (a cell whose counter ripened
before a drought does not climb into it); `Cell::next_rung` lives on the cell, since the
S02k overlay needs it too; the S02k field is signed and one `Base::Succession` row was
added (the S14 tab had the room); `--summary` reports `forest_pct` and `bare_pct` as shares
of the land, and the acceptance test `grazed_ground_wears_and_recovers` asserts on bare
cells and the two note kinds rather than a margin; `trample_w` defaults to 0.25, not 0.5,
after the sweep (see Result). Checksum re-baselined to
`0x10cd_7594_03e3_d96c`, save `VERSION = 22 (21 on main is quirks; both landed the same day)`; the succession-neutral tripwire pins the old
value and the territory tripwire now applies both neutral overlays. Results are in
[C2 FR12](chunks/c2-ecology.md#fr12-succession-and-trampling-2026-09-22) and summarised at
the end of this document.*

**Goal.** Let animals change the map. Two mechanics from the plant brainstorm in
[feature-ideas.md](feature-ideas.md#plants-that-animals-can-change): **succession**, where a
land cell's terrain climbs the ladder Dirt → sparse grass → grassland → meadow → forest when
it stays lush and lightly used for long enough, and wears back down it when it is grazed
bare and trodden; and **trampling**, where sustained prey traffic lowers a cell's vegetation
cap, so herds wear trails and the ground around water holes and dens degrades outward in
rings. The expected visible effect is the terrain composition on S06 drifting over years,
regrowth fronts and worn patches on the map, and a first read on the C5 question this
attacks from the habitat side: whether meadows closing into forest give voles the cover the
predator chunk's status note says they lack.

**Non-goals.** No new genome slot. No plant layers, plant roster, seed rain, mast, fire or
plant genetics: every one of those is parked in the brainstorm and builds on the cell state
this slice adds. No change to grazing, movement, cover or diet code: those read `terrain`
and pick up a flip for free. Sand, marsh, rock and water never change (drought's shallow
water ⇄ sand rule is untouched and stays the only other terrain change). No worldgen
change: the initial map is what it was. No save migration: v20 files are refused like every
earlier bump. No new event kind: the S07 chip row is full (the territory slice already
shares a chip), so succession speaks through `Note` events like regrowth sites do.

**Starting point.** Four existing pieces carry the weight.
- `ecology::step_vegetation` already computes a per-cell daily `target` from the terrain
  cap, biome scale, season cap and moisture. Trampling is one more factor in that product.
- `Cell.prey_pressure` is the traffic signal: `behavior::death::pressure` adds
  `pressure_per_creature_tick` (0.02) per prey per tick, capped at 1, and `day_boundary`
  multiplies it by `pressure_decay_per_day` (0.85). One grazer parked on a cell all day
  puts it near 0.5; a trail cell sits at 0.1–0.3; untouched ground decays to nothing in a
  fortnight. No new field is needed to know where the herds are.
- `step_regrowth_sites` is the pattern for a per-cell daily roll: one `rng.chance` per
  eligible cell in row-major order, one `Note` per region per day. Succession flips follow
  it exactly, placed after it so no existing draw moves position within a day.
- Every consumer of `Terrain` reads it fresh: `cover_by_terrain` (predation),
  `terrain_position` (diet), `max_vegetation` (ecology), `maybe_make_den` (dens on Dirt,
  meadow and forest), the palette and S06's composition table. A flipped cell changes what
  grows there, who can eat it, how well prey hide on it and how it is drawn, with no code in
  any of those places. The worldgen classifier's `allows_forest` biome rule is reused as the
  ceiling of the ladder.

**Standing constraints.** Everything in `AGENTS.md`. The ones that bite here: no `HashMap`
under `src/sim` (the tables are `BTreeMap<Terrain, _>` like `max_vegetation`); the step
order is fixed and RNG draws are never reordered, so the new draws sit in a new sub-step
after every existing draw of the day; every tunable has a `FIELD_DOCS` entry; the 800-line
ceiling (`ecology.rs` is at 594 with its tests inline, so it becomes a façade before any
code is added); `Cell` has no `Default` and is built by hand in eight files, every one of
which gains the two new fields; and the checksum hashes `terrain` and `vegetation`, so
trampling moves it on day one and the neutral overlay must be draw-free and
arithmetic-free to reproduce the old value.

---

## Design decisions

**D1 — The ladder, and who is on it.** Five rungs in `Terrain` code order:

```
Dirt (3) → GrassSparse (4) → Grass (5) → GrassDense (6) → Forest (7)
```

A cell is *on the ladder* when its terrain is one of those five. Sand (the drought marker
and delta bars), Marsh, Rock and both waters are never touched, and a cell with `dried_from`
set is Sand so it is excluded by construction and stays refillable. Dirt is the floor:
grazing never makes sand, because Sand means "was water" to `step_water_sand`. Forest is
the ceiling only where `cell.biome.allows_forest()`; on tundra, steppe and desert the
ladder stops at meadow, which is the same rule the classifier used to place forest at
generation. No cell is ever added to or removed from a region, a feature or a name:
`names.rs` keys features on Rock and water only.

**D2 — Two day counters per cell, integers, no float drift.**

```rust
pub struct Cell {
    /* … */
    /// Consecutive-ish days this cell has been lush and lightly used (D3).
    pub thrive_days: u16,
    /// Consecutive-ish days this cell has been grazed bare and trodden (D3).
    pub wear_days: u16,
}
```

Each day, after the vegetation step has set today's `target` (D4), every on-ladder cell is
classified once:
- **thriving** when `vegetation ≥ climb_veg × target` and `prey_pressure < trample_low`:
  `thrive_days += 1`, `wear_days = 0`;
- **worn** when `vegetation < wear_veg × untrampled target` and
  `prey_pressure ≥ trample_high`: `wear_days += 1`, `thrive_days = 0`;
- otherwise both counters fall by `relax_per_day` toward zero.
"Consecutive-ish" is the relax rule: a run of thriving days survives a short interruption
but not a season of neither. Using the *seasonal* target rather than the terrain cap means
a winter meadow at its winter target still counts as thriving, so a climb accumulates across
the year; using the *untrampled* target for wear means a cell the herd has trodden down
(D4) reads as worn even though it sits at its reduced cap, which is what trampling is for.
Wear needs pressure: drought alone never wears a cell, so a dry year does not desertify the
map. That is a deliberate narrowing (desertification is real) so the mechanic stays a
grazing effect and the sweep can attribute what it sees.

**D3 — The flip, one draw per ripe cell.** In row-major order, a cell with
`thrive_days ≥ climb_days[next rung]` whose next rung exists (D1) and whose moisture is at
least `climb_moisture[next rung]` rolls `rng.chance(flip_chance)`; on success it climbs one
rung and both counters reset. A cell with `wear_days ≥ wear_days_needed` above Dirt rolls
the same chance and drops one rung. The chance (0.10) spreads a region's flips over a couple
of weeks instead of turning a homogeneous basin (rain is per region, so moisture
homogenises within one) into forest on a single midnight. On any flip `vegetation =
min(vegetation, max_vegetation[new])`; nothing else on the cell changes, and `world.seeds`
retention already re-reads the cap. The draw is guarded by `flip_chance > 0.0`, so the
neutral overlay (D6) makes no draws at all.

**D4 — Trampling is one factor in the target.** In `step_vegetation`:

```
target = max_vegetation[terrain] × biome.vegetation_scale() × season_cap × min(1, moisture / 0.5)
       × (1 − trample_w × prey_pressure)
```

Pure arithmetic, no state, no draw. With `trample_w = 0.5` a cell a herd sits on all day
(pressure ≈ 0.5) grows toward three quarters of its cap; a trail cell loses a tenth; and
because pressure decays at 0.85 a day the ring around a water hole relaxes within a
fortnight of the herd moving on. Predator traffic is not counted: wolves do trample, but
the mechanic is about grazers, and `pred_pressure` stays a pure observation field.
`warm_up` never sees a creature, so it is unchanged by construction; succession does not run
in it either (D5).

**D5 — Where it sits in the day.** `daily_update` gains one sub-step, **8b succession**,
after `step_regrowth_sites` and before `sample_series`. The trampling factor lives inside
step 3. So the day's existing draws (rain, then regrowth sprouts) keep their positions and
only the draws after them within the day, and every draw of the next day, shift, which is
the same shape as the territory slice's contest roll. The counters and flips are skipped
while `flip_chance == 0` (D6) but the classification still runs, because it is cheap and
keeps the counters honest for an overlay that reads them under the neutral overlay.

**D6 — Parameters, and the neutral control.** A `[succession]` table,
`SuccessionParams`, after `territory` in `Params` (serde order is load-bearing; this is
part of the VERSION bump):

| key | default | role |
|---|---:|---|
| `trample_w` | 0.5 | weight of `prey_pressure` against the vegetation target (D4) |
| `climb_veg` | 0.9 | vegetation ≥ this × today's target counts as thriving |
| `wear_veg` | 0.5 | vegetation < this × the untrampled target counts as worn |
| `trample_low` | 0.10 | pressure below this allows thriving |
| `trample_high` | 0.30 | pressure at or above this allows wear |
| `relax_per_day` | 1 | counter decay on a day that is neither |
| `climb_days` | Sparse 60, Grass 90, Dense 120, Forest 360 | thriving days needed to climb *into* each rung |
| `climb_moisture` | Sparse 0.15, Grass 0.25, Dense 0.35, Forest 0.40 | cell moisture needed to climb *into* each rung |
| `wear_days_needed` | 45 | worn days needed to drop one rung |
| `flip_chance` | 0.10 | daily roll once a cell is ripe (D3) |

The moisture cuts are set against the runtime equilibria, not the worldgen field:
`moisture_equilibria_by_rainfall` puts the creature-free dry-world land mean near 0.60 and
the wet world above 0.8, so under defaults moisture almost never blocks a climb and the
table matters only on dry, sandy basins. That is intended for the first slice; the cuts
are the second lever after `climb_days` if forest spreads too far. Setting
`trample_w = 0` and `flip_chance = 0` is the **neutral control**: the target is unchanged
to the bit, no cell ever flips and no draw is made, so the run is today's run exactly and
step 2 pins that against the *old* checksum. No `enabled` flag, in the spirit of the diet
and territory neutral overlays.

**D7 — Save `VERSION = 22 (21 on main is quirks; both landed the same day)`, checksum re-baselined, and the territory tripwire.** The two
`Cell` fields and `Params.succession` are serde-visible. Under defaults the pinned checksum
moves on the first day trampling touches a trafficked cell. The existing territory tripwire
`neutral_territory_reproduces_the_old_checksum` pins the pre-territory value
`0xf883_9b51_57e3_c3f8` under a territory-neutral overlay and *default everything else*; it
would now fail. It is not deleted: it takes the succession-neutral overlay too, so its
meaning becomes "with both mechanisms off, the run is the pre-territory run", and the new
tripwire `neutral_succession_reproduces_the_old_checksum` pins `0xf9ea_eb02_3e27_c085`
under succession-neutral with territory at defaults. Both are stated in a comment. The
counters are not fed to the checksum, as scent is not: they are derived from hashed state.

**D8 — Seeing it, within budget.**
- **Events.** One `Note` per region per day per direction, in the regrowth-site style:
  `Scrub is closing over <region>` on the first climb of the day and `Grazing wears
  <region> back` on the first drop. No new chip; both filter under the existing `notes`.
- **S06.** The Terrain composition table already lists every terrain with a count and a
  share; it starts moving. No change to the screen in this slice.
- **Series and CSV.** `Sample` gains `forest_cells` and `bare_cells` (Dirt + Sand), the CSV
  gains the two columns at the end, and `--summary` gains `forest_pct` and `bare_pct` at
  the last day. Those are the two numbers the sweep asserts on.
- **S01 look mode** already prints the terrain name and vegetation, so a flipped cell reads
  correctly with no change.
- **S02k succession overlay**, a `Base::Succession` row on the S14 Base tab (the tab has
  thirteen rows and seven are used): tint `thrive_days / climb_days[next]` on the `veg` ramp
  for climbing cells and `wear_days / wear_days_needed` on the `heat` ramp for wearing
  cells, `glyphs::UP` / `glyphs::DOWN` on a ripe cell, off-ladder cells dimmed as the
  vegetation overlay dims rock. Sidebar copy: `Succession`, `ground climbing toward forest
  or wearing to dirt`, legend `worn … lush`, `By region` share of ripe cells. This is the
  last step and the one to cut if the modal's description pane needs its rows; the fallback
  is the S06 table plus the CSV columns.

**D9 — Documentation lands in C2.** Succession is ecology, so it is a new FR12 in
`c2-ecology.md` after FR11, with D1–D6 in requirement form, the D6 table, the neutral
control and a results row; the C2 decision that reads "grazing does not change terrain (C3
may revisit)" gets a dated strike-through pointing at FR12. The C5 status note gets one
line saying the habitat-side experiment ran and what it showed. `feature-ideas.md` marks
the slice's two bullets as planned here.

---

## Data model

```rust
// src/sim/params/succession.rs — `[succession]`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SuccessionParams { /* the ten fields of D6, documented */ }
impl SuccessionParams {
    /// D4: `1 − trample_w × pressure`, clamped to 0..=1.
    pub fn trample_factor(&self, prey_pressure: f32) -> f32;
    /// True when the overlay disables the mechanic (`trample_w == 0 && flip_chance == 0`).
    pub fn is_neutral(&self) -> bool;
}

// src/sim/world.rs
impl Terrain {
    /// D1: the next rung up, or `None` at the top of the ladder or off it.
    pub const fn climb(self) -> Option<Self>;
    /// D1: the next rung down, or `None` on Dirt or off the ladder.
    pub const fn wear(self) -> Option<Self>;
    pub const fn on_ladder(self) -> bool;
}
pub struct Cell { /* … */ pub thrive_days: u16, pub wear_days: u16 }   // after `parasite_load`

// src/sim/ecology.rs — façade after the split
mod succession;      // step 8b and its helpers
#[cfg(test)] mod tests;
// src/sim/ecology/succession.rs
pub(super) fn classify(cell: &Cell, target: f32, untrampled: f32, sp: &SuccessionParams) -> Signal;  // Thriving | Worn | Neither
pub(super) fn step_succession(world: &mut World, rng: &mut Rng, time: &Time, events: &mut EventRing, ecology: &EcologyParams, sp: &SuccessionParams, season_cap: f32, w: usize, h: usize);
```

`step_vegetation` and `step_succession` both need today's target per cell; rather than
recompute it, step 3 writes it into a scratch `Vec<f32>` it already sizes for `seed_mask`,
and 8b reads it. `daily_update` takes `&SuccessionParams` as one more plain argument
(`too_many_arguments` is allowed crate-wide and the function already carries the allow).

---

## Steps

Each step ends green on its tier before the next starts. Steps 0 and 1 leave the checksum
where it is; step 2 is the only one that moves it.

### Step 0 — `[succession]` params and the save bump
- `src/sim/params/succession.rs` with `SuccessionParams`, `Default`, the two helpers and
  unit tests `trample_factor_is_one_at_zero_pressure`, `neutral_overlay_is_detected`,
  `climb_tables_cover_every_rung_above_dirt`.
- Register in `params.rs` (`pub mod succession; pub use succession::SuccessionParams;`),
  the field after `territory`, ten `FIELD_DOCS` lines (tables get one line each, as
  `ecology.max_vegetation` does); `validate` rejects negative rates, `climb_veg ≤ wear_veg`,
  `trample_high < trample_low`, and a `climb_days` or `climb_moisture` table missing any of
  the four rungs above Dirt.
- `save::VERSION = 22 (21 on main is quirks; both landed the same day)`. The version tests build their files synthetically and need no
  fixture change.
- `scripts/affected-tests.sh`: the `src/sim/ecology.rs)` line becomes `src/sim/ecology*)`
  so the new child module maps to `units[sim::ecology]` and `chunks[ecology]`.
- Tier: `just test-unit sim::params`, `just test-unit sim::save`.

### Step 1 — the façade split and the new cell state (pure code motion first)
- Split `ecology.rs`: move `mod tests` verbatim to `src/sim/ecology/tests.rs`
  (re-applying its lint allows), then add `mod succession;` with the ladder helpers only.
  Checksum test green before anything else changes: that is the proof the split is pure.
- `world.rs`: `Terrain::{climb, wear, on_ladder}` with a unit test that walks the ladder
  both ways and returns `None` off it; `Cell.thrive_days` and `wear_days`, zeroed at
  generation and in every hand-built literal (`s06_ecology.rs`, `creatures.rs`,
  `ecology.rs`, `genetics.rs`, `behavior/tests.rs`, `world/classify.rs`,
  `genetics/tests.rs`, `widgets/map/tests.rs`).
- `just test-unit sim::tests::checksum` still green: the fields exist but nothing reads
  them.
- Tier: `just test-unit sim::ecology`, `sim::world`, `sim::tests::checksum`, `ui`.

### Step 2 — trampling, the counters, the flip (D2–D5)
- `step_vegetation`: the trample factor and the target scratch vector.
- `succession.rs`: `classify` and `step_succession`; the call in `daily_update` after
  step 8; the two `Note` texts.
- Tests in `ecology/tests.rs`, on a hand-built flat world where the fixture pattern in
  `predation.rs::flat_world` already exists:
  `trampled_cell_grows_toward_a_lower_target`, `thriving_meadow_climbs_after_climb_days`
  (rng seeded so the first ripe roll succeeds), `worn_grass_drops_after_wear_days`,
  `drought_alone_never_wears` (low vegetation, zero pressure, counters stay at zero),
  `relax_decays_both_counters`, `forest_needs_a_wooded_biome` (a tundra meadow never
  climbs), `climb_needs_moisture`, `sand_marsh_rock_and_water_never_flip`, `flip_clamps_
  vegetation_to_the_new_cap`, `one_note_per_region_per_day_per_direction`, and the two
  tripwires of D7: `neutral_succession_reproduces_the_old_checksum` (seed 1, the pinned
  tick count, asserting `0xf9ea_eb02_3e27_c085`) plus the amended territory tripwire.
- Re-baseline `sim::tests::checksum_is_fnv_stable` with a comment; update `AGENTS.md`.
- Tier: `just test-unit sim::ecology`, `sim::behavior::tests_territory`,
  `sim::tests::checksum`; `just test-chunk ecology` and `herbivores` in the background
  (`yearly_cycle_ratio` clears the herbivores and so sees no trampling and no wear, but a
  creature-free meadow may now climb to forest inside two years, which moves `veg_mean`;
  if the summer/winter ratio leaves 1.4..3.0 the test takes the neutral overlay, since it
  is a test of the seasonal cycle, not of succession).

### Step 3 — series, CSV, summary and the bench
- `stats.rs`: `Sample.forest_cells`, `Sample.bare_cells`, the CSV columns; `stats/tests.rs`
  `daily_sampling_fields` extended; `ecology::sample_series` counts them in the pass it
  already makes over the cells.
- `main.rs`: `forest_pct` and `bare_pct` after the territory columns.
- `examples/bench_c5.rs`: keys `trample_w`, `flip_chance`, `climb_days_forest`,
  `wear_days`, and a `succession=off` shorthand for the neutral overlay; the summary line
  adds forest and bare shares.
- Tier: `just test-unit sim::stats`, `just test-chunk headless` in the background.

### Step 4 — screens and docs
- S02k / S14: `Base::Succession` in `Base::ALL`, the overlay cell function, the sidebar
  copy and legend, `docs/screens/s02-map-overlay.md` row S02k and
  `s14-overlay-switcher.md`'s layer table; cut this bullet, not the others, if the modal
  has no spare row.
- `cargo test --lib -- --ignored regenerate_screen_renders` for S02k and S14; every glyph
  through `all_glyphs_are_cp437` (`UP` and `DOWN` are already in the table).
- `docs/chunks/c2-ecology.md`: FR12 and the struck decision (D9);
  `docs/chunks/c5-predators.md`: one status line; `AGENTS.md`: `VERSION = 22 (21 on main is quirks; both landed the same day)`, the
  checksum, the affected-tests glob; `docs/PERFORMANCE.md` only if step 5 measures more
  than a 1 % change.
- Tier: `just test-unit ui`; `cargo test --test components` only if a widget changed.

### Step 5 — verification and the first balance read
- `just check`; the unit tiers above; `ecology`, `herbivores`, `predators` and `headless`
  chunk binaries.
- `scripts/sweep.sh 1 6 5` twice, defaults and `succession=off`, and compare in
  `summary.csv`: `forest_pct` and `bare_pct` at years 1, 2 and 5; vole, hare and deer
  counts at years 1 and 2; prey starvation deaths; and the C5 bands
  `six_species_five_years`, `hunt_success_band`. The claims that must hold before any doc
  says so: (1) the neutral run is bit for bit today's run (the tripwire); (2) under
  defaults the terrain composition moves, forest or bare share differing from the neutral
  run by at least two points of the land on 5 of 6 seeds by year 5; (3) trampling does not
  raise prey starvation above the neutral run by more than the seed-to-seed spread. The
  hoped-for and unpromised result is voles alive at year 2 on more seeds than today.
- Two acceptance claims in `tests/ecology.rs`: `ungrazed_meadows_close_into_forest`
  (prey cleared, seed 42, five years, `forest_cells` above the generation count by a
  margin fixed from the sweep) and `grazed_ground_wears_and_recovers` (default roster,
  seed 42, two years, `bare_cells` exceeds the neutral run's at year 1 and at least one
  `Scrub is closing over` note appears by year 2).
- Record the numbers in the C2 FR12 results row and in memory (`succession-findings`).

---

## Test plan summary

| Layer | Test | Guards |
|---|---|---|
| params | `succession` unit tests, `field_docs_complete`, `validate` | tables complete, neutral control, docs |
| world | ladder walk, `on_ladder`, literal sites compile | D1, no cell left without the fields |
| save | `older_version_rejected`, `version_mismatch_rejected`, `round_trip_checksum_3_seeds` | format, counters round-trip |
| ecology | the twelve unit tests of step 2, the two tripwires | D2–D5, exact gating, draw-free neutral |
| determinism | `checksum_is_fnv_stable` (re-baselined), both tripwires | draw order, both mechanisms off = old run |
| chunk | `ecology`, `herbivores`, `predators`, `headless` in release | C2 cycle, prey untouched, C5 bands, CSV |
| UI | render regeneration, `ui` tier, CP437 scan | 155 × 45 budgets |

## Risks and how the plan handles them

- **Meadows close over the voles' food.** Forest sits at 0.9 on the diet axis, so a vole
  (breadth near 0) cannot eat a cell that used to be meadow, even though it now hides there
  at cover 1.0. That trade is the experiment, not a bug; if the sweep shows voles starving
  where they used to be eaten, the first lever is `climb_days[Forest]` (360 is one year of
  thriving; doubling it makes forest a five-year event), then `climb_moisture[Forest]`.
- **Lockstep flips.** Rain is per region, so a basin's cells ripen together. `flip_chance`
  spreads a basin's flips over a fortnight; if the map still flips in blocks, the follow-up
  is a per-cell jitter on `climb_days` seeded from the cell index, which is draw-free.
- **Trampling starves the herds.** A herd lowers its own cap and, if it does not move, eats
  down to nothing. Graze scoring already prefers the best cell in the sense ellipse, so the
  herd should rotate; the sweep's starvation column is the check and `trample_w` the
  lever. The tests are not.
- **Desertification runaway.** Wear requires pressure, so drought never wears a cell, and
  Dirt is the floor. A worn Dirt cell recovers through the normal regrowth path and can
  climb again.
- **The territory tripwire.** It pins a run with territory off and everything else at
  defaults; succession at defaults breaks it. D7 amends it deliberately rather than
  deleting it and says why in the test.
- **Fixture churn.** Eight files build `Cell` by hand; each gains two zeroed fields.
  Mechanical, but every one is in the step 1 list so none is missed.
- **Cost.** One more classification pass over the cells per day and one multiply per land
  cell in step 3; against a behaviour tick at 97 % of step time this is noise. Measured in
  step 5, recorded in `docs/PERFORMANCE.md` only if it exceeds 1 %.
- **The S14 modal row.** A thirteenth-row budget with seven used should fit an eighth base
  row; D8 names the cut and the fallback.

## Result (2026-09-22)

Everything through step 4 landed; step 5 ran as one-, two- and five-year sweeps on seeds
1–6 against the neutral overlay, plus `trample_w = 0.25` at one and two years. The numbers
are in [C2 FR12](chunks/c2-ecology.md#fr12-succession-and-trampling-2026-09-22). The short
version:

- **Claims 1 and 2 hold**: the neutral run is bit for bit today's run, and the terrain
  composition moves on every seed by year five, to forest shares of 50–60 % where the
  grazers die out.
- **Claim 3 holds on the letter and fails in spirit**: starvation stays within the seed
  spread, but deer are lower on every seed at year one and gone by year five on three seeds
  where the neutral run keeps large herds. The C8 herd-grazing rule stacks a herd on cells
  that trampling then halves. At `trample_w = 0.25` the herds survive to year five on five
  of six seeds and the map still moves, so **that became the default**; a herd that rotates
  off trampled ground is the first follow-up.
- **Voles**: better at year one on two seeds, worse on two, flat on two; fewer extinctions
  at year two (19 vs 23), the five-year collapse unchanged.
- **Cost** under 1 % of the step.

## Follow-ups not in this plan

All recorded under *Plants that animals can change* in [feature-ideas.md](feature-ideas.md):
carcass and dung fertilisation as a per-cell nutrient, mast years, two vegetation layers,
the plant roster, seed rain and animal-carried seed, grazing response, fire (which wants the
dead-biomass field this slice does not add), plant evolution, the green wave, and
predator trampling if it ever matters.
