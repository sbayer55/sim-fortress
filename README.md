# Sim Fortress

A terminal predator / prey / evolution / resource-scarcity simulation with a
Dwarf-Fortress-inspired text UI. **This repository currently contains static UI
prototypes only** — fixed fixture data, no simulation loop — so layouts, glyphs and
colors can be reviewed before any simulation code is written.

## Running the prototypes

Requirements: Rust (stable) and a terminal at least **155 columns × 45 rows** with
truecolor support. Any monospace font works — every glyph is from code page 437.

```bash
printf '\e[8;45;155t'   # resize most terminals to 155x45
cargo run               # start at the first prototype
cargo run -- S03b       # start at a specific prototype id
```

Row 0 of the screen always shows the prototype id and name.

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
src/main.rs            terminal setup / teardown
src/app.rs             event loop, fixed 155x45 frame, resize guard, screen cycling
src/theme.rs           truecolor palette and color ramps
src/glyphs.rs          named CP437 glyph constants (+ test that every glyph is CP437)
src/fixtures/          deterministic fixture data: world, creatures, species, series, events, lineage
src/widgets/           shared widgets: header, panel, bars, status bar, map renderer
src/prototypes/        one file per screen; `mod.rs` holds the registry
docs/PROTOTYPE_GUIDE.md  conventions for adding screens
```

Glyph rules and layout conventions are in [docs/PROTOTYPE_GUIDE.md](docs/PROTOTYPE_GUIDE.md).
