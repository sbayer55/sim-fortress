# C1 — Simulation core, world generation and app shell

Back to the [roadmap](README.md). Next: [C2 Ecology](c2-ecology.md).

## Goal
Turn the prototype viewer into a real application skeleton: a deterministic simulation
core that owns time and terrain, a screen stack that drives the existing widgets from live
state, and the controls the user needs at every later checkpoint (pause, speed, step,
help, world generation). No living things yet.

## Checkpoint (what the user sees)
The user runs `cargo run`, lands on the world-generation form, changes the seed and size,
presses Generate, and watches a terrain map with a running clock. Seasons change the
palette (winter snow, night tint), the ticker announces season changes, and the controls
modal pauses and changes speed. A headless run of the same seed prints identical
checksums twice.

## Scope

### In
- **Crate layout**: single crate with `src/lib.rs` exposing `pub mod sim; pub mod ui;
  pub mod widgets; pub mod theme; pub mod glyphs; pub mod fixtures; pub mod prototypes;`
  and a thin `src/main.rs`. Integration tests live in `tests/`. Sim/UI separation is
  enforced by a unit test that walks `src/sim/**` and fails if any file contains the token
  `ratatui`.
- **Dependencies added**: `serde` (derive), `toml = "0.8"`. CLI flags are parsed by hand
  (no clap).
- `sim::params::Params` with every tunable in this chunk, `Default`, `from_toml`, `to_toml`.
  Every `sim` type derives `serde::{Serialize, Deserialize}` from the start (C6 saves them),
  and `src/sim` uses only `Vec`/`BTreeMap` (no `HashMap`/`HashSet`; unit test).
- `sim::rng::Rng` (moved from `fixtures/rng.rs`), `sim::time::Time`, `sim::world`
  (`Terrain`, `Cell`, `World`, `World::generate(seed, &WorldParams)`), `sim::events`
  (`EventKind`, `Event`, ring buffer), `sim::Sim` with `new`, `step`, `checksum`.
- **Data types move to `sim` as plain data** (no methods returning ratatui types):
  `Terrain`, `Cell`, `World`, `Season`, `EventKind`, `Event`. `fixtures/mod.rs` re-exports
  them; UI styling becomes extension traits in `src/ui/style.rs` (`SeasonStyle::color/glyph`,
  `EventKindStyle::glyph/color/label`). `widgets::map::render` changes to take a
  `MapData<'a> { world: &World, creatures: &[MapCreature], selected: … }` view instead of
  `&Fixtures`; the prototypes adapt their one call site. `fixtures::world::generate(seed)`
  becomes a call to `sim::World::generate(seed, &WorldParams::default())` so prototype and
  live worlds come from one function.
- UI shell: `App`, screen stack, key routing, fixed 155×45 frame with the existing resize
  guard, tick accumulator driven by speed, redraw on tick or key.
- Live screens: **S01a, S01b, S01d**, **S09** (functional form), **S10**, **S11**.
- Headless mode and the `--prototypes [ID]` flag (prototype viewer stays available for
  side-by-side comparison until C6).

### Out
Vegetation/water dynamics, creatures, overlays, data screens, save/load, title screen.

## Dependencies
- Screen requirements: [S01](../screens/s01-world-map.md), [S09](../screens/s09-world-generation.md),
  [S10](../screens/s10-simulation-controls.md), [S11](../screens/s11-legend-help.md).
- Existing code: `src/widgets/*` (signature change to `map::render` only), fixtures
  generator and rng (moved), prototypes (adapted to the moved types).

## Functional requirements

### FR1 Params
```toml
[world]      width = 150  height = 40  water_pct = 20  forest_pct = 15  rock_pct = 5  rainfall = "normal"  # dry | normal | wet
[time]       season_days = 90  ticks_per_day = 24  start_hour = 6  sunrise_hour = 6  sunset_hour = 20
[ui]         speeds = [1, 2, 5, 10, 25]  base_ticks_per_second = 2.0
             auto_pause_on_extinction = true  log_births = false  pause_on_follow_death = true
[events]     capacity = 5000
```
Defaults for `water_pct/forest_pct/rock_pct` are frozen from the measured shares of the
0xC0FFEE fixture world (the implementer prints them once in a test and writes the rounded
values into `Default`; the values above are the expected magnitude). `rainfall` is stored
and has no effect until C2. Unknown TOML keys are an error naming the key. A `--params` file is a partial table
deep-merged over the defaults (so `[world] rainfall = "dry"` alone is valid).

### FR2 World generation
`World::generate(seed, &WorldParams)` is pure: one `Rng` seeded from `seed` feeds every
noise lattice and every erosion epoch, so the same seed and parameters give the same
cells. The generator lives in `src/sim/world/` (`noise`, `relief`, `flow`, `classify`).
1. **Tectonics** (`relief::tectonics`): elevation is 5-octave value noise sampled in a
   w × 2h space (cells are twice as tall as wide) through a low-frequency domain warp,
   with a 4-octave *ridged* field folded in only where the continent is high, so
   mountain chains sit on the uplands and lowlands roll gently. The base feature size is
   `max(w, 2h) / 5` (at least 22). A 3-octave rain field gives every cell a long-run
   rainfall weight in 0.5..=1.5.
2. **Time** (`relief::epoch`, `world.age` epochs, default 8): the lowest 60 % of the
   water target is fixed as base level. Each epoch lays a fresh seeded microrelief
   (scale 2.5 cells, amplitude 0.015), samples a seeded storm field that modulates
   rainfall, routes drainage by steepest descent (D8 with 1 : 2 : √5 step lengths),
   accumulates discharge, incises channels with the stream-power law `h ← (h + f·h_r) /
   (1 + f)`, `f = K·√(A·6000/N) / d` (implicit, so any epoch length is stable), and
   relaxes hillslopes with explicit diffusion (0.12, no-flux edges). Age 0 is the raw
   tectonic surface; age 30 is old, low and gullied.
3. **Water** (`classify::water_bodies`): the finished surface is depression-filled by
   priority flood from the base level and the map edges (rivers may leave the world), and
   drainage is re-routed over the filled surface. Of the `water_pct` target, up to 25 %
   goes to lakes (the deepest depressions), 15 % to rivers (the largest drainage areas,
   at least 6 cells) and the rest to the ocean (the lowest remaining cells). Because
   discharge only grows downstream, every river cell is connected to the ocean, a lake
   or the edge. Interior ocean and lake cells are `DeepWater`; shores and rivers are
   `ShallowWater`.
4. **Moisture**: `0.02 + 0.40·rain + 0.28·e^(−d/5) + 0.28·(1 − elevation) ± 0.07`, where
   `rain` is the rain field stretched to 1.8× contrast (rain shadows leave bare dirt),
   `d` is the chamfer distance to water and the last term is the rainfall climate
   (`dry` −, `wet` +), clamped to 0..=1. Land averages about 0.5 on a normal world with
   a few percent of bare dirt.
5. **Land cover** by quantile so the targets hold on any seed: the top `rock_pct` of all
   cells by `elevation + 0.6 × normalised slope` are `Rock`; up to 4 % of cells are
   `Sand`, the lowest land touching the ocean or a lake; the top `forest_pct` of the
   remaining land by moisture are `Forest`; the rest are `Dirt` / `GrassSparse` /
   `Grass` / `GrassDense` by moisture bands 0.28 / 0.42 / 0.58.
6. Initial vegetation per terrain as in the fixture; it does not change in C1.

Tests (`sim::world::tests`): for seeds 1..=20 at default size each share is within ±3
percentage points; generation is deterministic; age 30 is measurably smoother than age
0; a 0 % water target yields no water; water forms bodies, not speckle. Measured
generation time (best of 15): 150×40 ≈ 7 ms dev / 4 ms release, 200×60 ≈ 14 ms dev /
10 ms release, 1000×1000 ≈ 0.9 s release.

### FR3 Regions
The eight fixture rectangles, scaled `x' = round(x × W / 150)`, `y' = round(y × H / 40)`,
with the last column and row extended to the world edge so the regions tile the world.
Names are the fixed eight. Test `regions_cover_world`.

### FR4 Time
`tick: u64` starts at 0 = Year 1, Day 1 of Spring, `start_hour`.
- `hour() = (tick + start_hour) % 24`
- `day_index() = (tick + start_hour) / 24` (0-based, absolute)
- `day_of_season() = day_index % season_days + 1`
- `season() = (day_index / season_days) % 4` → Spring, Summer, Autumn, Winter
- `day_of_year() = day_index % (4 × season_days) + 1`
- `year() = day_index / (4 × season_days) + 1`
- `is_night() = hour < sunrise_hour || hour >= sunset_hour`
The clock label uses `day_of_season` (`Year 1, Day 1 of Spring  06:00`); S07/S05 labels use
`day_of_year` zero-padded to 3 (`Y1 D091`). Season boundaries (and tick 0) emit a `Season`
event with these texts: Spring `Spring returns to the valley; regrowth quickens`, Summer
`Summer settles over the valley; evaporation peaks`, Autumn `Autumn colours the valley;
regrowth slows`, Winter `Winter settles over the valley; vegetation regrowth halves`.

### FR5 Sim
`Sim { params, rng, time, world, events }`; `Sim::new(seed, params)`; `Sim::step() ->
StepReport { alerts: Vec<Alert> }` advances time and appends season events; `alerts` is
always empty until C5. `Sim::checksum() -> u64` is
FNV-1a 64 over, per cell in row-major order: `terrain as u8`, `elevation.to_bits()`,
`moisture.to_bits()`, `vegetation.to_bits()`; then `tick`, the rng state, `events.len()`.
Printed as `checksum=0x%016x`. Events are a ring buffer of `params.events.capacity`.

### FR6 App and screen stack
```rust
pub struct AppState { sim: Option<Sim>, paused: bool, speed_idx: usize, params: Params, speed_before_alert: Option<usize> }
pub struct App { state: AppState, stack: Vec<Box<dyn Screen>> }
pub enum Action { None, Unhandled, Push(Box<dyn Screen>), Pop, Replace(Box<dyn Screen>), Quit }
pub trait Screen {
    fn opaque(&self) -> bool;                       // false for modals
    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action;
    fn render(&self, app: &AppState, f: &mut Frame, area: Rect);
}
```
The **top screen sees every key first** and returns `Unhandled` for keys it does not
consume; only then does the stack apply the global table from
[docs/screens/README.md](../screens/README.md#global-key-bindings). Screens with text input
(S09) consume all printable characters while a text field is focused. Borrow mechanism:
`stack` lives outside the shared state (`App { state: AppState, stack: Vec<Box<dyn Screen>> }`)
and `handle_key`/`render` receive `&mut AppState` / `&AppState`, so a screen is never
borrowed twice. Rendering draws every screen from the lowest
`opaque` one upward; before drawing a non-opaque screen the stack dims everything drawn so
far by 55 % with `util::dim_area`, so modals show a dimmed backdrop and can stack (needed
by C5's alert over the controls modal). After every tick the loop drains
`StepReport.alerts` (empty until C5) and pushes the matching modal screens.

### FR7 Tick loop
`event::poll(16 ms)`; per frame accumulate `elapsed_seconds × base_ticks_per_second ×
speed` ticks (fractional carry), run at most 200 ticks per frame, redraw at most 30 times
per second. Unit test: an accumulator fed 10 s at x1 yields 20 ticks.

### FR8 World map screen (S01a/b/d)
Live world and clock. Panel title = world name (default `The Valley of Sunfall`). Default
viewport origin `(0, 0)`; `←→↑↓` scroll by 5 cells, clamped to `max_origin =
(W.saturating_sub(110), H.saturating_sub(40))` (defined once in `ui::viewport` and reused by
every centring rule in later chunks; worlds narrower than the viewport paint the frame
background in the unused columns). `Tab` toggles
the collapsed sidebar (S01b) — C3 uses `n` for "next creature" instead of `Tab`. Winter
and night palettes follow `Time` automatically. Sidebar: Clock live; Population rows show
`—` counts and empty sparklines; Resources: vegetation bar = mean vegetation over land
cells, water bar = `1.0`, counts of dens/regrowth/carcasses; Notable section shows one dim
line `no creatures yet`; Legend as prototype. Ticker: latest event (blank if none).
`q` returns to S09 (title flow arrives in C6). S09 handles `q` itself: inserted as text when
the name or seed field is focused, otherwise `Action::Quit`; no key reaches the global table
from S09.

### FR9 World generation screen (S09)
Form state `WorldGenForm { name, seed, world: WorldParams, initial_counts, evolution: (stored,
unused), preset }`, plus a preview `World`. Tab order: 8 world fields → 6 species rows
(stored in `params.creatures.initial_counts`; the rows become functional in C3) → 4
evolution fields (stored; live in C4) → presets (inert until C6) → 3 buttons; the panel hint reads
`field N of 26`. The Regrowth rate evolution field is live from C2. `Size` is one focusable field:
`←`/`→` adjust Width 100–1000 step 10, `↑`/`↓` adjust Height 30–1000 step 5. Seed accepts `0x`-prefixed hex or decimal u64; invalid text shows `invalid seed`
in the hint in WARN colour and Generate is refused. Name ≤ 23 characters. The preview
regenerates at most once per frame after a change; scale = smallest zoom-out (at least 1:2) that
fits 75×20, reported in the hint. Capacity/placement lines keep the prototype formulas.
Generate creates `Sim::new` and replaces the stack with S01. `Esc`/`[ Back ]` returns to
the running map if one exists, otherwise quits. If regeneration of a 200×60 preview
exceeds 16 ms, regenerate only on Enter (measure and record in the doc).

### FR10 Controls modal (S10)
The prototype's hard-coded strings in `s10_controls.rs` are updated in the same commit so the
glyph-for-glyph comparison stays meaningful. Conversion line `1 tick = 1 hour   1 day = 24 ticks   x‹n› = ‹2n› ticks/s` computed from
params. Step row `1 tick / 6 hours / 1 day`, `<`/`>` change step size; `.` pauses if
running, then steps that many ticks. `1`–`5` set speed directly; `+`/`-` do not wrap.
Options `[a]` auto-pause on extinction, `[b]` log births, `[c]` pause when a followed
creature dies map to `params.ui`.

### FR11 Help overlay (S11)
Static content from the prototype; only `?` and `Esc` close it in C1 (pass-through keys
`k o s` arrive with their screens).

### FR12 Headless
`sim-fortress --headless --seed N --ticks T [--params f]` prints
```
seed=42 ticks=4320 checksum=0x…
Y1 D001 06:00 season Spring returns to the valley; regrowth quickens
… (last 5 events, oldest first)
```
and exits 0.

## Acceptance criteria
- `cargo test` passes: determinism (two `Sim` with the same seed step 10 000 ticks →
  equal checksums), world-size bounds, percentage targets (seeds 1..=20), regions tile the
  world, time getters (season rollover, night hours, day-of-year at tick 0 = 1), screen
  stack push/pop/replace and modal-below rendering, key routing (`s09_text_field_consumes_printable_keys`, `s09_q_quits_when_no_text_focus`), tick
  accumulator, params TOML round trip, sim-has-no-ratatui, glyph CP437 test.
- `cargo run` opens S09; Generate leads to a live S01 ticking at x1 = 2 ticks/s (±10 %
  over 10 s, checked by the accumulator test and by eye).
- Live S01 chrome (borders, sidebar sections, ticker, status bar) matches `--prototypes
  S01a` glyph-for-glyph; terrain matches after scrolling the live map to x = 20, because
  both call `World::generate(0xC0FFEE, &WorldParams::default())` (the fixture then adds its
  own dens/carcasses/seeds/pressure decoration, which the live world does not have).
- `--headless --seed 7 --ticks 8640` finishes in under 1 s and prints the same checksum
  on two runs.
- No clippy warnings.

## Checkpoint demo script
1. `cargo run` → S09. Set seed `42`, Width `200`, Height `50`, Water `40`. Preview updates
   after each change.
2. Enter on Generate → S01a. Clock reads `Year 1, Day 1 of Spring  06:00`.
3. `+` twice → x5. A day passes in about 2.4 s; night tint appears after 20:00.
4. `p` → controls; `5` for x25; `Esc`. After about 45 s the clock reads Day 1 of Summer
   and the ticker shows the summer event; after about 2 more minutes Day 1 of Winter shows
   the snow palette.
5. `Tab` → wide map; `Tab` back. `?` → help; `Esc`. `q` → S09; `q` → quit.
6. `cargo run -- --headless --seed 42 --ticks 4320` twice; checksums match.

## Tests
- `sim::tests::{determinism_10k_ticks, checksum_is_fnv_stable}`
- `sim::world::tests::{size_bounds, target_percentages_20_seeds, regions_cover_world}`
- `sim::time::tests::{tick0_is_day1_spring_0600, season_rollover, night_hours, day_of_year_padding}`
- `sim::params::tests::{toml_round_trip, unknown_key_is_error}`
- `sim::tests::no_ratatui_in_sim`
- `ui::screens::tests::{push_pop_replace, modal_renders_below, s09_text_field_consumes_printable_keys, s09_q_quits_when_no_text_focus, viewport_max_origin_small_world, tick_accumulator_10s_x1_is_20}`

## Decisions made here (revisit at the checkpoint)
- Tick = 1 hour; speeds multiply a 2 ticks/s base; start at 06:00 so the first frame is day.
- Quantile thresholds for terrain so the S09 percentages are honest.
- Top screen gets keys first; globals apply only to unhandled keys.
- Single crate with `lib.rs`; prototypes kept behind `--prototypes` until C6.

## Risks
- Moving types out of `fixtures` touches every prototype file; do it first, in one commit,
  with the prototypes compiling before any live screen is written.
- Preview regeneration cost at 200×60 (see FR9 fallback).
