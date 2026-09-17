# AGENTS.md — Sim Fortress

Guidance for agents working in this repository. It is a router into `docs/`, not a
replacement for it. Read this first, then the doc that matches your task.

## What this is

`sim-fortress` is a Rust 2021 (pinned toolchain **1.90**) terminal predator/prey/
evolution simulation with a Dwarf-Fortress-inspired ratatui 0.30 UI. A configurable
roster of species (six by default: voles, hares, deer, foxes, wolves, lynxes) lives on
a procedurally generated map and evolves a twelve-trait genome. It is an internal
application (`publish = false`) with no real downstream users, so there is freedom to
change internals — but determinism and save format are load-bearing.

```
src/lib.rs       crate root: modules + the `cast!` macro
src/main.rs      hand-rolled CLI: --headless, --seeds, --summary, --profile, live app
src/sim/         pure, deterministic core; no terminal code
src/ui/          ratatui layer: App/AppState (ui/app.rs), screen stack, screens
src/widgets/     shared drawing: the components (docs/components) and the helpers that wrap them
src/theme.rs     truecolor palette, ramps and styles
src/glyphs.rs    named CP437 glyph constants
tests/           integration acceptance tests (one file per chunk)
examples/        throwaway balance/bench/diagnostic dev tools
docs/            the specification: roadmap chunks, screen requirements, references
scripts/         sweep.sh (parallel seed sweep), affected-tests.sh, hooks/ (Claude Code)
justfile         named test tiers — see "Tests are tiered"
```

Time model (fixed): 1 tick = 1 simulated hour, 24 ticks/day, 90 days/season,
360 days/year. A season is announced as a `Season` event.

## Commands

```sh
cargo build                                   # dev profile is opt-level 1
cargo run                                     # live app (title → New World → play)
cargo clippy --all-targets                    # MUST be 0 warnings
just check                                    # clippy + 800-line guard, seconds
just test-unit sim::disease                   # cargo test --lib <filter>
just test-chunk predators                     # one tests/*.rs binary, in release
just test-affected                            # the tests the current diff touches
```

## Tests are tiered — run the narrowest tier that covers the change

The lib suite takes ~95 s (save round-trip dominates) and each acceptance binary
in `tests/` simulates years of world time, so an unfocused `cargo test` costs
minutes and tells you nothing a focused run would not. A PreToolUse hook
(`scripts/hooks/guard-cargo-test.sh`, wired in `.claude/settings.json`) rejects
bare `cargo test`, unfiltered `cargo test --lib`, and slow chunks run without
`--release`. The `justfile` names the tiers:

| Tier | Command | Cost | When |
|---|---|---|---|
| gate | `just check` | seconds | always, before calling anything done |
| unit | `just test-unit <module::path>` | seconds to tens of seconds | the module you edited (`sim::disease`, `ui`, `sim::params`) |
| components | `cargo test --test components` | seconds | any edit under `src/widgets/` or `docs/components/`: every sheet example is rendered cell for cell |
| chunk | `just test-chunk <name>` | 1–3 min in release | the chunk your change affects; **run in the background** |
| affected | `just test-affected` | varies | when the diff spans modules; `just test-affected-plan` prints the plan first |
| lib | `just test-unit-all` | ~95 s | **only when the user explicitly asks** |
| full | `just test-full` | several minutes | **only when the user explicitly asks** |

Rules:

- **Never run the full or whole-lib suite on your own initiative.** "Make sure
  the tests pass" means the tiers above. If you believe a full run is warranted,
  say so and let the user ask for it; the hook prompts them on `just test-full`.
- **Run chunk tests in the background** (`run_in_background`) and keep working;
  read the result when it arrives. Do not sit and wait on a multi-minute run.
- **Do not re-run a green tier** unless a file it covers changed since.
- **Filter by module path.** `cargo test --lib sim::behavior` runs every unit
  test under that module. Failing test names are full paths, so a failure tells
  you the filter for the re-run. The determinism tripwire is
  `just test-unit sim::tests::checksum`.
- **Slow chunks are debug-hostile** (`predators`, `evolution`, `disease`,
  `sweep`); `just test-chunk` already passes `--release`. The 20-seed sweep
  (`cargo test --release --test sweep -- --ignored`) is never part of any tier.
- **Source-to-test mapping** (`scripts/affected-tests.sh`): `src/sim/disease*` →
  `disease`; `genetics*`/`lineage` → `evolution`; `predation`, `behavior/hunt`,
  `behavior/threat`, `behavior/migration` → `predators`; other `behavior/*` →
  `herbivores`; `ecology` → `ecology`; `params*`, `save`, `main.rs` → `headless`;
  the shared sim primitives and `Cargo.toml` fan out to every chunk. Update the
  script's table when you add a module.

Headless experiments and tooling:

```sh
cargo run -- --headless --seed 42 --years 5            # prints checksum + last events
cargo run -- --headless --seed 42 --ticks 4320
cargo run -- --seeds 1-20 --years 10 --summary         # in-process sweep → summary.csv
cargo run -- --dump-params                             # defaults with one comment per field
cargo run -- --params params.toml                      # partial TOML deep-merged over defaults
cargo run -- --params boar.toml --headless --seed 42 --years 2 --summary   # a [[species]] overlay: new animal, no rebuild
cargo run --release -- --headless --seed 1 --ticks 100000 --profile   # per-system timings
scripts/sweep.sh 1 20 10                               # parallel processes → summary.csv
cargo run --release --example bench_c5 -- 42 5 quiet   # dev benches in examples/
```

Refresh the screen snapshots a UI change touches (writes `docs/screens/renders/*.txt`
from a 155×45 `TestBackend`):

```sh
cargo test --lib -- --ignored regenerate_screen_renders
```

`params.toml` in the working directory is applied automatically and `--params FILE`
overlays it; **saved parameters win on load**, so `--params` is ignored there. UI
options are separate: `~/.config/sim-fortress/ui.toml` (or `$XDG_CONFIG_HOME/...`).

## Hard invariants — enforced by tests and lints

Do not weaken these to make a change pass. Fix the change.

- **Determinism.** Same seed + params must produce an identical run, tick for tick.
  A step is ordered: season event → creature behaviour → midnight boundary → disease
  → ecology → migration/extinction. **Never reorder or merge RNG draws.** Three RNG
  streams exist (`rng`, `creature_rng`, `disease_rng`) so enabling one subsystem does
  not perturb another. `sim::tests::checksum_is_fnv_stable` pins the exact checksum
  `0xd0e3_ee1a_c665_f531`; only re-baseline deliberately, with a comment saying why.
- **Sim/UI separation.** `src/sim` is pure data and logic: **no `ratatui`, `HashMap`
  or `HashSet`** anywhere under it. `src/sim/mod.rs` guards this by scanning every
  `.rs` file there for those substrings — **comments and strings included** — skipping
  only `mod.rs` itself. The UI only reads sim state through `&Sim` and sends commands
  (pause, speed, step, select). Every simulation feature must be reachable headlessly.
- **800-line file ceiling.** No file under `src/` may exceed 800 lines
  (`tests/file_size.rs`); the allowlist was deleted, so the ceiling is absolute and
  the only remedy is to split. Use the façade pattern: a root file with a module doc,
  `mod` declarations, `pub use` re-exports and top-level orchestration, plus sibling
  submodules (`s01_map.rs` + `s01_map/`, no `mod.rs`). Target **200–450 lines** per
  module. Extract `#[cfg(test)] mod tests;` to a child of the defining module so
  `use super::*` and private access keep working.
- **Determinism-preserving refactors are pure code motion.** When splitting `src/sim`,
  move bodies verbatim; never change a signature, deduplicate, or move a draw. The
  checksum test is the tripwire.
- **CP437 glyphs only.** Use constants from `src/glyphs.rs` (add new ones there;
  `tests::all_glyphs_are_cp437` checks every glyph is CP437 and one cell wide). No
  braille, no eighth-blocks, no `❄ † ‖ ☾ ✓ ✗` extras. ratatui `Chart`/`Canvas` must
  use `Marker::HalfBlock`, `Marker::Block` or `Marker::Dot` — never `Marker::Braille`.
  The one exception is a species' `glyph` in the `[[species]]` roster, which
  `Params::validate` checks is an ASCII lowercase letter (adults are drawn uppercase).
- **Colours come only from `src/theme.rs`** — the palette, the ramps (`heat`, `veg`,
  `water`, `parasite`, `species_ramp`), `lerp`/`dim`/`night`, and the styles
  (`text`, `dim_text`, `title`, `key`, `label`, `border`, `border_focus`, `selected`).
  The one exception is a species' `color = [r, g, b]` in the `[[species]]` roster; the
  UI turns it into a `Color` through `ui::style::SpeciesStyle` on `Roster`.
- **Species are data, not code.** `Params.species` (`[[species]]`, `src/sim/params/species.rs`)
  is the only place a species' name, plural, glyph, colour, kind, diet, founding count,
  life-history numbers, prey preference and base genome live. A `SpeciesId` is just a
  roster index; every per-species table in the sim is a `Vec` in roster order. Code
  that needs a species fact takes `&Roster` (`sim.roster()`), never a match on a name.
  Adding an animal is a TOML overlay: `[[species]] name = "boar" plural = "Boars"
  glyph = "b" color = [120, 90, 60] initial_count = 40` plus `[species.base_genome]`
  and, for predators, `[species.prey_preference]` keyed by prey name. Overlays merge
  arrays of named tables (`[[species]]`, `[[disease.pathogens]]`) **by `name`**: a
  known name edits that entry in place, a new name appends, and an entry cannot be
  removed (set `initial_count = 0` instead), so existing indices never move.
- **Parameters, not constants.** Every number that tunes behaviour lives in
  `sim::Params` with a documented default and **must** have a matching entry in
  `FIELD_DOCS` (`src/sim/params/docs.rs`); `params::tests::field_docs_complete` checks
  the table against the serialized struct in both directions. `--dump-params` emits the
  table as comments. Presets are overlays in `src/sim/params/presets.rs`.
- **All numeric casts go through `cast!(expr => Ty)`** (defined in `src/lib.rs`).
  Bare `as` is denied; the macro is the one reviewed place that preserves `as`
  semantics and works in `const` context.
- **Save format is versioned and never migrated.** Binary `SIMF` files, `VERSION = 11`
  in `src/sim/save.rs`. A version mismatch is rejected (`SaveError`), never half-read;
  old files stay listable so they can be deleted. `Params`/`Sim` serde field order and
  attributes are load-bearing — changing them means bumping `VERSION` and accepting
  that existing saves no longer load.
- **Clippy is strict.** `Cargo.toml` enables pedantic + nursery and denies
  `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `indexing_slicing`,
  `integer_division`, `float_cmp`, `mem_forget`, `lossy_float_literal`,
  `cognitive_complexity` and `too_many_lines`. `clippy.toml` sets an 80-line function
  limit, cognitive complexity 15, 6 arguments, 3 struct bools, and disallows the names
  `foo`, `bar`, `baz`, `tmp`, `temp`. `src/lib.rs` carries a justified crate-wide allow
  list (numeric hot loops, `indexing_slicing` from checked `0..len()` loops, and
  float-math lints whose suggestions would change simulation results). **Integration
  test and example crates are separate compilation roots** — they do not inherit that
  list, so each adds its own `#![allow(...)]` with the same justification.

## Workflow and conventions

- **The docs are the spec.** `docs/chunks/` is the C1–C8 delivery roadmap; each chunk
  doc has scope, requirements, measurable acceptance criteria and a checkpoint demo.
  `docs/chunks/VALIDATION.md` records the review that made them unambiguous. Do not
  start a chunk before the previous checkpoint is accepted. When you change behaviour,
  update the chunk doc's balance table/results in the same change.
- **`docs/screens/`** holds per-screen requirements and `renders/<id>.txt` snapshots;
  `docs/PROTOTYPE_GUIDE.md` is the accurate guide to the current UI tree (its filename
  is historical — there are no static prototypes any more). `docs/PERFORMANCE.md`
  records the measured budget; `docs/file-split-plan.md` records how the 800-line
  ceiling was reached. `docs/feature-ideas.md` is the prioritised backlog. `docs/ai-requirements.md` is binding
  for any feature that sends a prompt to a language model (opt in, optional, never in the step).
- **Adding or changing a screen.** Screens live in `src/ui/screens/sNN_*.rs` and
  implement `Screen` (`opaque`, `handle_key`, `render`). The **top screen sees every
  key first** and returns `Action::Unhandled` for keys it does not consume; only then
  does the global table in `src/ui/screens/mod.rs` apply. Register the module there,
  keep the status bar (`widgets::StatusBar`) and `?` help honest, and refresh the affected
  renders. Modals return `opaque() == false` so the stack dims what is beneath them.
- **Tests.** Acceptance criteria live in `tests/` (one file per chunk); unit tests live
  in `src/**/tests.rs`. Prefer extending the existing acceptance test over inventing a
  parallel suite. Run them by tier (see above), never the whole suite unprompted.
- **State of the tree.** Work happens on `main`; `claude/*` branches and
  `.claude/worktrees/` are other agents' in-flight work — leave them alone. Check
  `git status` before editing.

## Known-open items — not new bugs

- **Balance / population bands are open (C5/C6).** Under default parameters voles have
  no refuge (foxes eliminate them by year 1–2) and wolves/lynxes struggle to find
  mates; predator populations collapse by year 5. The `#[ignore]`d tests state this in
  their reasons: `predators::{six_species_five_years, oscillation_lag,
  hunt_success_band}`, `evolution::dry_world_selection_7_of_10`,
  `disease::spillover_is_rare_but_real`, `sweep::twenty_seed_survival`. Do not "fix" a
  failing ignored test by deleting it; the fix is a mechanism (e.g. the dens idea in
  `docs/feature-ideas.md`).
- **The performance budget is not met.** Headless measures ≈590 ticks/s at ~1 000
  creatures against a 3 000 ticks/s target, and the x25 UI budget is far off; see
  `docs/PERFORMANCE.md`. The behaviour tick dominates.
- **C7 and C8 docs are drafted, not yet validated** (`docs/chunks/README.md` status
  table). Their implementation exists but has no recorded acceptance run.

## Gotchas learned in this repository

- **Submodule name collisions.** A child module named `disease` where
  `crate::sim::disease` or `use crate::sim::disease::{self, ..}` is bound breaks
  unqualified paths — hence `sim/params/pathogen.rs` and `ui/screens/s01_map/disease_overlay.rs`.
  Likewise `params/toml.rs` would shadow the `toml` crate (`toml_util.rs`), and
  `behavior/predation.rs` would shadow the sim module (`threat.rs` + `hunt.rs`).
- **`clippy::redundant_pub_crate`.** A `pub(crate)` item defined in a *non-public*
  submodule always trips it; marking the submodule `pub(crate)` does not help. Define
  crate-facing items in the root module.
- **Sibling modules do not inherit privacy.** Shared helpers need `pub(super)` and
  belong in the **root**, not in a section module (a child can reach a parent's private
  items; siblings cannot reach each other's).
- **`clippy` caches.** `touch src/lib.rs` (or `cargo clean -p sim-fortress`) before
  trusting a clean clippy run — a cached run once masked three real warnings.
- **Test lint attributes must propagate.** Extracting a test module leaves the denied
  lints firing; re-apply `#[allow(clippy::float_cmp)]` etc. on each new test module.
- **Doc drift is real.** A stale `PROTOTYPE_GUIDE.md` once caused an agent to cite a
  function that no longer existed. Verify claims against `src/` before repeating them,
  and correct the doc when it is wrong.
- **Run from the repository root.** `rust-toolchain.toml` pins stable 1.90 with
  clippy and rustfmt; the crate is both a library (used by `tests/`) and a binary.

## Before you call a change done

```sh
just check                                      # 0 clippy warnings, no src file over 800 lines
just test-unit <module::path>                   # the modules you touched; checksum unchanged
just test-chunk <name>                          # the affected chunk(s), in the background
git diff --stat                                 # only the files you meant to touch
```

If you touched `src/sim`, re-run `cargo test --lib sim::tests::checksum_is_fnv_stable`
and confirm the value is still `0xd0e3_ee1a_c665_f531` unless the change deliberately
re-baselines it.
