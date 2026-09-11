# C6 — Persistence, title flow, balance and tooling

Back to the [roadmap](README.md). Previous: [C5](c5-predators.md).

## Goal
Make Sim Fortress a complete, shippable loop: start from a title screen, save and load
worlds, tune parameters without recompiling, run headless experiments at scale, and meet
the performance budget. Remove the prototype scaffolding.

## Checkpoint (what the user sees)
Title screen → New World or Load World → play → save with a key → quit → reload and
continue from the same tick with identical subsequent behaviour. A `params.toml` in the
working directory changes the balance. A batch script runs 20 seeds headless and produces
a summary table.

## Scope

### In
- `sim::save` with `postcard` + `serde` (all `sim` types derive `Serialize/Deserialize`
  since C1; the spatial index is rebuilt on load).
- **S00 Title**, **Load World list**, **confirm modal**, **Options modal** (specified in
  FR3–FR5 because they have no prototypes).
- `--params`, `--dump-params` (with per-field comments), preset overlays, S09 Presets
  row live.
- Headless sweep tooling and `scripts/sweep.sh`.
- Balance pass and `docs/PERFORMANCE.md`.
- Removal of `src/prototypes` and `src/fixtures`; text renders of each prototype kept as
  `docs/screens/renders/<id>.txt` (rendered with `ratatui::backend::TestBackend` before
  deletion, diffable).

### Dependencies added
`postcard = { version = "1", features = ["alloc"] }`, `toml_edit = "0.22"`.

### Out
Networking, mod support, non-terminal frontends, backward-compatible save formats before 1.0.

## Dependencies
- [C5](c5-predators.md) complete.
- Screen requirements: [S00](../screens/s00-title.md), [S09](../screens/s09-world-generation.md),
  [S10](../screens/s10-simulation-controls.md).

## Functional requirements

### FR1 Save format
File = `magic [u8; 4] = b"SIMF"`, `version: u16`, `header_len: u32`, postcard-encoded
`SaveHeader { seed, tick, season_days, start_hour, saved_at_unix, world_name, counts: [u32; 6],
strip_rows: [Vec<u8>; 4] (terrain codes of world rows round(H × {0.45, 0.475, 0.75, 0.775}),
central min(120, W) columns padded with a blank code to 120, for the S00 terrain strips) }`,
then the postcard-encoded `Sim`. A newer `version` fails with `save is from a newer
version (N > M)`. `saves/` is relative to the cwd, overridable by `--saves-dir`, created
on first save. Filename = lowercase world name with non-`[a-z0-9]` runs replaced by `-`,
then `-<tick>.simf`; autosaves are `<slug>-autosave.simf`, overwritten. `F5` saves, `F9`
quick-loads the newest save for the current world name after a confirm modal;
`params.ui.autosave_days` (0 = off). `dirty = sim.tick != last_saved_tick`.

### FR2 Round trip
`load(save(sim))` then N ticks equals `sim` then N ticks by checksum for N = 10 000 and
seeds 1..=3. `sim` uses only `Vec`/`BTreeMap` (C3 test) so iteration order survives; the
rng state is serialised.

### FR3 Title screen and Load list
S00 as the prototype; Load World opens a 70×20 Focus-bordered modal titled `Load World`
over S00 listing saves newest first, one row `<name>  Year y, Day d  <prey> prey / <pred>
predators  <saved_at>`; `↑↓` select, `Enter` load, `Del` delete (confirm), `Esc` back; 14
rows then scroll. New World → S09. Options → FR5. Quit → confirm modal if a world is
loaded and dirty. `q` on any screen with a loaded world: confirm if dirty, then replace the
stack with S00 (supersedes C1 FR8). The "last world" summary and terrain strips come from the newest save's
header (defaults when none).

### FR4 Confirm modal
50×7 centred, one question line, `[ Yes ]  [ No ]`, `←→`/`Tab` move, `Enter` select,
`Esc` = No. Used for quick-load, delete save, quit with unsaved changes.

### FR5 Options modal
S10 layout enlarged to 60×20 with the Options section (rows 9–14) extended to five rows: auto-pause on extinction, log
births, pause when a followed creature dies, autosave every `◄ N ►` days, day/night tint
on/off. Persisted to `$XDG_CONFIG_HOME/sim-fortress/ui.toml`, default
`~/.config/sim-fortress/ui.toml` on every Unix including macOS (no `dirs` crate). `ui.toml`
overrides `params.ui` from a loaded save.

### FR6 Params files, overlays and dump
`Params` structs carry `#[serde(deny_unknown_fields)]`. An **overlay** is a partial TOML
table deep-merged onto the default table before deserialising; `--params` and presets are
overlays. `params.toml` in the cwd is applied automatically if present. When loading a
save the saved `Params` win and `--params` is ignored with a `Note` event.
`--dump-params` writes the defaults via `toml_edit`, inserting one comment per leaf from
`Params::field_docs() -> &[(path, doc)]`; a test asserts every leaf has a doc and that
parsing the dump equals `Params::default()`.

### FR7 Presets
`predation.difficulty` (stored since C5) gets its meaning here: `Params::effective_kill_base()`
= `kill_base + {easy +0.10, normal 0, hard −0.10}` and `effective_detect_threshold()` =
`detect_threshold + {easy −0.1, normal 0, hard +0.1}`, read at the use sites; stored values
are never mutated (so save → load cannot double-apply). Presets (S09 row live; selecting writes into the form):

| Preset | Overlay |
|---|---|
| Balanced | defaults |
| Harsh winter | `time.season_days = 180`, `ecology.regrowth_rate = 0.6` |
| Lush | `world.forest_pct = 30`, `ecology.regrowth_rate = 1.4`, `predation.difficulty = "hard"` |
| Archipelago | `world.water_pct = 55` |
| Fast evolution | `genetics.mutation_rate = 0.10`, `genetics.mutation_strength = 0.12` |

### FR8 Headless tooling
`--seeds 1-20` (inclusive; also `1..20`), `--years N` (overrides `--ticks`), `--summary`
prints and writes `summary.csv`: `seed, years, final counts × 6, extinctions, lag_days
(stats::peak_lag), mean_speed_by_species × 6`. `scripts/sweep.sh <first> <last> <years>`.
`--profile` prints time per system per 1 000 ticks.

### FR9 Performance budget
This chunk includes an optimisation pass, ordered: `--profile` first, then the hot spots it
reveals (expected: perception bucket scans, the predator-first flee query, census). Targets:
UI ≥ 30 FPS at x25 with 3 000 creatures on 200×60; headless ≥ 3 000 ticks/s at 150×40 with
1 000 creatures. `scripts/sweep.sh` runs seeds in parallel with `xargs -P $(nproc)`.
Measured numbers, machine and method recorded in `docs/PERFORMANCE.md`.

### FR10 Cleanup
Delete `src/prototypes`, `src/fixtures` and the `--prototypes` flag; widgets take
`&sim::World` / `&sim::Creature` only (no `fixtures` symbol remains); the CP437 glyph test
stays; each `docs/screens/*.md` gains a `Live since: C<n>` line; S11 gains `F5`/`F9` rows;
README describes the game.

## Acceptance criteria
- Save/load round trip (FR2) passes; a 150×40 save with 2 000 creatures is < 10 MB
  (names are `NameId`s, not strings); save and load each < 500 ms.
- Title → New → play → `F5` → `q` → Load → continue: checksum after 1 000 more ticks equals
  an unsaved run.
- Determinism and `tests/predators.rs::six_species_five_years` pass with every preset except
  Archipelago (documented expected species loss); numeric bands are required for Balanced
  only.
- 20-seed sweep, 10 years each, completes in under 20 minutes wall-clock (parallel) on the
  reference machine and shows at least five species alive at year 10 in at least 14 seeds
  (aligned with C5's seed-42 criterion).
- Performance budgets met and recorded; `cargo build` has no reference to `prototypes` or
  `fixtures`.

## Checkpoint demo script
1. `cargo run` → S00. New World → S09 → Generate → a year at x25. `F5` saves.
2. `q` → confirm → S00; Load World → the save → play continues; ticker and counts match.
3. Write `params.toml` in the cwd with `[predation] kill_base = 0.15`; restart; New World
   with the same seed; predators starve more (S04 deaths/day).
4. `scripts/sweep.sh 1 20 10` → summary table.
5. `cargo run -- --profile --headless --seed 1 --ticks 100000` → per-system timings.

## Tests
- `sim::save::tests::{header_only_read, newer_version_rejected, round_trip_checksum_3_seeds, autosave_interval,
  filename_slug}`
- `sim::params::tests::{unknown_key_errors, dump_params_round_trip, preset_overlay_merge, field_docs_complete}`
- `ui::tests::{title_menu_navigation, load_list_sorted, confirm_modal_keys, options_persist}`
- `tests/sweep.rs` (ignored by default, nightly): 20-seed survival criterion.

## Decisions made here
- `postcard` binary saves with a version header; no compatibility promise before 1.0.
- UI options and simulation params are separate files; saved params win over CLI overlays.
- XDG-style config path on all Unix platforms.

## Risks
- Non-determinism after load from container ordering or rng state: the round-trip test is
  the first task in this chunk.
- Deleting the prototypes removes the visual reference; the text renders replace it.
