# Legend

Back to the [component index](README.md).

A grid of glyph, colour and description entries that explains what the map
draws: terrain and resource symbols, then the species letters. The
[S01](../screens/s01-world-map.md) sidebar shows it in two columns; the
[S11](../screens/s11-legend-help.md) help modal shows the same entries in one
column with a note after each. The terrain rows of
[S06](../screens/s06-ecology.md) and [S09](../screens/s09-world-generation.md)
reuse the glyph and colour pairs but are [Table](table.md) rows, not a Legend.

## Anatomy

```
 <Glyph> <Label>          <Glyph> <Label>         
 <Pair> <Name>    <Pair> <Name>    <Pair> <Name>  
 <Footer>                                         
```

Entry rows hold `columns` entries of `entry_w` cells each after a one-cell
margin. Species rows hold three `Pair Name` entries of 13 cells. Footer is
the `UPPER adult   lower juvenile` line.

## Slots
| Slot   | Required | Position                                                     | Overflow                                            |
|--------|----------|--------------------------------------------------------------|-----------------------------------------------------|
| Glyph  | yes      | first cell of an entry, in its own colour                    | never cut                                           |
| Label  | yes      | one space after Glyph, padded to `label_w` (17 in S01, 14 in S11) | cut at `label_w`, no marker                    |
| Note   | no       | after Label, free text (S11 only)                            | cut at the column's right edge                      |
| Pair   | yes      | `Vv`, adult then juvenile letter, bold in the species colour | never cut                                           |
| Name   | yes      | one space after Pair, padded to 10                           | cut at 10                                           |
| Footer | no       | last row, dim                                                | cut at the area width                               |

## Sizing
Width is the area width. Entry width is `1 + 1 + label_w`: 19 in S01, so two
columns take 39 of the sidebar's 41 inner cells. Species entries are `2 + 1 +
10 = 13`, three to a row. Height is `ceil(entries / columns) + ceil(species /
3) + 1`; with twelve map entries and six species that is `6 + 2 + 1 = 9`
rows. S11 uses one column of `3 + 14 + note` and no species rows; its
creature column is a separate table (see Variants). The component never
writes outside its area; rows past the bottom are dropped.

## Variants
| Variant    | What changes                                                                          | Used by                |
|------------|---------------------------------------------------------------------------------------|------------------------|
| Sidebar    | two columns, `label_w` 17, dim labels, species rows in threes, Footer                 | S01 sidebar `Legend` section |
| Help       | one column, Glyph as ` g ` bold, Label 14 in `theme::text()`, then a Note              | S11 `Terrain` column, also its `Events` and `Map marks` lists |
| Creatures  | `V  v  Vole   prey      seeds, roots` rows with a `theme::label()` header and Footer   | S11 `Creatures` column |
| Terrain-led rows | glyph and colour from `map::terrain_cell`, then a bar and counts                | S06 `Terrain`, S09 preview; a Table, listed for cross-reference |

## Interaction
None. Display only. The keys of the screen around it are in [keys.md](keys.md).

## Styling
- Glyph: its own colour from `map::legend()` on `theme::PANEL_BG`:
  `theme::DEEP_WATER_FG`, `theme::SHALLOW_FG`, `theme::SAND_FG`,
  `theme::DIRT_FG`, `theme::GRASS_SPARSE_FG`, `theme::GRASS_FG`,
  `theme::GRASS_DENSE_FG`, `theme::FOREST_FG`, `theme::ROCK_FG`,
  `theme::DEN`, `theme::CARCASS`, `theme::SEED`. Help adds `Modifier::BOLD`.
- Label: `theme::dim_text()` in Sidebar, `theme::text()` in Help.
- Note: `theme::dim_text()`.
- Pair: the species colour from the roster, `Modifier::BOLD`.
- Name: `theme::dim_text()` in Sidebar, `theme::text()` in Creatures.
- Footer: `theme::dim_text()`.
- Creatures header: `theme::label()`; role column `theme::GOOD` for prey,
  `theme::BAD` for predators; diet `theme::dim_text()`.

## Glyphs
Map entries, in `map::legend()` order: `glyphs::DEEP_WATER` `≈`,
`glyphs::SHALLOW_WATER` `~`, `glyphs::SAND` `·`, `glyphs::DIRT` `.`,
`glyphs::GRASS_SPARSE` `,`, `glyphs::GRASS` `"`, `glyphs::GRASS_DENSE` `♣`,
`glyphs::FOREST` `♠`, `glyphs::ROCK` `▲`, `glyphs::DEN` `Ω`,
`glyphs::CARCASS` `%`, `glyphs::SEED` `*`. Species letters come from the
roster (`adult_glyph`, `glyph`) and have no constants.

## Composition
The two columns are the Grid variant of [Columns](columns.md): `[Fixed(20),
Fill(1)]`, one entry per cell.
*Contains:* nothing.
*Contained by:* [Panel](panel.md) bodies under a [Divider](divider.md) (S01
sidebar), [Modal](modal.md) columns (S11), [Scroll Region](scroll-region.md)
when the sidebar is short.

## API
### Today
```rust
widgets::map::legend() -> Vec<(char, Color, &'static str)>
ui::screens::s01_map::base::legend_section(f, inner, row, roster)
ui::screens::s11_help::terrain_column(buf, col) -> u16
ui::screens::s11_help::creature_column(buf, col, roster) -> u16
```
`legend_section` draws the Divider, the two-column entries, the species rows
and the Footer. The S11 functions are private.

### Planned
```rust
Legend::map()                            // entries from map::legend()
    .columns(2).label_w(17)              // Sidebar defaults
    .species(roster)                     // adds the Pair Name rows and Footer
    .render(buf, area)

Legend::map().columns(1).label_w(14).notes(&TERRAIN_NOTES)   // Help
Legend::new(&[(glyphs::BIRTH, theme::GOOD, "birth / litter")])   // any list
```
`height` is the row count for the given width; `min_width` is
`1 + 2 + label_w` for one column.

## Gaps today
- `legend_section` draws its own [Divider](divider.md); the planned component
  is the grid only.
- S11 rebuilds the glyph cell and column layout by hand
  (`glyph_span`, `terrain_column`) instead of sharing S01's loop.
- The species rows and the Footer are fixed at three per row and one line; a
  roster of more than nine species pushes the Footer out of the sidebar.
- `glyphs::SEED` `*` is also `glyphs::SNOW` and `glyphs::WINTER`, so the
  `regrowth` entry is ambiguous in winter.

## Examples

### Sidebar, S01 rows (43 columns)
The nine rows of the S01a `Legend` section, cell for cell.
```
║ ≈ deep water       ~ shallow water      ║
║ · sand             . bare dirt          ║
║ , sparse grass     " grassland          ║
║ ♣ meadow           ♠ forest             ║
║ ▲ rock             Ω den / burrow       ║
║ % carcass          * regrowth           ║
║ Vv Vole      Hh Hare      Dd Deer       ║
║ Ff Fox       Ww Wolf      Ll Lynx       ║
║ UPPER adult   lower juvenile            ║
```

### Sidebar under its Divider (43 columns)
```
║─ Legend ────────────────────────────────║
║ ≈ deep water       ~ shallow water      ║
║ · sand             . bare dirt          ║
```

### Help, one column with notes (40 columns)
The first three `Terrain` rows of S11a, between the modal's `║` and the
column rule `│`.
```
║ ≈ deep water    impassable, drinkable│
║ ~ shallow water drinkable, slow      │
║ · sand          no forage            │
```

### Creatures, S11 column (40 columns)
```
│ ad jv  species role      diet        │
│ V  v  Vole   prey      seeds, roots  │
│ H  h  Hare   prey      grass, bark   │
│ UPPER adult   lower juvenile         │
```

## Open questions
- Should the species rows wrap by the available width rather than always
  three per row, so a wider panel shows more per line?
- Should Legend own the S11 `Events` and `Map marks` lists too, since they are
  the same glyph, colour and text shape with different sources?
- Should the map legend and `map::terrain_cell` share one table so the S06 and
  S09 terrain rows cannot drift from it?
