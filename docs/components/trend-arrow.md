# Trend Arrow

Back to the [component index](README.md).

A single coloured cell, `↑`, `↔` or `↓`, that says whether a daily count
series rose, held or fell over the last 30 days. Sits after the count in every
population row of the [S01](../screens/s01-world-map.md) sidebar, in the
`30-day trend` column of the [S04](../screens/s04-species-browser.md) table,
in the S04 detail and narrative lines, and beside the totals in
[S05](../screens/s05-population-charts.md). The prototype
[S06](../screens/s06-ecology.md) `30d:` line uses the same arrows.

## Anatomy

```
<Arrow>
```

## Slots
| Slot  | Required | Position                    | Overflow                       |
|-------|----------|-----------------------------|--------------------------------|
| Arrow | yes      | the one cell it is given    | never cut; the cell is 1 wide  |

## Sizing
Height 1, width 1. No parameters other than the series. The arrow is chosen
by the C3 / FR15 rule:

- `a = counts[min(len − 30, len − 1)]`, the value 30 days ago or the oldest.
- `b = counts[len − 1]`, today.
- `pct = (b − a) / a × 100`; when `a == 0`, `pct` is `+∞` if `b > 0` and `0`
  otherwise.
- `↑` when `pct > 3`, `↓` when `pct < −3`, `↔` otherwise, and `↔` whenever
  `len < 2`.

The thresholds are exclusive: exactly +3 % is `↔`.

## Variants
| Variant    | What changes                                                        | Used by                                   |
|------------|---------------------------------------------------------------------|-------------------------------------------|
| Bare       | the arrow alone, after a count                                      | S01 sidebar, S02h disease overlay          |
| Bold       | `Modifier::BOLD`; takes the row's `theme::SELECT_BG` when selected  | S04 table column                           |
| With word  | `↓ declining`, `↔ stable`, `↑ growing` in the arrow's colour        | S04 detail `30-day trend` row              |
| With percent | the arrow leads a `+12 %` figure and a sentence (S04 narrative) or a `+5% / 30d` figure (S05a totals) | S04 narrative line, S05a totals, S04b population line |

## Interaction
None. Display only. The keys of the screen around it are in [keys.md](keys.md).

## Styling
- `↑`: `theme::GOOD`. `↓`: `theme::BAD`. `↔`: `theme::DIM`. Background
  `theme::PANEL_BG`. This is `common::arrow_color`.
- With word and With percent colour the whole phrase the same way.
- Bold adds `Modifier::BOLD`; a selected S04 row swaps the background for
  `theme::SELECT_BG`.

## Glyphs
`glyphs::UP` `↑`, `glyphs::DOWN` `↓`, `glyphs::FLAT` `↔`.

## Composition
*Contains:* nothing.
*Contained by:* population rows in a [Panel](panel.md) body, [Table](table.md)
cells, the [Status Bar](status-bar.md) line of S05. Normally paired with a
[Sparkline](sparkline.md) of the same series.

## API
### Today
```rust
ui::screens::common::trend_arrow(counts: &[u16]) -> char
ui::screens::common::arrow_color(a: char) -> Color
ui::screens::s01_map::base::trend_arrow(samples: &[Sample], i: usize) -> char
```
The S01 version applies the same rule to `Sample.population[i]` and inlines
the same colour match.

### Planned
```rust
TrendArrow::new(&counts)                 // &[u16]; picks the glyph and colour
    .bold()                              // S04 table
    .render(buf, cell_area)              // 1 × 1

TrendArrow::new(&counts).word()          // "↓ declining", width 11
```
`height` is 1; `min_width` is 1, or 11 with `.word()`.

## Gaps today
- The rule exists twice: `common::trend_arrow` over `&[u16]` and
  `s01_map::base::trend_arrow` over `&[Sample]`. The S01 copy also repeats
  the colour match instead of calling `arrow_color`.
- The word and percent suffixes are formatted by each screen.
- A series shorter than 30 compares today with the oldest sample, so a young
  world shows a "30-day" trend over fewer days without saying so.

## Examples
The sparkline cells in these rows are context; their inputs are in
[Sparkline](sparkline.md).

### Rising (43 columns)
250 thirty days ago, 267 today: +6.8 %, so `↑` in `theme::GOOD`.
```
║ V Vole    267 ↑    █████▓▓▓▓▒▒▒░░░░░▒   ║
```

### Flat (43 columns)
35 thirty days ago, 35 today: 0 %, so `↔` in `theme::DIM`.
```
║ F Fox      35 ↔    ░░░░░░▒▒▒▓▓▓▓████▓   ║
```

### Falling (43 columns)
30 thirty days ago, 27 today: −10 %, so `↓` in `theme::BAD`.
```
║ W Wolf     27 ↓    ░░░▒▒▒▒▓▓▓█████▓▓▓   ║
```

### With word (43 columns)
As in the S04 detail row, with the strip shortened to 14 cells to fit.
```
║ 30-day trend █▓▓▓▒▒▒▒░░░░░░ ↓ declining ║
```

## Open questions
- Should the 3 % threshold and the 30-day window move to `theme` or params
  as named constants? They are also written out in the S11 help text.
- Should `↔` for `len < 2` be distinguishable from a real flat trend, for
  example by drawing nothing until two samples exist?
