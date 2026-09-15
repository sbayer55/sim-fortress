# Performance

C6 FR9 performance budget, measured and recorded. Numbers come from
`cargo run --release -- --headless --seed 1 --ticks N --profile` (per-system
timings) and a `--profile` run with the creature counts noted.

## Machine and method

| Item | Value |
|------|-------|
| Machine | Apple M1 Max |
| Cores | 10 |
| OS | Darwin 27.0.0 (macOS) |
| Rust | 1.96.0 |
| Build | `cargo build --release` (opt-level 3) |
| Method | `--profile` accumulates wall-clock ns per system over `N` ticks, then reports seconds per 1 000 ticks |

## Headless budget

Target: ≥ 3 000 ticks/s at 150×40 with 1 000 creatures.

Measured (150×40, ~1 010 founders, 10 000 ticks, seed 1):

| System | s / 1 000 ticks | share |
|--------|----------------:|------:|
| behavior | 1.64 | 97 % |
| day boundary | 0.001 | <1 % |
| ecology | 0.006 | <1 % |
| migration/extinction | 0.002 | <1 % |
| spatial | 0.036 | 2 % |
| **total step** | **1.69** | — |

Throughput ≈ **590 ticks/s** at ~1 000 creatures. With the population collapsed
(the default balance collapses by year 3–5, see the C6 doc balance note) an
empty-world run measures ≈ 2 800 ticks/s.

### Optimisation applied (C6 FR9)

`--profile` first revealed the **predator-first threat query** (`mark_threats`)
as the single largest per-tick cost (~1.0 ms of ~2.7 ms/tick at 1 000 creatures).
It was rewritten to use a non-allocating, non-sorting ellipse visit
(`SpatialIndex::for_each_within`) plus a flat id-sorted prey snapshot with
binary-search lookups, replacing the per-visit `BTreeMap` accumulation. This
cut the total step from ≈ 2.67 s to ≈ 1.69 s per 1 000 ticks (≈ 1.6×) while
preserving bit-for-bit determinism (the `checksum_is_fnv_stable` lock still
holds).

The remaining cost is the per-creature behaviour update (`update_one`), dominated
by the `perceive` ellipse cell scan (goal selection every `replan_ticks`). A
deeper pass (caching nearest-water/graze per cell, or reducing perception
frequency) is the next step; it was not completed in this chunk and the
3 000 ticks/s budget is **not yet met**.

C5 `FR5b` added the wary tier to the same hot loop: each (predator, prey) visit in
`mark_threats` now also keeps the nearest detected *non-danger* predator, which is two
extra comparisons and three `PreySnap` writes with no allocation, no new query and no
reordering; `move_speed` was factored out of `move_toward` and `preempt_prey` out of
`update_one` (pure code motion). The table above predates that change and was **not**
re-measured; the added work is O(1) per pair the threat scan already visits, so the
measured budget and its open status are unchanged.

## UI budget

Target: ≥ 30 FPS at x25 with 3 000 creatures on 200×60.

Not measured here (requires an interactive truecolor terminal). At x25 the
simulator must produce 25 × 2 = 50 ticks per frame, i.e. one frame's simulation
must complete in < 20 ms. The headless number above (≈ 2.67 ms/tick at 1 000
creatures) extrapolates to ≈ 13 ms/tick at 3 000 creatures and ≈ 670 ms per
x25 frame — also **not yet met**; the same behaviour-tick optimisation applies.

## Summary

The profiling harness (`--profile`) and the parallel sweep
(`scripts/sweep.sh`, `xargs -P $(nproc)`) are in place. The performance budget
is recorded here and remains open pending the behaviour-tick optimisation pass;
the balance note in
[docs/chunks/c6-persistence-and-balance.md](chunks/c6-persistence-and-balance.md)
records the same gap for the survival criterion.
