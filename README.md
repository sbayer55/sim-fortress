# Sim Fortress

A terminal predator / prey / evolution / resource-scarcity simulation with a
Dwarf-Fortress-inspired text UI. Six species — voles, hares, deer, foxes, wolves
and lynxes — are born, graze, drink, hunt, breed, mutate and die by the numbers
in their genomes across a procedurally generated world of forests, meadows,
rivers and rock. You watch it run, follow individual creatures, inspect their
traits and lineages, and tune the balance from TOML files without recompiling.

## Running

Requirements: Rust (stable) and a terminal with truecolor support. Any monospace
font works — every glyph is from code page 437. The live application adapts to any
terminal size; **155×45** is the reference layout.

```bash
cargo run                                        # live app (title → New World → play)
cargo run -- --width 200 --height 50             # pre-fill the world size
cargo run -- --headless --seed 42 --ticks 4320   # headless: prints a checksum + last events
cargo run -- --headless --seed 42 --years 5      # run N years (overrides --ticks)
cargo run -- --dump-params                       # defaults with one comment per field
```

The game starts on the **title screen**: `New World`, `Load World`, `Options`,
`Quit`. On the world map, `F5` saves the current world, `F9` quick-loads the
newest save for it, and `q` returns to the title (prompting if there are unsaved
changes). `p` opens the controls/options modal; `?` opens the legend & help.

## Saving and loading

Saves are binary `SIMF` files in `saves/` (override with `--saves-dir DIR`),
created on first save. Filenames are the world name lowercased and slugified plus
the tick: `the-valley-of-sunfall-4320.simf`; autosaves are
`the-valley-of-sunfall-autosave.simf`. `Load World` lists saves newest first;
`Enter` loads, `Del` deletes (with confirmation). Autosave runs every
`ui.autosave_days` days (see Options; 0 = off).

The on-disk format is versioned and never migrated: **the genome widened to eleven
traits, so saves written before that change are rejected** with an "older version"
message in the Load list rather than being half-read. Old files still appear in the
list (marked `v<n>`) and can be deleted; only loading them fails.

## Parameters and presets

Every tunable lives in `src/sim/params.rs` with a documented default. A
`params.toml` in the working directory is applied automatically; `--params FILE`
is an additional overlay (partial TOML tables deep-merge over the defaults). When
you load a save the saved parameters win and `--params` is ignored. The
world-generation form offers five presets — Balanced, Harsh winter, Lush,
Archipelago and Fast evolution — that write their values into the form.

```bash
cargo run -- --dump-params > params.toml   # a commented starting point
cargo run -- --params params.toml          # play with those parameters
```

UI options (auto-pause, log births, follow-death pause, autosave interval,
day/night tint) are a separate file: `~/.config/sim-fortress/ui.toml`
(`$XDG_CONFIG_HOME/sim-fortress/ui.toml` when set).

## Headless experiments

```bash
cargo run --release -- --headless --seed 1 --years 10 --summary         # one summary row
cargo run --release -- --seeds 1-20 --years 10 --summary                # in-process sweep
scripts/sweep.sh 1 20 10                                                # parallel sweep → summary.csv
cargo run --release -- --headless --seed 1 --ticks 100000 --profile     # per-system timings
```

`summary.csv` columns: `seed, years,` then final counts × 6, `extinctions`,
`lag_days` (predator–prey lag) and `mean_speed_by_species` × 6.
`scripts/sweep.sh <first> <last> <years>` runs each seed as its own process with
`xargs -P $(nproc)`.

## Flame Graph

```bash
cargo install flamegraph
CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph --root -- --headless --seed 1 --ticks 12000
open ./flamegraph.svg
rm ./flamegraph.svg
```

## Code layout

```
src/lib.rs             crate root: pub mod sim; pub mod ui; pub mod widgets;
src/main.rs            CLI dispatch: --headless | --seeds | --summary | --profile | live app
src/sim/               pure, deterministic core (params, rng, time, world, behavior, save, Sim)
src/ui/                app shell: AppState, screen stack, viewport, S00/S01/S09/S10/S11 + data screens
src/theme.rs           truecolor palette and color ramps
src/glyphs.rs          named CP437 glyph constants (+ test that every glyph is CP437)
src/widgets/           shared widgets: panel, bars, status bar, map renderer
tests/                 integration tests (determinism, ecology, evolution, predators, sweep)
docs/                  roadmap chunks, screen requirements and rendered prototype snapshots
```

## Documentation

- [docs/chunks/README.md](docs/chunks/README.md) — the six-chunk delivery roadmap.
- [docs/screens/README.md](docs/screens/README.md) — screen overview and navigation map.
- [docs/chunks/c6-persistence-and-balance.md](docs/chunks/c6-persistence-and-balance.md) — save/load, title flow, presets, headless tooling.
- [docs/PERFORMANCE.md](docs/PERFORMANCE.md) — measured performance budget and method.
- [docs/screens/renders/](docs/screens/renders/) — text snapshots of every screen.
