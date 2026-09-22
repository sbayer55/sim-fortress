# Top dynasties — implementation plan

*Validated against `main` at 0e6d3e9 (the territory merge, PR #29) on 2026-09-22.*

**Goal.** Build [S16 Top Dynasties](screens/s16-top-dynasties.md): one region at a time,
its four best dynasties by kills, a banner with portrait and six stats against the valley
mean, six cumulative race charts against the region's other lines, the living members, a
Top-member sidebar and a Watch strip of pins. The S16 document is the specification; this
file is the order of work, the design decisions behind it, and what "done" means for each
step.

**Non-goals.** No prey dynasties on screen (the root is stored for every species; the
store keeps predators). No winter severity (every winter counts). No persistence of pins.
No change to how any existing screen draws: the S01 scent sidebar delegates to a moved
helper and its render is byte-identical.

**Starting point** (`main` at 0e6d3e9). A dynasty needs three things the sim does not
keep: the kills of dead members (`LifeRecord` in `src/sim/lineage/lifelog.rs` carries no
id, and `Lineage::prune` at `src/sim/lineage.rs:298-329` deletes dead nodes), a per-year
history per line (`Series` keeps 720 days per species), and a cheap territory count
(`s03_inspector/life.rs:155` scans the species block per creature). File sizes:
`src/sim/mod.rs` 624, `src/sim/lineage.rs` 501, `src/sim/creatures.rs` 677,
`src/sim/behavior.rs` 362, `src/sim/save.rs` 568, `src/sim/world.rs` 385,
`src/ui/app.rs` 655, `src/ui/tests.rs` 671, `src/ui/screens/tests.rs` 682,
`src/ui/screens/mod.rs` 133, `src/widgets/chart.rs` 306, `src/widgets/table.rs` 324.
`app.rs` and the two `ui` test files are within 150 lines of the 800-line ceiling; this
plan adds no more than twelve lines to any of them.

**Standing constraints.** Everything in `AGENTS.md`: the 800-line ceiling and the façade
pattern, clippy pedantic with the 80-line / complexity-15 / 6-argument / 3-bool limits,
CP437 glyphs from `glyphs.rs`, colours from `theme.rs`, species as data, casts through
`cast!`, the components tier for anything under `src/widgets/` or `docs/components/`.
Determinism: no RNG draw and no event is added, so `checksum_is_fnv_stable` stays
`0xf9ea_eb02_3e27_c085` and `tests_territory`'s neutral-overlay checksum stays
`0xf883_9b51_57e3_c3f8`. The save format moves to `VERSION = 18`.

---

## Design decisions

**D1 — `root` on the lineage node, dynasties in a persisted store under `Lineage`.**
`LineageNode` gains `root: CreatureId` (the mother's root, else self), filled once in
`Lineage::record` (`lineage.rs:154-166`). A new `Lineage.dynasties: Dynasties`
(`#[serde(default)]`, precedent `Sim.chronicle` at `src/sim/mod.rs:139-140`) keyed by root
holds what pruning would lose: the founder label, member counts, dead-member totals and
the per-year rows. Both writers (`record` and the death fold) already run inside `Lineage`
with the creature and the roster in hand.

**D2 — Predators only in the store; `root` on every node.** The default world starts 18
predator founders and 510 prey founders, and prey have no scent block. `Lineage::record`
folds into `dynasties` only when `roster.kind(c.species) == Kind::Predator`; `root` is
stored for every species so a prey view can come later without touching `record`'s
callers.

**D3 — Dead totals fold at death, living totals are read live, year rows close at
Spring.** `behavior::kill` (`src/sim/behavior.rs:215`) is the single death funnel: after
`lineage.record_death(..)` it calls `lineage.record_dynasty_death(c, roster, day)`. At the
year boundary (`run_midnight`, `mod.rs:319-326`, when `time.day_of_year() == 1`) one pass
over the living creatures grouped by root plus one `held_cells_by_holder` pass per predator
species gives `living_totals`; each line pushes `YearRow { year, dead + living }` with
territory and age as levels.

**D4 — Two survival tallies on `Creature`, hooked where the state flips.**
`droughts_survived: u16` bumps for living animals standing in a region whose drought
eased today (the `Sim.drought` array is diffed around `ecology::daily_update` in
`ecology_step`, `mod.rs:404-423`); `winters_survived: u16` bumps for every living animal
when `time.advance()` returns `Season::Spring` (`mod.rs:254-256`; `advance` never returns
the tick-0 Spring). `Creature::survival_events()` sums infections survived, escapes,
contests won and the two tallies.

**D5 — Territory tally promoted to `World`.** `World::held_cells_by_holder(species,
hold_min) -> Vec<(CreatureId, u32)>` is the loop from `s01_map/scent.rs:36-48`; the S01
scent sidebar delegates to it.

**D6 — Retention constants, not params.** `YEARS_KEPT = 40` rows per line and
`EXTINCT_KEEP_YEARS = 10` after the last member dies, as `pub const`s like
`LIFE_WINDOW_DAYS` / `LIFE_LOG_MAX` (`lifelog.rs:18-20`). No `[dynasties]` table.

**D7 — Save VERSION 18 in the first sim step.** The creature tallies are the first
field-order change, so step 1 bumps `save.rs:42` with a history sentence covering all of
this work; every intermediate commit loads its own saves. `round_trip_checksum_3_seeds`
(`save.rs:500-517`) gains a `dynasties` equality assertion because the checksum excludes
the lineage.

**D8 — An S15-shaped opaque data screen with a per-day cache.** `DynastiesScreen`
mirrors `TraitsScreen` (`src/ui/screens/s15_traits.rs:33-41, 153-158`): a
`RefCell<Option<Ranked>>` keyed on `sim.time.day_index()`, a `View` bundle handed to the
panel modules, `Esc -> Action::Pop`, everything else `Action::Unhandled`.

**D9 — Selection by identity, pins on `AppState`.** The screen keeps the selected root
id and member id; when the ranking shifts the selection follows the line, falling back to
the region's first line. `AppState.pins: Vec<Pin>` with `enum Pin { Dynasty(CreatureId),
Member(CreatureId) }`, at most four, session only.

**D10 — A `RaceChart` component rather than `Chart`.** `widgets::Chart` bins samples
into columns and draws `▀▄` half-blocks with a six-cell label gutter and a seven-label
`┼` axis (`src/widgets/chart.rs:23, 260-277`, `chart/axes.rs`); the approved look is one
point per year at a two-cell pitch, `■` bold points for the lead, `∙` dim points for
rivals, step connectors `─ │ ┌ ┘ ┐ └`, the y maximum inside the plot and a `└1───3` foot.
A ~230-line `src/widgets/race_chart.rs` with a sheet registered in
`tests/components/registry.rs` is smaller than bending `Chart`, and it settles
`chart.md`'s open question about `└` having no glyph constant.

**D11 — `Table` for both lists, with two recorded deviations.** The Table owns column 0
as its `►` marker, so the table is drawn one cell right of the inner edge and the screen
puts `♦` at the edge on pinned rows; the unfocused list is drawn with `.selected(None)`
and a `·` in the marker column. The Table header is `theme::dim_text()`, not the mockup's
`HEADER_BG` band; the sheet wins.

**D12 — Portraits keyed by roster glyph with a kind fallback.** `portraits.rs` holds the
mockup's 15 × 7 art for `F`, `W`, `L` plus generic predator and prey portraits;
`for_species(roster, id)` matches the adult glyph letter, else the kind. No name is
matched.

**D13 — Global key `d`; `s` and `p` are consumed while S16 is up.** `d` is unbound
globally and no S01 mode uses it. On S16 `s` cycles the species filter and `p` pins, as
S04 consumes `s` for sort; both are advertised in the status bar.

**D14 — Height-flexible member list.** The main panel has 34 fixed inner rows; member rows
are `inner_h − 34` clamped to 1..=5: five on a live 45-row terminal, four in the 44-row
test harness and the S16a render.

## Data model

```rust
// src/sim/lineage.rs — LineageNode gains, after `parents`:
/// The founder this node descends from through mothers only: the mother's
/// `root`, or the node itself for founders and for a newborn whose mother's
/// node is absent. Set once by `record`, never changed.
pub root: CreatureId,

// src/sim/lineage/dynasties.rs (new)
pub const YEARS_KEPT: usize = 40;
pub const EXTINCT_KEEP_YEARS: u32 = 10;

/// The six S16 stats: running totals (kills, young, surv, muts) and levels (terr, age in days).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally { pub kills: u32, pub terr: u32, pub young: u32, pub surv: u32, pub age: u32, pub muts: u32 }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct YearRow { pub year: u32, pub stats: Tally }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dynasty {
    pub root: CreatureId,
    pub species: SpeciesId,
    /// `Creature::label` at record time; the node is pruned eventually.
    pub founder: String,
    /// Negative for founders (`Creature::born_day`).
    pub founded_day: i32,
    pub root_generation: u32,
    pub max_generation: u32,
    pub members_ever: u32,
    pub members_living: u32,
    /// Folded at death: kills, young, surv, muts of dead members (terr and age stay 0).
    pub dead: Tally,
    /// Day the last living member died; the line never regrows after this.
    pub died_out_day: Option<u32>,
    /// Closed years, oldest first, at most `YEARS_KEPT`.
    pub years: Vec<YearRow>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Dynasties { lines: BTreeMap<CreatureId, Dynasty> }

impl Dynasties {
    pub fn record_birth(&mut self, root: CreatureId, c: &Creature, roster: &Roster);
    pub fn record_death(&mut self, root: CreatureId, c: &Creature, day: u32);
    pub fn close_year(&mut self, year: u32, day: u32, year_days: u32, living: &BTreeMap<CreatureId, Tally>);
    pub fn get(&self, root: CreatureId) -> Option<&Dynasty>;
    pub fn iter(&self) -> impl Iterator<Item = &Dynasty>;
}

/// Views for the screen.
pub struct DynastyMember { pub id: CreatureId, pub label: String, pub region: u8, pub stats: Tally, pub contests: (u16, u16) }
pub struct DynastyView {
    pub root: CreatureId, pub species: SpeciesId, pub founder: String, pub founded_day: i32,
    pub generations: u32, pub members_ever: u32, pub members_living: u32,
    /// Region holding the plurality of living members (lowest index on ties); None when extinct.
    pub region: Option<u8>,
    pub totals: Tally,                 // dead + living, now
    pub years: Vec<YearRow>,           // closed years, oldest first
    pub members: Vec<DynastyMember>,   // living, kills desc then id
}
/// Per root, the six stats of its living members: one pass over the living plus one
/// `held_cells_by_holder` pass per predator species. No RNG, no events.
pub fn living_totals(sim: &Sim) -> BTreeMap<CreatureId, Tally>;
/// Every dynasty with its members, kills desc, then founded_day asc, then root id.
pub fn rank(sim: &Sim) -> Vec<DynastyView>;

// src/sim/creatures.rs — after `contests_lost`:
/// Droughts that eased in the region this animal stood in while alive (S16 survival).
pub droughts_survived: u16,
/// Springs reached alive; every winter counts, the sim has no winter severity.
pub winters_survived: u16,
impl Creature { pub fn survival_events(&self) -> u32; }

// src/sim/behavior/survival.rs (new)
pub fn droughts_eased(store: &mut CreatureStore, world: &World, before: [bool; 8], after: [bool; 8]);
pub fn winter_survived(store: &mut CreatureStore);

// src/sim/world.rs — after `scent_block`:
pub fn held_cells_by_holder(&self, species: SpeciesId, hold_min: f32) -> Vec<(CreatureId, u32)>;

// src/ui/screens/s16_dynasties.rs (façade) and its modules rank, watch, header, banner, race, members, sidebar, portraits, tests
pub struct DynastiesScreen { region: usize, species: Option<SpeciesId>, focus: Focus, sel_dynasty: Option<CreatureId>, sel_member: Option<CreatureId>, cache: RefCell<Option<Ranked>> }
pub enum Focus { Dynasties, Members }
pub enum Pin { Dynasty(CreatureId), Member(CreatureId) }   // AppState.pins, MAX_PINS = 4
pub(super) enum Stat { Kills, Terr, Young, Surv, Age, Muts }

// src/widgets/race_chart.rs (new component, sheet docs/components/race-chart.md)
pub struct RaceSeries<'a> { values: &'a [f32], start: usize, color: Color, lead: bool }
pub struct RaceChart<'a> { title, series: Vec<RaceSeries<'a>>, rows: u16, first_label: usize, y_max: Option<f32>, fmt: Option<&'a dyn Fn(f32) -> String> }
```
No struct carries a bool; `droughts_eased` has four arguments (arrays, not bools);
`close_year` has four; `BTreeMap` only under `src/sim`; `u16::saturating_add`; `get` and
`entry`, never indexing.

## Steps

Each step ends green: `just check`, the module's `just test-unit`, and the two checksum
tests. Steps 8 and 9 also run `cargo test --test components`. One commit per step, named
`S16 Top Dynasties step N: <what>`.

### Step 0 — docs first
This file and [screens/s16-top-dynasties.md](screens/s16-top-dynasties.md); index rows in
`screens/README.md`, the key row in `screens/s01-world-map.md`, the status paragraph in
`components/README.md`. *Done when* `git diff --stat` is docs only.

### Step 1 — survival tallies, `survival_events()`, hooks, VERSION 18
- `src/sim/creatures.rs`: the two fields after `contests_lost` (:331-332), `survival_events`
  after `age_days` (:352), the founder literal (:536-537); the newborn literal
  `src/sim/genetics.rs:338-339`; test literals `src/sim/behavior/tests.rs:100-101` and
  `src/sim/genetics/tests.rs:118-119`.
- New `src/sim/behavior/survival.rs` with `droughts_eased` and `winter_survived`; `mod
  survival;` and a `pub use` in `behavior.rs` beside `migration_daily` (:26).
- `src/sim/mod.rs:254-256`: after `push_season_event(season)`, `if season == Season::Spring
  { behavior::winter_survived(&mut self.creatures); }`.
- `src/sim/mod.rs:404-423` `ecology_step`: `let before = self.drought;` before
  `ecology::daily_update`, `behavior::droughts_eased(&mut self.creatures, &self.world,
  before, self.drought)` after it, inside the profiled block; region via
  `world.region_index(c.x, c.y).min(7)` (the idiom at `mod.rs:478`).
- `src/sim/save.rs:30-42`: `VERSION = 18` and the history sentence; `AGENTS.md`
  `VERSION = 18`.
- Tests: `sim::behavior::survival` (a drought easing bumps only the living in the flipped
  region, carcasses untouched; Spring bumps every living animal), `sim::save`.

### Step 2 — `World::held_cells_by_holder`, S01 delegates
- `src/sim/world.rs` after `scent_block` (:380-384): the loop from `s01_map/scent.rs:36-48`
  without the by-count sort. `holders()` in `src/ui/screens/s01_map/scent.rs:36-50` becomes
  the helper call plus its sort.
- Tests: `sim::world::tests` (marks set through `mark_mut` :373; counts per holder and the
  `hold_min` cutoff). Renders regenerate with no diff.

### Step 3 — `LineageNode.root`
- `src/sim/lineage.rs:17-40` the field; `:154-166` compute
  `c.parents.map(|(m, _)| m).and_then(|m| self.nodes.get(&m).map(|n| n.root)).unwrap_or(c.id)`
  before the insert. The only `LineageNode {` literal is at `:135`.
- Tests (`sim::stats::tests`, helper `lineage_creature` at `src/sim/stats/tests.rs:174`):
  founder root = self; a chain 1 → 2 → 3 gives root 1; the root survives `prune` with keep
  0; an orphan newborn's root is itself.

### Step 4 — the `dynasties.rs` store, birth and death folds
- New `src/sim/lineage/dynasties.rs` and `dynasties/tests.rs`; `pub mod dynasties;` in
  `lineage.rs:8-9`; re-export from `src/sim/mod.rs:30`.
- `Lineage.dynasties` (`:90-96`, `#[serde(default)]`) with an accessor; `record`
  (`:154-166`) calls `record_birth` for predators; new
  `Lineage::record_dynasty_death(&mut self, c: &Creature, roster: &Roster, day: u32)`;
  called at `src/sim/behavior.rs:215` right after `record_death`, before `record_life`.
- Tests (`sim::lineage::dynasties`): a founder creates a line; a child bumps `members_*`
  and `max_generation`; prey are ignored; a death folds kills, young, surv and muts and
  sets `died_out_day`; **pruning does not shrink totals** (fold deaths, `prune(&[20; 6],
  0, &empty_store)`, totals identical).

### Step 5 — `living_totals`, `close_year`, `Sim::close_dynasty_year`
- `dynasties.rs`: `living_totals(sim)` (roster `predator_ids()`, `c.age_days(day)`,
  `c.survival_events()`, `c.mutations.len()`, the held map per species); `close_year`
  (push, trim to `YEARS_KEPT`, drop extinct lines older than `EXTINCT_KEEP_YEARS ×
  year_days`).
- `src/sim/mod.rs:319-326` `run_midnight`: after `midnight_systems`, before
  `refresh_group_stats`: `if self.time.day_of_year() == 1 { self.close_dynasty_year(); }`
  with a private fn computing `year = time.year() − 1`, `day = cast!(day_index => u32)`,
  `year_days = 4 × season_days`.
- Tests: a small world (`Params::default()`, `clear_initial_counts()` and four foxes, the
  pattern at `mod.rs:86-97`; `season_days = 2` for eight-day years) stepped 200 ticks gives
  one row per line with `year == 1`, `age` = the founder's age at day 8 and `terr` = the
  held sum; 45 pushes leave `YEARS_KEPT` rows; an extinct line is dropped after
  `EXTINCT_KEEP_YEARS`; `checksum_is_fnv_stable` is still green (it crosses the year
  boundary at tick 8 634).

### Step 6 — `rank()` and the views
- `dynasties.rs`: `DynastyView`, `DynastyMember`, `rank(sim)`.
- Tests: Σ `members_living` equals the living predator count; `totals == dead +
  living_totals`; region plurality with ties to the lowest index; two seed-7 sims give
  identical `rank` output.

### Step 7 — save round-trip
- `src/sim/save.rs:500-517`: assert `loaded.lineage.dynasties() == sim.lineage.dynasties()`
  and spot-check a `root`. Test `sim::save::tests::round_trip_checksum_3_seeds`.

### Step 8 — glyphs
- `src/glyphs.rs:62-69`: `BOX_TL '┌'`, `BOX_TR '┐'`, `BOX_BL '└'`, `BOX_BR '┘'`; append to
  `ALL` (:137-145). `just test-unit glyphs`.

### Step 9 — `RaceChart` component and sheet
- `src/widgets/race_chart.rs` and `race_chart/tests.rs` (rising and falling corners, the
  lead drawn last, the `start` offset, window clipping, the y-max label overwritten, an
  empty series draws only the axes); `pub mod` and `pub use` in `src/widgets/mod.rs`.
- `docs/components/race-chart.md` from the template with two cell-exact examples pasted
  from the component's own render; `fn race_chart()` beside `chart()` in
  `tests/components/registry.rs:862` and an entry in `examples()` (:893-921); an index row
  under Data and a mention in the status paragraph of `docs/components/README.md`.
- Drift fix while there: `docs/components/chart.md` `### Today` (:117-138) and its open
  question on `└` (:252-254), `docs/components/sparkline.md` `### Today` (:72-78) now name
  the structs that landed.
- *Done when* `just test-unit widgets::race_chart && cargo test --test components` pass.

### Step 10 — pins on `AppState`
- An empty façade `src/ui/screens/s16_dynasties.rs` holding `Pin` and `MAX_PINS`;
  `pub mod s16_dynasties;` after `src/ui/screens/mod.rs:27`; `pub pins: Vec<Pin>` in
  `AppState` (`src/ui/app.rs:52-106`) initialised in `AppState::new` (:108-135); the import
  beside `:32`.

### Step 11 — façade and `rank.rs`
- The struct, `Focus`, `Stat`, `Ranked::build(sim)` over `lineage::dynasties::rank(sim)`
  (predator lines; every line when the roster has no predators), `View`, the `Screen` impl
  with the full key table; panels stubbed to Panel frames and the status bar.
- Tests (`s16_dynasties/tests.rs`, harness copied from `s15_traits/tests.rs:10-31`): region
  wrap through the All stop, `Tab`, `↑↓` by root id (the selection sticks when a rival
  overtakes), `s`, `p` up to four and unpin, `1`–`4`, `f` sets `app.follow` and pops, `l`
  pushes, `Esc` pops, `x` is unhandled; `rank_desc`, `top_pct`, `region_short`.

### Step 12 — `watch.rs` and `header.rs`
- The Watch strip; the region line, the chips, the dynasty Table with `♦` and `·` marks.
- Tests: the whole buffer is CP437 (S15 `tests.rs:71-79`) and the borders hold (columns 0
  and 154 on rows 1–3, columns 0, 111, 112 and 154 below; pattern `:93-100`).

### Step 13 — `banner.rs` and `portraits.rs`
- Divider, portrait, standing line, six `Bar` rows with label, value and suffix, the
  carried-by and pin-hint rows.
- Tests: every portrait line is 15 cells and CP437; the banner names the top line;
  `for_species` falls back by kind for an unknown glyph.

### Step 14 — `race.rs`
- Divider with the coloured legend, six `RaceChart`s in `Stat::ALL` order (territory and
  age series are the year rows' levels, the rest cumulative), the note row.
- Tests: the divider and six title rows with `■`; a line founded this year draws one point.

### Step 15 — `members.rs`
- Divider (focus colour) and the member Table with `Bar` cells and tails; `rows = inner_h −
  34` clamped to 1..=5.
- Tests: the 44-row harness shows `the top 4 by kills` and four rows; `Tab` turns the
  divider `BORDER_FOCUS`.

### Step 16 — `sidebar.rs`
- Every section in the spec's order; survival lines from the tallies; genetics from
  `Mutation { trait_idx, delta, generation }`.
- Tests: a sidebar line helper; a fresh world at day 0 draws; a member with no mutations
  reads `no mutation at birth`.

### Step 17 — wiring
- `src/ui/app.rs`: the import beside `:32`, `KeyCode::Char('d') =>
  Action::Push(Box::new(DynastiesScreen::new()))` after the `'t'` arm (:423).
- `src/ui/screens/s11_help.rs:196-210`: `("d", "top dynasties")`.
- `src/ui/screens/s01_map.rs:379`: ` [s] species · [t] traits · [d] dynasties`.
- Tests: `just test-unit ui::` (the S01 and S11 render tests still pass).

### Step 18 — render snapshot
- `src/ui/tests.rs`: the import beside `:150`, a files entry after `:222`
  (`docs/screens/renders/S16a.txt`, title `S16a  Top Dynasties - race chart`); run
  `cargo test --lib -- --ignored regenerate_screen_renders`; every other render
  regenerates with no diff.

### Step 19 — docs close-out and revalidation
- The spec's Status line becomes built, the S16a render replaces the mockup render, the
  Components deviations and the 44/45-row rule are recorded; this file gains an
  *Implemented* note. `git fetch origin main && git log --oneline HEAD..origin/main`, then
  `wc -l` on every touched file and `just check && just test-affected`.

## Test plan summary
| Level | Where | Covers |
|-------|-------|--------|
| unit | `sim::behavior::survival`, `sim::world::tests`, `sim::stats::tests`, `sim::lineage::dynasties`, `sim::save` | tallies and hooks, holder tally, root, folds, year rows, retention, rank, round trip |
| unit | `sim::tests::checksum_is_fnv_stable`, `sim::behavior::tests_territory` | determinism tripwires after every step |
| unit | `ui::screens::s16_dynasties` | every key, geometry, CP437 buffer, borders, empty world, pins |
| unit | `widgets::race_chart` | corners, lead order, start offset, window, y-max label, empty series |
| components | `tests/components.rs` via `docs/components/race-chart.md` + `registry.rs` | the two sheet examples, cell for cell |
| snapshot | `docs/screens/renders/S16a.txt` | visual regression via `regenerate_screen_renders` |
| chunk | none | no behaviour change: pure counters, no RNG, no events |

## Risks and how the plan handles them
- **Checksum.** Every hook is a counter placed after the existing draws of its tick and no
  event is pushed (`events.total()` is hashed). Nothing in `kill` is reordered around the
  new call.
- **Year-boundary cost.** One living pass and one scent pass per predator species once per
  8 640 ticks.
- **Node missing at death.** `prune` keeps `alive()` nodes so it cannot happen; the fold
  skips silently and a test asserts the incremental `members_living` equals the store.
- **A newborn whose mother's node is gone** founds a new line with `founded_day > 0`:
  visible, not silent.
- **Line budgets.** `app.rs`, `ui/tests.rs` and `ui/screens/tests.rs` each gain at most
  twelve lines; S16 modules target 300 lines or fewer.
- **`f` then `Tab`.** S01e's `Tab` and `n` change the followed creature; the pins survive
  on `AppState`, so `d` then `1`–`4` gets back. Left as an open question in the spec.
- **Living ranks partition the living predators.** True by construction (D2 records every
  predator birth); a step-6 test asserts it.
- **Sim API drift into the screen.** Only `Ranked::build` and `Stat::of_*` read the sim
  views; every panel reads `View` and `Standing`.
- **Clippy.** `handle_key` delegates per arm; sidebar sections are one function each;
  `Focus` is an enum; helpers are `pub(super)` in the façade only.

## Follow-ups not in this plan
- Winter severity once the sim has one; `winters_survived` needs no format change.
- A prey dynasties view: `root` is already on every node.
- S03 showing the two new tallies on its Survival line.
- A colour option on `Divider`, replacing the members divider re-draw.
