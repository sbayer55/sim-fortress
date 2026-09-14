# File-split plan & progress

**Goal.** No file under `src/` exceeds **800 lines**, and every module has a single
clear purpose.

**Enforcement.** `tests/file_size.rs` — a *ratchet* test. `OVER_BUDGET` lists the
files still above the ceiling together with the size they had when the guard
landed. The list may only shrink: splitting a file **requires** deleting its entry,
or `budget_list_is_not_stale` fails the suite. New files are capped at 800
immediately, so the refactor can land one file at a time.

**Baseline** (measured before Wave 0): 9 files / **13,231 lines** = 51% of the
25,853 lines under `src/`.

**Current state** (after Wave 2): **4** files remain over the ceiling —
`sim/behavior.rs`, `sim/disease.rs`, `sim/params.rs`, `ui/screens/s01_map.rs`.

```
   2721 ./src/sim/behavior.rs         1005 ./src/ui/screens/s04_species.rs
   2495 ./src/ui/screens/s01_map.rs    974 ./src/ui/screens/s03_inspector.rs
   1561 ./src/sim/disease.rs           961 ./src/sim/genetics.rs
   1347 ./src/ui/screens/s05_charts.rs 859 ./src/sim/stats.rs
   1308 ./src/sim/params.rs
```

`cargo clippy --all-targets` → **0 warnings** · `cargo test` → **green** ·
determinism checksum pinned at `0x348e3c6eeec2e6d6`.

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
| 3 | sim data / serde risk | `sim/disease.rs`, `sim/params.rs` | ⬜ not started |
| 4 | Largest UI file | `s01_map.rs` | ⬜ not started |
| 5 | Highest risk (determinism) | `sim/behavior.rs` | ⬜ not started |
| 6 | Finalize | delete `OVER_BUDGET`, optional 600 ceiling | ⬜ not started |

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

## Wave 3 — sim data / serde risk ⬜

**`disease.rs` (1561)** → façade ~70 + `types.rs` ~195 · `effects.rs` ~85 ·
`contagion.rs` ~140 · `spillover.rs` ~155 · `lifecycle.rs` ~110 · `daily.rs` ~120 ·
`outbreaks.rs` ~155 · `emergence.rs` ~125 · `tests.rs` ~461.

**`params.rs` (1308)** → façade ~120 + `world.rs` ~140 · `creatures.rs` ~90 ·
`predation.rs` ~170 · `genetics.rs` ~95 · `social.rs` ~50 · `ecology.rs` ~85 ·
`disease.rs` ~205 · `docs.rs` ~195 (`FIELD_DOCS`) · `presets.rs` ~35 ·
`toml_util.rs` ~50 · `tests.rs` ~138.

Top risks: serde `deny_unknown_fields` + field order (save compatibility),
`FIELD_DOCS` ↔ struct adjacency (`field_docs_complete`), `PRESETS` positional
ordering (`s09_worldgen.rs:255`).

## Wave 4 — `s01_map.rs` (2495) ⬜

Root ~330 (struct, state/selection `impl`, `impl Screen` orchestrator,
`map_options`, `map_origin_title`, `map_hint`, `status_keys`, `draw_ticker`) +
`input.rs` ~280 · `base.rs` ~220 · `overlays.rs` ~110 · `sense.rs` ~190 ·
`follow.rs` ~250 · `look.rs` ~60 · `species.rs` ~120 · `health.rs` ~110 ·
`disease.rs` ~280 · `parasites.rs` ~140 · `regions.rs` ~110 · `tests.rs` ~209.

Note: there are **three** `impl WorldMap` blocks in total (state/selection,
input + dispatch, per-overlay sidebars) — that is the natural seam. Shared helpers
across siblings (`overlays_selector`, `region_count`, `region_load_mean`,
`worst_region`, `fmt2`, `immune_to_shown`, `trend_arrow`) need `pub(super)`.

## Wave 5 — `sim/behavior.rs` (2721) ⬜

**Highest risk — do last.** Root ~130 (doc, `mod`s, re-exports, `tick_creatures`,
`day_boundary`) + `perception.rs` ~165 · `movement.rs` ~305 · `goals.rs` ~390 ·
`vitals.rs` ~100 · `death.rs` ~135 · `migration.rs` ~175 ·
`predation/threat.rs` ~180 + `predation/hunt.rs` ~420.

Required re-exports: `kill` and `find_walkable_near` (`disease.rs`), `needs`
(`genetics.rs`), `migration_daily` (`sim/mod.rs`), plus `Perception` / `Kin`.

The 841-line test module must itself split into ~4–5 `#[cfg(test)]` child files
(841 still exceeds the ceiling), which requires bumping ~15 helpers to `pub(super)`.

## Wave 6 — finalize ⬜

- [ ] Delete `OVER_BUDGET` entirely; the 800 ceiling becomes absolute.
- [ ] Optionally lower `MAX_LINES` to 600 as a second ratchet.
- [ ] Optionally tighten `too-many-lines-threshold` (measured: **30 violations** at 60).
- [ ] Optionally fix the stale `s01_map::render_base` / `ORIGIN` reference in
      `docs/PROTOTYPE_GUIDE.md:41`.

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
