# File-split plan & progress

**Goal.** No file under `src/` exceeds **800 lines**, and every module has a single
clear purpose.

**Enforcement.** `tests/file_size.rs` caps every file under `src/` at **800 lines**.
During the refactor it was a *ratchet*: an `OVER_BUDGET` allowlist grandfathered the
still-large files at their then-current size and could only shrink, so the work
could land one file at a time. That list has now been deleted, so the ceiling is
absolute — the only way to satisfy it is to split a file.

**Baseline** (measured before Wave 0): 9 files / **13,231 lines** = 51% of the
25,853 lines under `src/`.

**Current state** (after Wave 5): **no file under `src/` exceeds 800 lines** — the
original nine offenders are all split, and the `OVER_BUDGET` allowlist has been
deleted so the ceiling is absolute.

```
   2721 ./src/sim/behavior.rs         1005 ./src/ui/screens/s04_species.rs
   2495 ./src/ui/screens/s01_map.rs    974 ./src/ui/screens/s03_inspector.rs
   1561 ./src/sim/disease.rs           961 ./src/sim/genetics.rs
   1347 ./src/ui/screens/s05_charts.rs 859 ./src/sim/stats.rs
   1308 ./src/sim/params.rs
```

`cargo clippy --all-targets` → **0 warnings** · `cargo test` → **green** ·
determinism pinned by `sim::tests::checksum_is_fnv_stable` (that value is
re-baselined deliberately whenever behaviour changes).

---

## Why a test and not a clippy rule

`clippy::too_many_lines` is already denied at an 80-line threshold, but it measures
**functions**, not files: `src/sim/behavior.rs` is 2,721 lines and passes clippy
cleanly today. There is **no clippy lint for file length** — probed on the pinned
1.90 toolchain, `clippy::too_many_lines_in_file`, `clippy::excessive_file_length`
and `clippy::file_length` are all unknown lints, while `clippy::too_many_lines` is
known. Upstream: [rust-clippy#16674] (open request for this lint) and
[rust-clippy#15922] (unmerged PR proposing `excessive_file_length`).

So the ceiling is enforced by a test that walks `src/`, mirroring the existing
structural guards `no_ratatui_in_sim` / `no_hashmap_in_sim` in `src/sim/mod.rs`.

[rust-clippy#16674]: https://github.com/rust-lang/rust-clippy/issues/16674
[rust-clippy#15922]: https://github.com/rust-lang/rust-clippy/pull/15922

**Function-level clippy stays as-is.** Lowering `too-many-lines-threshold` from 80
to 60 was measured to produce **30 new violations**, so it is separate follow-up
work (Wave 6), not part of this refactor.

---

## Gate after every step

```sh
cargo clippy --all-targets                            # must stay at 0 warnings
cargo test                                            # must stay green; checksum unchanged
find ./src -type f -exec wc -l {} + | awk '$1 > 800'  # must be strictly shrinking
```

…plus deleting the matching `OVER_BUDGET` entry **in the same commit**.

## Refactor rules

- Sibling-directory layout, no `mod.rs`: `src/sim/behavior.rs` +
  `src/sim/behavior/perception.rs` (edition 2021).
- **Pure code motion.** Never reorder or merge RNG draws, never change a
  signature, never deduplicate. `checksum_is_fnv_stable` is the tripwire.
- The parent file becomes a façade: module doc + `mod` declarations + `pub use`
  re-exports + top-level orchestration. This preserves `crate::sim::X` and
  `sim::stats::*` paths for the UI, `main.rs` and the integration tests.
- Target **200–450 lines** per new module; split a cluster again rather than
  shipping a 700-line "smaller" file.
- Extract `#[cfg(test)] mod tests;` to a child file of the *defining* module, so
  `use super::*` keeps working and private access is preserved.
- New sim files are still scanned by `no_ratatui_in_sim` / `no_hashmap_in_sim`,
  which match **substrings** — even in comments.

---

## Progress

| Wave | Scope | Files | Status |
| ---- | ----- | ----- | ------ |
| 0 | Ratchet guard | `tests/file_size.rs` | ✅ done |
| 1 | Tests-only extraction | `sim/genetics.rs`, `sim/stats.rs` | ✅ done |
| 2 | UI screens (no sim risk) | `s05_charts`, `s04_species`, `s03_inspector` | ✅ done |
| 3 | sim data / serde risk | `sim/disease.rs`, `sim/params.rs` | ✅ done |
| 4 | Largest UI file | `s01_map.rs` | ✅ done |
| 5 | Highest risk (determinism) | `sim/behavior.rs` | ✅ done |
| 6 | Finalize | ceiling absolute, stale guide rewritten, cleanups done; optional tightening measured | 🔶 stretch left |

---

## Wave 0 — ratchet guard ✅

- [x] Add `tests/file_size.rs` (`MAX_LINES = 800`, `OVER_BUDGET` allowlist).
- [x] Three assertions: per-file budget, no stale/regrown entries, list stays sorted.
- [x] Confirm the suite still passes with all 9 files grandfathered.
- [x] **Verified the guard actually bites**: injecting a 900-line `src/tmp_too_big.rs`
      fails `no_source_file_exceeds_its_budget` with
      `src/tmp_too_big.rs is 900 lines (budget 800); split it into modules…`
      (probe removed afterwards).

## Wave 1 — tests-only extraction ✅

Zero-risk warm-up: move the trailing `#[cfg(test)] mod tests` out of each file.
Both files drop under 800 on this alone, so their `OVER_BUDGET` entries are deleted.

| File | Before | After | Extracted |
| ---- | ------ | ----- | --------- |
| `src/sim/genetics.rs` | 961 | **592** | `src/sim/genetics/tests.rs` (369) |
| `src/sim/stats.rs` | 859 | **552** | `src/sim/stats/tests.rs` (307) |

- [x] `#[cfg(test)] #[allow(clippy::float_cmp)] mod tests;` left in each parent, so
      the child module keeps `use super::*` and the test-only lint allowances.
- [x] Production bytes verified **identical** to `HEAD` (first 588 / 548 lines diff
      clean); extracted test bodies verified **verbatim** (non-blank line compare).
- [x] Both `OVER_BUDGET` entries deleted — the ratchet tightened with the change.
- [x] `cargo clippy --all-targets` → 0 warnings; `cargo test` → **green, exit 0**
      (182 lib tests passed / 1 ignored, all integration binaries green, 6 ignored);
      `cargo test --test file_size` → 3 passed.

Optional, lower value (production is already 591 / 551 lines): `genetics/delivery.rs`
(~340: `newborn`, `deliver`, `LitterCtx`, `deliver_litter`, `parent_genome`,
`litter_cells`, `bear_pup`, `credit_parents`, `announce_litter`) and
`stats/{census,species,series,lag}.rs` behind a façade. Not required for the ceiling.

## Wave 2 — UI screens ✅

No simulation-determinism risk, so these went before the sim files. Actual result
(every child is under 350 lines):

| Screen | Before | Root | Children |
| ------ | ------ | ---- | -------- |
| `s05_charts.rs` | 1347 | **241** | `time.rs` 259 (S05a) · `phase.rs` 224 (S05b) · `stacked.rs` 303 (S05c) · `infections.rs` 285 (S05d) · `tests.rs` 93 |
| `s04_species.rs` | 1005 | **200** | `table.rs` 157 (S04a) · `summary.rs` 343 (S04a) · `histograms.rs` 102 (S04b) · `drift.rs` 202 (S04b) · `tests.rs` 61 |
| `s03_inspector.rs` | 974 | **228** | `identity.rs` 322 · `genome.rs` 138 · `life.rs` 274 · `style.rs` 40 · `tests.rs` 39 |

- [x] Every original top-level item verified **verbatim** in exactly one new file
      (47 / 37 / 32 items); test bodies moved with only their imports re-homed.
- [x] `cargo clippy --all-targets` → 0 warnings; `cargo test` → **green, exit 0**
      (182 lib tests passed / 1 ignored, including the S01–S13 render snapshot
      tests in `src/ui/tests.rs`; all integration binaries green);
      `cargo test --test file_size` → 3 passed.
- [x] Three `OVER_BUDGET` entries deleted.

**Deviations from the plan, and why:**

- **`s03_inspector`: no `geo.rs`.** Its items are all `pub(crate)`, and
  `clippy::redundant_pub_crate` (nursery, warn) rejects a `pub(crate)` item inside
  a *private* submodule. Those six helpers (`killer_and_scavengers`, `kin_name`,
  `compass`, `contagion_risk`, `local_forage`, `clip`) therefore stayed in the root
  module, which also keeps their `s03_inspector::*` paths for `s01_map.rs` intact.
  `style.rs` holds only the private `sp` / `species_style` / `trait_color` /
  `delta_style`.
- **`s04_species`: no `sort.rs`.** `SortCol`, `impl SortCol` and the `pub fn
  sorted_indices` stayed in the root, so the paths used by `screens/mod.rs` tests
  and `app.rs` are unchanged. `selection_pressure` moved to `drift.rs` and is
  re-exported with `pub use`.
- **Shared helpers went to the root**, not to a section module: `round_up` and
  `stats_of` for s05. A child module can reach a parent's private items, so no
  visibility bump was needed for those.

**Tooling notes for later waves:**

- `cargo fix` silently skips *partial* import-group removals (rustc emits a span but
  no replacement). A span-precise pruner is needed; see the Wave 3/4 procedure.
- A `{self}` import must not be collapsed out of its braces.

## Wave 3 — sim data / serde risk ✅

| File | Before | Root | Children |
| ---- | ------ | ---- | -------- |
| `disease.rs` | 1561 | **31** (pure façade) | `types.rs` 196 · `effects.rs` 92 · `contagion.rs` 147 · `spillover.rs` 169 · `lifecycle.rs` 116 · `daily.rs` 135 · `outbreaks.rs` 163 · `emergence.rs` 131 · `tests.rs` 475 |
| `params.rs` | 1308 | **88** | `world.rs` 130 · `creatures.rs` 94 · `predation.rs` 178 · `genetics.rs` 100 · `social.rs` 54 · `ecology.rs` 89 · `pathogen.rs` 213 · `docs.rs` 195 · `presets.rs` 36 · `toml_util.rs` 51 · `tests.rs` 144 |

- [x] 46 + 42 items verified **verbatim**; all serde attributes, field order and
      `#[repr(u8)]` moved untouched (save compatibility preserved).
- [x] `pub use` façades keep every `crate::sim::disease::*` and
      `crate::sim::params::*` path working for `sim/mod.rs`, `main.rs`, the UI and
      the integration tests.
- [x] `cargo clippy --all-targets` → 0 warnings; `cargo test --lib` → 182 passed /
      1 ignored (the `no_ratatui_in_sim` / `no_hashmap_in_sim` guards now scan the
      new files too); `cargo test --test file_size` → 3 passed.
- [x] Two `OVER_BUDGET` entries deleted.

**Deviations from the plan, and why:**

- **`params/pathogen.rs`, not `params/disease.rs`.** A child module named `disease`
  would shadow `crate::sim::disease` for unqualified paths in that module.
- **`params/toml_util.rs`, not `params/toml.rs`.** A module named `toml` would
  collide with the external `toml` crate used by `deep_merge`.
- **`field_docs` is not a top-level item.** It is an associated fn of `impl Params`
  (see the tooling note below), so it moved with that impl block rather than into
  a module of its own.

**Important tooling fix — the splitter is now brace-aware.** The original
`params.rs` has a pre-existing indentation defect: `pub const fn field_docs` sits at
**column 0** even though it is inside `impl Params { … }`. A column-only splitter
treats it as a top-level item and silently re-parents it. The parser now tracks brace
depth across comments, strings, raw strings and char literals, and only column-0
declarations at depth 0 start an item. This is also why the same bug could not have
been caught by the verbatim check alone — it was caught by the compiler rejecting a
bogus `use super::field_docs;`.

Also handled: `pub(super)` is now applied to *impl methods* a sibling module calls
(e.g. `DiseaseState::push_outbreak`), while **trait impls are skipped**, since
visibility qualifiers are not permitted on their methods.

## Wave 4 — `s01_map.rs` (2495) ✅

| Before | Root | Children |
| ------ | ---- | -------- |
| 2495 | **415** | `follow.rs` 347 · `input.rs` 299 · `disease_overlay.rs` 280 · `base.rs` 173 · `sense.rs` 210 · `parasites.rs` 186 · `health.rs` 134 · `species.rs` 131 · `regions.rs` 127 · `overlays.rs` 110 · `look.rs` 50 · `tests.rs` 220 |

- [x] 40 whole items + **31 impl methods** verified verbatim in exactly one file.
- [x] `cargo clippy --all-targets` → 0 warnings; `cargo test --lib` → 182 passed /
      1 ignored; `cargo test --test file_size` → 3 passed.
- [x] `OVER_BUDGET` entry deleted — one file left.

**Deviations from the plan, and why:**

- **`disease_overlay.rs`, not `disease.rs`.** `s01_map.rs` imports the sim module as
  `use crate::sim::disease::{self, PathogenId};`, so a child module named `disease`
  would collide with that binding (9 unqualified `disease::` uses).
- **The `impl WorldMap` blocks had to be split by method.** Three separate
  `impl WorldMap` blocks exist, but the 2nd (502 lines) and 3rd (407 lines) mix
  concerns — `region_sidebar` and `select_overlay` are input/dispatch, while
  `sense_sidebar`, `disease_sidebar`, `parasite_sidebar` … are per-overlay rendering.
  Rust allows the same type to be implemented in several modules, so the splitter now
  regroups methods into one `impl WorldMap { … }` block per target module.
  `impl Screen for WorldMap` is a trait impl and stayed whole in the root.
- **`sidebar` (the dispatcher) stayed in the root** with `impl Screen`, so the root
  remains the orchestrator rather than a pure façade — hence 415 lines rather than ~330.

**Tooling notes:**

- Item names are no longer unique (three `impl WorldMap` blocks), so the generator
  disambiguates them (`impl WorldMap`, `impl WorldMap#2`, `#3`).
- Methods are a second namespace: they need `pub(super)` but must never be emitted as
  `use` imports, and `add_vis` must not be applied to indented method text.
- The verifier now checks impl **methods** individually, since a regrouped impl block
  is not verbatim as a whole.

## Wave 5 — `sim/behavior.rs` (2721) ✅

| Before | Root | Children |
| ------ | ---- | -------- |
| 2721 | **143** | `hunt.rs` 407 · `goals.rs` 391 · `movement.rs` 308 · `threat.rs` 205 · `migration.rs` 174 · `perception.rs` 155 · `death.rs` 137 · `vitals.rs` 104 |

Tests (841) split across `tests.rs` 230 (shared fixtures) + `tests_vitals.rs` 270 ·
`tests_social.rs` 176 · `tests_migration.rs` 93 · `tests_flee.rs` 92 ·
`tests_death.rs` 75.

- [x] 61 whole items + 1 impl method verified verbatim in exactly one file;
      714 test lines relocated verbatim across the five test modules.
- [x] **Determinism preserved**: `checksum_is_fnv_stable` and
      `round_trip_checksum_3_seeds` both pass — RNG call order unchanged.
- [x] `cargo clippy --all-targets` → 0 warnings; `cargo test --lib` → 182 passed /
      1 ignored.
- [x] `OVER_BUDGET` emptied and then removed outright — **no file exceeds 800**.

**Deviations from the plan, and why:**

- **`threat.rs` + `hunt.rs`, not a `predation/` subdirectory.** `behavior.rs`
  imports the sim module as `use crate::sim::predation::{self, MAX_SENSE_CELLS};`,
  so a child module named `predation` would collide with that binding.
- **Two re-exports are `#[cfg(test)]`.** `find_walkable_near` and `needs` are used
  *only* from other modules' tests, so a plain `pub(crate) use` is dead code in a
  non-test build and `cargo fix` correctly deletes it — which then breaks the test
  build. Gating them with `#[cfg(test)]` keeps both builds honest.
- **The test module needed splitting to fit**, not just extracting: fixtures stayed
  in `tests.rs` (marked `pub(super)`), and the 28 `#[test]` fns moved into five
  themed modules that import the fixtures via `use super::tests::{…}`.
- **Test lint attributes had to propagate.** `#[allow(clippy::float_cmp)]` sat on
  the single `mod tests`; every new test module needs it too, or the denied
  `float_cmp` lint fires.

**Tooling notes:**

- Private helpers used only by tests (`move_toward`, `maybe_die`, `pressure`,
  `replan`, `migrate_group`, `maybe_make_den`, …) needed `pub(super)` and explicit
  `use super::<module>::{…}` imports in the test modules.
- The pruner needed a wider span pattern: rustc's span for a whole-line unused
  import is the *bare path* (`crate::sim::geom`), and for a glob it is `super::*`.

## Wave 6 — finalize 🔶

- [x] Delete `OVER_BUDGET` entirely; the 800 ceiling becomes absolute.
      (`tests/file_size.rs` is now a single unconditional ceiling check — no
      exemption mechanism remains to be reintroduced.)
- [ ] Optionally lower `MAX_LINES` to 600 as a second ratchet.
- [ ] Optionally tighten `too-many-lines-threshold` (measured: **30 violations** at 60).
- [x] Fix the stale `PROTOTYPE_GUIDE.md` reference — and then the rest of it.
      The one-line fix was not enough: the whole doc described a `src/prototypes/`
      + `fixtures` + `Prototype` architecture that no longer exists, and it was
      what misled an earlier pass into citing a nonexistent `s01_map::render_base`.
      It is now an accurate guide to the current tree (filename kept, because
      `docs/screens/README.md` and `docs/screens/s01-world-map.md` link to it).
- [x] Re-indent `Params::field_docs` in `src/sim/params.rs` (it sat at column 0
      inside `impl Params` — a pre-existing defect the splitter now tolerates).
- [x] **Regression found while finishing:** Wave 5 left three
      `clippy::redundant_pub_crate` warnings that a cached clippy run had masked.
      `kill`, `needs` and `find_walkable_near` are `pub(crate)`, and a `pub(crate)`
      item defined inside a *non-public* submodule always trips the lint — marking
      the submodule `pub(crate)` does **not** help. The fix is the Wave 2 pattern:
      define crate-facing items in the root module. Regenerated with those three in
      `behavior.rs`; `behavior.rs` is 275 lines. **Always `touch src/lib.rs` (or
      clean) before trusting a clippy run to be warning-free.**
- [ ] **Optional stretch — lower `MAX_LINES` to 600.** Would require splitting six
      more files: `widgets/map.rs` 793, `s09_worldgen.rs` 761, `sim/mod.rs` 739,
      `s08_lineage.rs` 690, `screens/mod.rs` 655, `sim/ecology.rs` 601.
- [ ] **Optional stretch — tighten `too-many-lines-threshold`.** Measured against
      the current tree: **70 → 11 functions** over, **60 → 30** over.

---

## Risk register

1. **Visibility churn** — sibling modules do not inherit privacy; shared helpers
   need `pub(super)`. `kill` / `needs` / `find_walkable_near` stay `pub(crate)`.
2. **Determinism** — `contagion_pass`, `spillover`, `emerge`, the `replan*` draws
   and `deliver_litter` consume RNG in a fixed order. Move bodies verbatim.
3. **Save compatibility** — `Params` and `DiseaseState` are serialized `Sim`
   fields; `#[serde(default, deny_unknown_fields)]` and field order must not move.
4. **`FIELD_DOCS` adjacency** — `field_docs_complete` checks the table against
   struct fields in both directions; moving it to `docs.rs` removes that adjacency.
5. **`PRESETS` ordering is load-bearing** (`s09_worldgen.rs:255`).
6. **Import splintering** — expect transient unused-import / `wildcard_imports`
   clippy errors; prefer explicit imports over `use super::*` in new modules.
7. **The sim guards scan new files for substrings** and skip only `mod.rs`.
8. **Doc-comment misattribution** while cutting/pasting — `missing_docs` is not
   enforced, so this fails silently.
9. **Scope discipline** — s03's `clip` duplicates `common::clip`, and its
   `trait_color` / `delta_style` shadow `common::*` with a real behavioural
   difference (fallback `SICK` vs `DIM`). Dedup is a separate, snapshot-verified
   change, not part of a pure refactor.

## Log

| Date | Step | Result |
| ---- | ---- | ------ |
| 2026-09-13 | Baseline | clippy 0 warnings; 182 lib tests + integration green; checksum `0x348e3c6eeec2e6d6` |
| 2026-09-13 | Wave 0 | `tests/file_size.rs` added, 3 tests pass; guard proven to fail on a 900-line file |
| 2026-09-13 | Wave 1 | `genetics.rs` 961 → 592, `stats.rs` 859 → 552; tests extracted verbatim; production bytes unchanged; clippy 0 warnings; full `cargo test` green (exit 0); 2 `OVER_BUDGET` entries removed |
| 2026-09-13 | Wave 2 | `s05_charts` 1347 → root 241 + 5, `s04_species` 1005 → root 200 + 5, `s03_inspector` 974 → root 228 + 5; 116 items verified verbatim; clippy 0 warnings; full `cargo test` green (exit 0); 3 `OVER_BUDGET` entries removed; 4 files left |
| 2026-09-13 | Wave 3 | `disease` 1561 → root 31 + 9, `params` 1308 → root 88 + 10; 88 items verified verbatim; serde layout untouched; clippy 0 warnings; full `cargo test` green (exit 0); 2 `OVER_BUDGET` entries removed; 2 files left |
| 2026-09-13 | Wave 4 | `s01_map` 2495 → root 415 + 12; 40 items + 31 impl methods verified verbatim; clippy 0 warnings; full `cargo test` green (exit 0); 1 `OVER_BUDGET` entry removed; 1 file left |
| 2026-09-13 | Wave 5 | `behavior` 2721 → root 143 + 8 production + 5 test modules; 61 items + 1 impl method + 714 test lines verbatim; determinism checksum unchanged; clippy 0 warnings; full `cargo test` green (exit 0); `OVER_BUDGET` deleted — **0 files over 800** |
| 2026-09-13 | Wave 6 | `PROTOTYPE_GUIDE.md` rewritten as a current guide; `Params::field_docs` re-indented; three `redundant_pub_crate` warnings from Wave 5 fixed by moving `kill`/`needs`/`find_walkable_near` into the `behavior` root; fresh clippy 0 warnings; full `cargo test` green (exit 0); determinism checksum unchanged |
