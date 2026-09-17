# Sparkline

Back to the [component index](README.md).

A one-row strip of `░▒▓█` cells, one cell per sample, shaded by where the
sample sits between the strip's own minimum and maximum. Shows the recent
population of each species in the [S01](../screens/s01-world-map.md) sidebar,
the `30-day trend` column and detail row of
[S04](../screens/s04-species-browser.md), and the per-generation drift rows of
S04b. The prototype [S06](../screens/s06-ecology.md) Totals panel shows 240-day
strips beside each resource bar.

## Anatomy

```
<Cells>
```

One cell per value. When there are fewer values than the width, the strip is
left-aligned and the remaining cells are not written.

## Slots
| Slot  | Required | Position                                    | Overflow                                                      |
|-------|----------|---------------------------------------------|---------------------------------------------------------------|
| Cells | yes      | starts at the left edge, one cell per value | only the last `w` values are drawn; older values are dropped  |

## Sizing
Height 1. Width `w` is a parameter; the strip draws `n = min(values.len(), w)`
cells and never more than `w`. Minimum `w` is 1. Widths in use: 18 (S01
sidebar), 14 (S04 table), 30 (S04 detail), 36 (S04b drift), 44 (S06
prototype).

Each cell is chosen from the last `n` values only:

- `t = (v − min) / (max − min)` over the drawn slice, or `0.5` when
  `max == min`; `max` is at least 1.
- `i = round(t × 3) + 1`, so `1..=4`.
- `cell = glyphs::SHADES[min(i, 4)]`: `░ ▒ ▓ █`.

A flat series gives `t = 0.5`, `round(1.5) = 2`, so every cell is `▓`. The shade is relative to the drawn slice, not to any global
scale, so two strips are not comparable by shade.

## Variants
| Variant   | What changes                                                     | Used by                                   |
|-----------|------------------------------------------------------------------|-------------------------------------------|
| Recent    | last `w` daily counts, species colour                            | S01 sidebar (18), S04 table (14), S04 detail (30) |
| Stepped   | each sample repeated three times so one generation is 3 cells    | S04b drift rows (36)                      |
| Wide      | 44 cells across the last 240 days; prototype only                | S06 Totals `Now` rows                      |
| Dimmed    | `theme::DIM` instead of the species colour for an extinct species | S04 table                                 |

## Interaction
None. Display only. The keys of the screen around it are in [keys.md](keys.md).

## Styling
- Cells: the caller's colour on `theme::PANEL_BG`. Species rows use the
  species colour from the roster; S04b drift rows use
  `common::trait_color(t)`; S04 uses `theme::DIM` when the species count is 0.
- Cells past `n` are left as they were; the caller's row style shows through.

## Glyphs
`glyphs::SHADES[1..=4]`: `glyphs::SHADE_1` `░`, `glyphs::SHADE_2` `▒`,
`glyphs::SHADE_3` `▓`, `glyphs::SHADE_4` `█`. `glyphs::SHADE_0` (space) is in
the table but unreachable, since `i` starts at 1.

## Composition
*Contains:* nothing.
*Contained by:* [Table](table.md) cells (S04 table), population rows in a
[Panel](panel.md) body (S01 sidebar, S04 detail). Usually paired with a
[Trend Arrow](trend-arrow.md) for the same series.

## API
### Today
```rust
widgets::bars::sparkline(buf, x, y, w, values: &[u16], color)
```
Callers: `s01_map/base.rs::population_section` (`w` 18, the last 30 samples),
`s04_species/table.rs::table_row` (`w` 14, `s.trend`),
`s04_species/summary.rs` (`w` 30, `s.trend`), `s04_species/drift.rs` (`w` 36).

### Planned
```rust
Sparkline::new(&counts)                  // &[u16]; draws the last `w`
    .color(roster.color(id))
    .render(buf, row_area)               // w = row_area.width
```
`height` is 1; `min_width` is 1.

## Gaps today
- Takes `x, y, w` instead of a one-row `Rect`.
- No downsampling: the S01 sidebar gathers 30 days but the 18-cell strip shows
  the last 18. `common::downsample` exists but only the S04 history plot uses
  it.
- S06 Totals draws no sparklines in the live code; the `sparklines = last 240
  days` hint and the strips exist only in the prototype render.
- A series of all zeros has `max` forced to 1 and `min` 0, so it renders `░`,
  not the flat `▓`.

## Examples

### Recent, S01 sidebar row (43 columns)
`w` 18 with the 18 values `310 308 305 302 300 296 290 285 281 275 268 262 258
254 252 250 251 267`; min 250, max 310. The arrow is a
[Trend Arrow](trend-arrow.md).
```
║ V Vole    267 ↑    █████▓▓▓▓▒▒▒░░░░░▒   ║
```

### Flat series, S04 table cell (43 columns)
`w` 14 with fourteen values of 13. Every cell is `▓`. The row is the S04a
Deer row cut to the columns that fit.
```
║ D Deer    prey     13   ▓▓▓▓▓▓▓▓▓▓▓▓▓▓ ↓║
```

### Fewer values than width (43 columns)
`w` 18 with only ten values `250 262 275 290 300 310 305 296 281 267`. The
strip is left-aligned and the last eight cells are untouched.
```
║ V Vole    267 ↑    ░▒▒▓███▓▓▒           ║
```

### Wide, prototype S06 row (90 columns)
`w` 44. Reproduced with the 44 values `2 2 3 3 3 3 3 2 2 1 1 1 1 1 1 1 2 2 3 3
3 3 3 2 2 2 1 1 0 0 0 0 0 1 1 3 3 3 3 3 2 2 2 1` (min 0, max 3, so each value
maps straight to a shade). The bar is a [Labeled Bar](labeled-bar.md) with
`label_w` 13, then three blank cells before the strip.
```
║ vegetation  [████████░░░░░░░░░░]    44%   ▓▓█████▓▓▒▒▒▒▒▒▒▓▓█████▓▓▓▒▒░░░░░▒▒█████▓▓▓▒ ║
```

## Open questions
- Should the planned struct downsample to `w` (mean per bucket, as
  `common::downsample`) instead of taking the last `w` values, so the S01
  strip really covers 30 days?
- Should shade be relative to the species peak rather than the drawn slice,
  so a strip of small wobbles does not look like a boom and bust?
