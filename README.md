# Sim Fortress

A terminal predator / prey / evolution / resource-scarcity simulation with a
Dwarf-Fortress-inspired text UI. C1 is implemented: a deterministic `src/sim` core
(seed → terrain → clock → seasons → events) and a `src/ui` app shell with the live
world-generation form, world map, controls modal and help overlay. The static UI
prototypes remain available behind `--prototypes` for side-by-side comparison.

## Running

Requirements: Rust (stable) and a terminal with truecolor support. Any monospace font
works — every glyph is from code page 437. The live application adapts to any terminal
size; **155×45** is the reference layout (resize to it to compare against the
prototypes, which are drawn at a fixed 155×45).

```bash
cargo run                                        # live app (opens the world-generation form)
cargo run -- --width 200 --height 50             # live app, pre-fill the world size
cargo run -- --headless --seed 42 --ticks 4320   # headless: prints a checksum + last events
cargo run -- --headless --seed 42 --ticks 4320 --width 200 --height 50 --params world.toml
cargo run -- --prototypes S01a                   # static prototype viewer (fixed 155x45)
```

The world size is configurable in the S09 form (Width 100–200, Height 30–60), via the
`--width`/`--height` flags, or via a `[world] width = … / height = …` table in a
`--params` TOML file. The prototype viewer shows the prototype id and name on row 0.

| Key                      | Action                              |
|--------------------------|-------------------------------------|
| `]` `PgDn` `→` `l`       | next prototype                      |
| `[` `PgUp` `←` `h`       | previous prototype                  |
| `0`–`9`                  | jump to screen S0n                  |
| `Home` / `End`           | first / last prototype              |
| `q` `Esc` `Ctrl-C`       | quit                                |

## Prototype screens

| Id   | Screen                 | Variant                                             |
|------|------------------------|-----------------------------------------------------|
| S00a | Title / Main Menu      | default                                             |
| S01a | World Map              | default: map + sidebar + ticker + status bar        |
| S01b | World Map              | wide map, sidebar collapsed                         |
| S01c | World Map              | look / cursor mode with floating tooltip            |
| S01d | World Map              | winter season, night palette                        |
| S01e | World Map              | following a creature (trail, target, vitals)        |
| S02a | Map Overlay            | vegetation density heatmap                          |
| S02b | Map Overlay            | population / predator pressure                      |
| S02c | Map Overlay            | water & moisture                                    |
| S02d | Map Overlay            | sense range of the selected predator                |
| S03a | Creature Inspector     | prey (Bramble the hare)                             |
| S03b | Creature Inspector     | predator (Ashfang the wolf)                         |
| S03c | Creature Inspector     | corpse (Thistle the deer)                           |
| S04a | Species Browser        | species table                                       |
| S04b | Species Browser        | species detail: trait histograms and drift          |
| S05a | Population Charts      | prey vs predator over time                          |
| S05b | Population Charts      | predator–prey phase plot                            |
| S05c | Population Charts      | stacked species + vegetation biomass                |
| S06a | Resources / Ecology    | totals, season modifiers, per-region scarcity       |
| S07a | Event Log              | full log with filter chips                          |
| S07b | Event Log              | deaths & extinctions with detail pane               |
| S08a | Lineage / Family Tree  | Ashfang w#042                                       |
| S09a | World Generation       | new world form with preview                         |
| S10a | Simulation Controls    | modal over the world map                            |
| S11a | Legend & Help          | overlay over the world map                          |
| S12a | Alert Modal            | extinction event                                    |
| S13a | Local Zoom View        | 3×1 tiles around the cursor                         |

## Code layout

```
src/lib.rs             crate root: pub mod sim; pub mod ui; pub mod widgets; …
src/main.rs            CLI dispatch: --headless | --prototypes | live app
src/sim/               pure, deterministic core (params, rng, time, world, events, Sim)
src/ui/                app shell: AppState, screen stack, viewport, live S01/S09/S10/S11
src/theme.rs           truecolor palette and color ramps
src/glyphs.rs          named CP437 glyph constants (+ test that every glyph is CP437)
src/fixtures/          prototype fixture data (re-exports the `sim` data types)
src/widgets/           shared widgets: header, panel, bars, status bar, map renderer
src/prototypes/        one file per screen; `mod.rs` holds the registry + viewer
tests/                 integration tests (lib-level determinism, params round-trip)
docs/PROTOTYPE_GUIDE.md  conventions for adding screens
```

## Documentation

- [docs/chunks/README.md](docs/chunks/README.md) — delivery roadmap: six sequential chunks with checkpoints, one document per chunk.
- [docs/screens/README.md](docs/screens/README.md) — screen overview, navigation map and links to a requirements file per screen.
- [docs/PROTOTYPE_GUIDE.md](docs/PROTOTYPE_GUIDE.md) — glyph rules, palette and widget conventions for adding screens.
