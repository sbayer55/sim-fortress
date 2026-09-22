# Hunt Watch — implementation plan

*Validated against `main` at 9c38900 (the S16 Top Dynasties merge, PR #30) on 2026-09-22.*

**Goal.** Build [S17 Hunt Watch](screens/s17-hunt-watch.md): one row per predator that is
hunting or has just failed, a hunter column, a ribbon that plots the chase tick by tick, a
prey column, a details panel for the selected row, pins shared with S16's Watch strip, and a
details toggle. The S17 document is the specification; this file is the order of work, the
design decisions behind it, and what "done" means for each step.

**Non-goals.** No new event kinds (the play-by-play is narrated from recorded samples). No
prey rows. No persistence of pins. No change to how the hunt is decided: the sim gains
observation, not behaviour, and the checksum locks prove it.

**Starting point** (`main` at 9c38900). The sim has the live facts of a hunt
(`hunt_phase`, `hunt_target`, `chase_start_tick`, `hunt_cooldown_until`, `eat_until`,
`kills`, `attempts`, `chase_stats`, `last_kill` in `src/sim/creatures.rs`; `kill_chance` in
`src/sim/params/predation.rs`) but no per-tick history, no record of how a hunt failed
(`fail_hunt` at `src/sim/behavior/hunt.rs:65-75` takes no reason), and no per-hunter
attempt history (the life log is prey-indexed and carries no killer). File sizes:
`src/sim/creatures.rs` 694, `src/sim/behavior/goals.rs` 468, `src/sim/behavior/hunt.rs` 421,
`src/sim/predation.rs` 415, `src/sim/behavior.rs` 367, `src/sim/mod.rs` 644,
`src/sim/save.rs` 575, `src/ui/app.rs` 660, `src/ui/tests.rs` 677, `src/glyphs.rs` 173,
`src/widgets/map.rs` 409. Nothing touched is within 100 lines of the ceiling; this plan adds
at most 25 lines to `mod.rs` and 12 to `app.rs`.

**What changed since the mockup was planned.** Main gained S16 Top Dynasties: the screen
number is 17, the global `d` opens Dynasties (so S17's details toggle is a documented local
override), `AppState.pins: Vec<Pin>` already exists with `Pin::Member(CreatureId)` and a cap
of four (`s16_dynasties.rs:48`), and the save format is at 18; a `RaceChart` component with
a sheet is the freshest precedent for the Ribbon.

**Standing constraints.** Everything in `AGENTS.md`: the 800-line ceiling and the façade
pattern, clippy pedantic with the 80-line / complexity-15 / 6-argument / 3-bool limits, CP437
glyphs from `glyphs.rs`, colours from `theme.rs`, casts through `cast!`, no `HashMap` under
`src/sim`, the components tier for anything under `src/widgets/` or `docs/components/`.
Determinism: no RNG draw and no event is added, so `checksum_is_fnv_stable` and
`tests_territory`'s neutral checksum keep their values. The save format moves to
`VERSION = 19`.

---

## Design decisions

**D1 — Hunt traces are a side table on `Sim`, not `Creature` fields.** `Sim.hunts: HuntWatch`
(`BTreeMap<CreatureId, HuntTrace>`, predators only), like `Lineage` and its life log. Saved,
`#[serde(default)]` after `chronicle` (`src/sim/mod.rs:144-146`). Not added to
`Sim::checksum`: it is derived state the step never reads; determinism is asserted by a
two-runs-equal test instead.

**D2 — One `Ledgers` bundle replaces `tallies` on the hunt path.** `pub struct Ledgers<'a> {
tallies: &'a mut DeathTallies, hunts: &'a mut HuntWatch }` in `src/sim/behavior.rs`, threaded
through `tick_creatures` → `update_one` → `update_hunt_stalk`, `fail_hunt`, `hunt_contacts`,
`resolve_kill`, `resolve_miss`. Every other pass keeps `&mut DeathTallies` via
`ledgers.tallies`. No function gains an argument.

**D3 — Three hooks plus one observe pass.** `begin` at the top of `update_hunt_stalk`
(idempotent while the same hunt is open; the six-tick replan re-picks the same prey and
resets `chase_start_tick`, so the trace keeps its own first Chase tick); `end(reason)` in
`fail_hunt`, which gains a `HuntOutcome` parameter (`Lost` at the two sense-range branches,
`Timeout` at the clock branch, `Miss` from `resolve_miss`), and in `resolve_kill`
(`Kill { eat_until }`); `HuntWatch::observe(store, tick)` in `tick_creatures` right before
`hunt_contacts` to push one post-movement sample per open hunt, record the chase start,
close hunts the replan abandoned as `Dropped`, mark dead hunters, and prune.

**D4 — Five outcomes.** `Kill, Miss, Timeout, Lost, Dropped`. `Dropped` is what the sim does
when a hungry hunter's replan chooses Drink or Rest mid-hunt: no `fail_hunt`, no attempt.

**D5 — The sample carries what the narration needs.** `gap: u8, phase: HuntPhase,
hunter_energy, prey_energy, hunter_hunger: f32, prey_goal: Goal, prey_terrain: Terrain`
(both enums already serialise). Ring of 36.

**D6 — Kill odds have one source.** `predation::odds(pp, sp, pred, prey, extra, sick) ->
OddsParts` and `predation::pack_participants` (moved from `hunt.rs:259-282`, `pub`);
`hunt_contacts` calls `odds(..).total`, so the UI and the roll cannot drift and the checksum
lock proves the refactor bit-identical. `kill_odds(sim, pred, prey)` is the UI entry.

**D7 — Rows live in the screen, pins in `AppState.pins`.** `render` is `&self`, so slot
upkeep uses the S15/S16 `RefCell` pattern. Pins reuse `Pin::Member(id)` and S16's
`MAX_PINS`; a fifth pin is refused with a status-bar note.

**D8 — The ribbon is a component.** `widgets::Ribbon` takes plain values (`stalk: &[f32]`,
`chase: &[f32]`, `outcome`, `live`, `caption`, `chase_max`, `seg`) and a `Rect`; the screen
maps samples to `p`. The sheet `docs/components/ribbon.md` carries a 30-column example
(2 stalk ticks, a 12-tick clock) and the 74-column S17 geometry.

**D9 — The tug value is a UI estimate**, spec item 9, with the verdict thresholds as
constants in one module.

**D10 — Between-hunt states are derived, not stored** (spec item 3).

## Data model

```rust
// src/sim/hunt_watch.rs
pub const RIBBON_LEN: usize = 36;
pub const HISTORY_LEN: u8 = 12;
pub const DEAD_LINGER_TICKS: u64 = 4;

pub struct HuntSample { pub tick: u64, pub gap: u8, pub phase: HuntPhase, pub hunter_energy: f32,
    pub prey_energy: f32, pub hunter_hunger: f32, pub prey_goal: Goal, pub prey_terrain: Terrain }
pub enum HuntOutcome { Kill { eat_until: u64 }, Miss, Timeout, Lost, Dropped }
pub struct HuntEnd { pub outcome: HuntOutcome, pub tick: u64, pub chase_ticks: u16 }
pub struct ChaseStart { pub tick: u64, pub hunter_energy: f32 }
pub struct HunterDeath { pub cause: Cause, pub tick: u64 }
pub struct HuntTrace { hunter, species, prey, start_tick, chase: Option<ChaseStart>,
    samples: VecDeque<HuntSample>, end: Option<HuntEnd>, history: u16, history_len: u8, died: Option<HunterDeath> }
pub struct HuntWatch { traces: BTreeMap<CreatureId, HuntTrace> }
// begin(hunter, species, prey, tick) · end(hunter, species, prey, outcome, tick, chase_ticks) · observe(store, tick)
// trace(id) · traces() · HuntTrace getters · history() most recent first

// src/sim/predation.rs
pub struct OddsParts { pub base, speed, aggression, size, formula, pack, sick, total: f32 }
pub struct HuntPair { pub pred: CreatureId, pub pred_species: SpeciesId, pub prey: CreatureId, pub prey_at: (usize, usize) }

// src/ui/screens/s17_hunts.rs
pub struct HuntWatch { selected: Option<CreatureId>, details: bool, lanes: RefCell<Lanes> }
struct Lanes { slots: [Option<CreatureId>; 8], tick: Option<u64> }

// src/widgets/ribbon.rs
pub struct Ribbon<'a> { stalk: &'a [f32], chase: &'a [f32], outcome: Option<RibbonEnd>, live: bool, caption: Option<&'a str>, chase_max: usize, seg: u16 }
pub enum RibbonEnd { Kill, Escaped, TimedOut, Lost }
```

No struct has a `bool` (the screen's `details` is the only one); no `Option<Option<_>>`;
no function above six arguments without the existing `allow`.

## Steps

Each step ends green: `just check`, then the named tier. Steps 1–6 are the sim, 7–13 the UI.

### Step 0 — docs first
`docs/screens/s17-hunt-watch.md`, this file, `docs/screens/README.md` rows (table, data
screens, navigation, stack diagram, global keys, renders). *Done when* the docs build is
a single commit with nothing under `src/`.

### Step 1 — `hunt_watch.rs`
New `src/sim/hunt_watch.rs` + `hunt_watch/tests.rs`; `pub mod hunt_watch;` in `mod.rs`.
Tests: `ring_caps_at_36`, `history_keeps_last_12_most_recent_first` (a `Dropped` end adds
no bit), `begin_is_idempotent_for_the_open_hunt_and_resets_after_end` (a different prey
while open closes the old hunt as `Dropped`), `end_without_begin_creates_the_trace`,
`prune_drops_dead_hunters_after_the_linger`. `just test-unit sim::hunt_watch`.

### Step 2 — `Sim.hunts`, VERSION 19
Field after `chronicle`, `HuntWatch::default()` in `Sim::new`, `VERSION = 19` with a
changelog sentence, `affected-tests.sh` row (`src/sim/hunt_watch*` → `sim::hunt_watch`,
chunk `predators`). `just test-unit sim::save::tests::version_mismatch_rejected`.

### Step 3 — `Ledgers`
`pub struct Ledgers<'a>` in `behavior.rs`; `tick_creatures`, `update_one`,
`update_hunt_stalk`, `fail_hunt`, `hunt_contacts`, `resolve_kill`, `resolve_miss` take it;
`ledgers.tallies` everywhere else; `run_behavior` builds it; the two test call sites
(`behavior/tests.rs:140`, `tests_social.rs:153`) build one. No recording yet.
`just test-unit sim::behavior`, `just test-unit sim::tests::checksum` (unchanged: the
tripwire for steps 3–5).

### Step 4 — hooks
`begin` in `update_hunt_stalk`; `fail_hunt(.., HuntOutcome)` at its three call sites;
`resolve_kill` ends with `Kill { eat_until }`; `observe` before `hunt_contacts`;
`HuntPhase::label()` next to `Goal::plain()`. Same tiers plus `sim::predation`.

### Step 5 — odds in one place
`predation::{OddsParts, HuntPair, pack_participants, odds, kill_odds}`; `hunt_contacts`
delegates to `odds(..).total` and `pack_participants`. Checksum unchanged.

### Step 6 — behaviour tests
`predation/tests/watch.rs` on the `arena` harness: trace opens on the first stalk tick; one
sample per tick and the chase start recorded; kill / miss / timeout / lost / dropped each
recorded with `chase_ticks` and the right history bit; a starved hunter is marked then
pruned; `kill_odds` equals the contact expression; two same-seed runs give equal `hunts`;
the save round-trip preserves `hunts`. Background: `just test-unit sim::save`,
`just test-chunk predators`.

### Step 7 — UI prep
`glyphs::CLOCK_OUT` (+ `ALL`); `common::wrap` moved from `s15_traits/sidebar.rs`
(S15a render unchanged). `just test-unit glyphs`, `just test-unit ui`.

### Step 8 — `Ribbon` component and sheet
`src/widgets/ribbon.rs`, re-export, `docs/components/ribbon.md` (Anatomy, Slots, Sizing,
Variants live / dimmed / resolved / with caption, Styling, Glyphs, Interaction none,
Composition, Testing), registry fixtures, README index row. `cargo test --test components`.

### Step 9 — `model.rs` and `beats.rs`
`HuntView::collect(sim, pins)` (the only reader of `sim.hunts`), `tug`, the verdict words,
`state_word`, `escape_routes`, `beats(trace, names, pp, now)`; tests on synthetic traces.
`just test-unit ui::screens::s17_hunts`.

### Step 10 — the screen
`s17_hunts.rs` + `lane.rs`; `pub mod s17_hunts;`; `h` in `app.rs`. Tests: borders, CP437 on
every cell (details on and off, with an idle lane), slots stable across ticks, pinned-first
order, `d` toggles 5 ↔ 8 lanes, arrows wrap and skip free rows, `p` / `Enter` (sets
`follow`, clears `follow_death_tick`, `leave_look`, Pop) / `i` / `o` / Esc / fall-through,
an empty world draws free rows. `just test-unit ui`.

### Step 11 — `details.rs`
Three columns, the stacked odds bar, the routes line, the play-by-play.

### Step 12 — other screens
S01 Notable row, map pin marker (`MapOpts.pins`), sidebar `♦ pinned:` line, S03 `♦`, S11
row (`s11_help_keys_column_fits`). `just test-unit ui`.

### Step 13 — renders and docs close-out
S17a and S17b entries in `regenerate_screen_renders` (after the existing snaps, step until
a predator is in Chase, cap 30 days); `just renders` (S01a, S11a, S17a, S17b change; the
others must not); spec Status → built; components README; revalidation stamp here.

## Test plan summary
| Level | Where | Covers |
|-------|-------|--------|
| unit | `sim::hunt_watch` | ring, history bits, begin/end semantics, prune |
| unit | `sim::predation::tests::watch` | every outcome through the real step, odds parity, determinism, save round-trip |
| unit | `sim::tests::checksum` | no behaviour change (steps 3–5) |
| unit | `ui::screens::s17_hunts`, `ui` | geometry, CP437, slots, pins, keys, S01/S11 effects |
| components | `tests/components.rs` via `docs/components/ribbon.md` | the Ribbon, cell for cell |
| snapshot | `docs/screens/renders/S17a.txt`, `S17b.txt` | visual regression |
| chunk | `predators` (background) | the hunt path still balances |

## Risks and how the plan handles them
- **Timeout is rare** because the replan resets `chase_start_tick` every six ticks; the trace
  keeps its first Chase tick, and the timeout test uses a slow wolf with `chase_max_ticks = 3`.
- **Same-tick open and close**: `end` is an upsert.
- **Death detection is one tick late** for midnight deaths (age, disease); acceptable for a
  four-tick STARVED row.
- **Argument limits**: `Ledgers` replaces `tallies` rather than joining it; `odds` has six
  parameters exactly.
- **Checksum locks**: nothing here draws RNG or changes a decision; steps 3–5 are gated on
  the unchanged value.
- **Row identity on the 44-row harness**: the layout derives from the area height, so the
  render differs from the terminal only by one blank row.

## Follow-ups not in this plan
- Pins sorting first in S04's individuals and marked in S08's tree.
- The Ribbon reused in S01e's Following sidebar and S03b's Hunt stats.
- A wrapping `Text` option to replace `common::wrap` and the two other private copies
  (`load_world.rs`, `s07_log/chronicle.rs`).
