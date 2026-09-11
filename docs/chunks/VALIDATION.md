# Chunk readiness validation

Each chunk document was reviewed by an independent reviewer agent against this checklist:
unambiguous goal and checkpoint; complete scope; dependencies satisfied by earlier chunks;
every requirement implementable without asking (names, formulas, defaults, units); measurable
acceptance criteria; executable demo script; sufficient tests; feasibility; decisions an AI
agent would otherwise have to guess. Reviewers did not edit the documents; the roadmap owner
applied the fixes and re-submitted for a second pass.

## Pass 1 (2026-09-10) — original drafts

| Chunk | Verdict | Blockers | Headline findings | Resolution |
|-------|---------|----------|-------------------|------------|
| C1 | NOT READY | 3 | Widgets could not be reused "unchanged" (they take `&Fixtures`); global keys before the top screen would break text input on S09; time getters/day numbering undefined and the demo mixed two day conventions; terrain percentages had no method; regions rule contradictory; S10 content contradicted the tick model; no lib target for `tests/`; checksum and TOML deps unspecified | Rewritten: types move to `sim`, `MapData` view, top-screen-first key routing, full time getter definitions, quantile thresholds, scaled regions, lib.rs + serde/toml deps, FNV checksum, S09 form fully specified, dimming and `StepReport` added |
| C2 | NOT READY | 4 | Vegetation formula had no die-back so no seasonal pulse; moisture numbers made drought impossible; S06 Plenty rule unreachable without prey; water⇄sand lacked state | Rewritten: seasonal target with `season_cap` and `dieback_k`, proportional evaporation with per-rainfall equilibria, land-only means and prey clause gated on herbivores existing, `dried_from` marker, fixed daily update order, series fields including `drought_flags`, S07 chip semantics, S05/S06 content rules |
| C3 | NOT READY | 3 | hp loss killed before the starvation timer so starvation could never be a cause; acceptance run length inconsistent with "0 by day 720"; metabolism trait affected nothing (dooming C4's selection test); inspector "every field live" unimplementable; several balance numbers missing | Rewritten: hp-based death with cause attribution and `DeathThirst`, metabolism scales hunger, per-species `adult_age_days`, fractional movement, hysteresis and formulas for every goal, `MapSource` trait, live/placeholder table for S03, pressure increment, `sim::geom`, determinism rules |
| C4 | NOT READY | 4 | "mean generation ≥ 25" impossible with 90-day maturity; selection criterion unmeasurable; default fecundity guaranteed collapse against a hard threshold; S08 as specified rendered the whole population | Rewritten: per-species maturity, density-dependent mating, lower litter/cooldown defaults, blocker vs target criteria with a bounded balance table, 7-of-10-seed selection test, S08 rooted `lineage_up` generations with a node cap, templated S04 prose, births ticker gate |
| C5 | NOT READY | 4 | `can_detect` acceptance test contradicted the formula; demo needed S09 species rows owned by no chunk; extinction fired for absent species and re-fired daily; no channel for the sim to raise a modal | Rewritten: cover table with a passing test, S09 rows assigned to C3, once-per-species extinction with `StepReport` alerts, single kill roll with chase bonus, per-tick flee query, pressure scale and local migration trigger with cooldown, per-creature hunt/prey stats, `peak_lag` definition, Chebyshev vs ellipse metrics |
| C6 | NOT READY | 3 | Headless budget (50 000 ticks/s) infeasible; presets "two" vs five and an undefined difficulty knob; four screens without prototypes unspecified | Rewritten: 3 000 ticks/s budget, all five presets with overlay tables and `predation_difficulty` mapping, Load list / confirm / options modals specified, postcard + serde derives from C1, save header and paths, XDG config path, `toml_edit` dump, overlay merge semantics |

## Pass 2 (2026-09-10) — revised documents

All six revised documents were re-reviewed. Every pass-1 blocker was confirmed resolved
(two "partially": C1 `q` on S09 and C4 generation criterion, both fixed below). Reviewers
ran numeric sanity checks: C2 moisture equilibria (0.30 / 0.75 / 1.0) and a Monte-Carlo of the
vegetation model (year-2 summer/winter ratio ≈ 2.2), C3 survival times (vole ≈ 9 days, deer
≈ 6.5 days without food; thirst ≈ 5.5 days is the binding need), C4 generation turnover, C5
detection values and chase geometry.

| Chunk | Verdict | Findings applied after pass 2 |
|-------|---------|-------------------------------|
| C1 | READY WITH FIXES → **READY** | S09 owns `q` (text when a field is focused, else Quit); `AppState`/stack split to avoid a double borrow; `max_origin` defined once with `saturating_sub`; S10 prototype strings updated in the same commit; field count 27; terrain comparison after scrolling to x = 20 |
| C2 | READY WITH FIXES → **READY** | Drought detection, drying, refill and risk use land-cell means (displayed water = 1.0 is presentation only); `DroughtEased` event kind under the droughts chip; regrowth sites clear at 0.8 × terrain max; drought band needs ≥ 2 regions; ratio bound widened to 3.0 (structural value 2.2); "land cells" defined; S09 Regrowth rate live |
| C3 | READY WITH FIXES → **READY** | `born_day: i32`; single event capacity param (5000); rest end conditions split; `last_water` memory; genome stats in the census; age lower bound clamped; `MapSource` noted as replacing C1's `MapData` |
| C4 | READY WITH FIXES → **READY** | Random-parent inheritance instead of averaging (averaging halves variance each generation and collapses the histograms); generation blocker = max ≥ 12 and mean ≥ 8, ≥ 20 kept as a target with CSV columns; S08 always keeps the root→focus chain and the focus's kin; vegetation mating gate is prey-only; six-species genetics maps; `first_birth_day`; notable threshold 0.10; drift uses a high-water mark; `--params` deep-merge stated in C1 |
| C5 | READY WITH FIXES → **READY** | Chase clock starts at the trigger distance, `chase_max_ticks` 30, stalk uses the bonus budget; scavenging consumes decay; predator-first flee query (per-prey bucket scans forbidden); balance-table clause; `predation.difficulty` stored; `peak_lag` returns `Option`; migration uses seasonal shortfall, edge-sharing adjacency and a defined group; day-one hiding note corrected; demo uses `w` |
| C6 | READY WITH FIXES → **READY** | `difficulty` applied via `effective_*()` accessors, never mutating stored params; survival criterion aligned with C5 (≥ 5 species in ≥ 14 seeds); optimisation pass ordered after `--profile`, parallel sweep; header carries `season_days`, `start_hour` and four strip rows; autosave under `[ui]`; options modal 60×20; `q` behaviour supersedes C1; new deps listed |

Residual notes the implementer should read before starting a chunk are kept in each
document's **Risks** section (for example C2's within-region moisture homogenisation and
C4's competitive-exclusion risk for deer). No blocker remains open.
