# Columns

Back to the [component index](README.md).

A shared column spec that a block of rows draws against, so cells line up
down the block. It is resolved once for the whole block, before any row is
drawn; each row is then an [HStack](hstack.md) that uses the resolved offsets
instead of its own constraints. The Population rows on
[S01](../screens/s01-world-map.md), every [Table](table.md), the two-column
[Legend](legend.md), and the [Histogram](histogram.md) grid on S04b are
Columns blocks.

## Anatomy

```
<Col 1  ><Col 2       ><Col 3><Col 4          >
<Col 1  ><Col 2       ><Col 3><Col 4          >
<Col 1  ><Col 2       ><Col 3><Col 4          >
```

Every row shares the same column edges. A row may opt out and span the width
(a totals line, a note); it is then a plain child of the enclosing
[VStack](vstack.md).

## Slots
| Slot    | Required | Position                                                          | Overflow                                                      |
|---------|----------|-------------------------------------------------------------------|---------------------------------------------------------------|
| Columns | yes, 1+  | left to right; each has a constraint and a default cell alignment | whole trailing columns are dropped when they do not fit; a column is never squeezed |
| Rows    | yes, 1+  | one HStack per row, cells placed at the resolved column offsets   | as [VStack](vstack.md)                                        |

## Sizing
Width is the area width. Resolution happens once per block:

1. `Fixed(n)` columns take `n`.
2. `Min(n)` columns are measured: the block asks every row for that cell's
   `min_width()` and takes the widest, never less than `n`. This is the second
   pass a per-row HStack cannot do; it is why a count of `4133` widens the
   count column for every row instead of pushing one row's cells right.
3. `Fill(w)` columns share what is left by weight; the remainder goes to the
   last `Fill` column.
4. `Auto` is not allowed here; it would mean `Min(0)`, so say that.

Height is the number of rows; `height(width)` is the row count. `min_width` is
the sum of the `Fixed` widths and the `Min` floors.

Cell alignment inside a column (`left`, `right`) is part of the column spec
so every row agrees; a cell may still override it. Numbers are right-aligned,
text left-aligned, as the [Table](table.md) sheet says.

## Variants
| Variant  | What changes                                                       | Used by                              |
|----------|--------------------------------------------------------------------|--------------------------------------|
| Fixed    | every column `Fixed`; resolves from the width alone                | S01 Population rows today            |
| Measured | one or more `Min` columns; rows are measured before drawing        | counts and names that can grow       |
| Headed   | the first row is a header sharing the same Columns                 | Table                                |
| Grid     | equal `Fill(1)` columns holding whole components                   | Legend, S04b histogram grid          |

## Interaction
None. Display only. Keys reach the rows through the screen; see
[keys.md](keys.md).

## Styling
None of its own. Columns paints nothing; cells and rows carry their styles.
A selected row's `theme::selected()` fill is the row's, applied across every
column including the gaps.

## Glyphs
None.

## Composition
*Contains:* rows, each an [HStack](hstack.md) of cells: Text,
[Labeled Bar](labeled-bar.md) (Bare), [Sparkline](sparkline.md),
[Trend Arrow](trend-arrow.md), [Range Bar](range-bar.md), or a whole component
per cell in the Grid variant ([Histogram](histogram.md), a [VStack](vstack.md)
of Legend entries).
*Contained by:* [VStack](vstack.md), [Table](table.md), [Panel](panel.md) and
[Modal](modal.md) bodies, [Scroll Region](scroll-region.md).

## API
### Today
No helper. Rows align because every row uses the same literal widths:
```rust
// src/ui/screens/s01_map/base.rs, population_section
Span::styled(format!(" {} ", glyph), ..)      // 3
Span::styled(format!("{:<6}", name), ..)      // 6
Span::styled(format!("{count:>5} "), ..)      // 6
Span::styled(arrow.to_string(), ..)           // 1
Span::styled("  ", ..)                        // 2, then 2 untouched cells
bars::sparkline(buf, inner.x + 20, y, 18, ..) // 18 at a literal offset
```
`format!` pads but never cuts, so a six-digit count would push the arrow one
cell right on that row only.

### Planned
```rust
let cols = Columns::new(&[
    Fixed(3),                 // " V "
    Fixed(6),                 // name, left
    Min(5),                   // count; grows for every row if one needs it
    Fixed(1),                 // gap
    Fixed(1),                 // trend arrow
    Fixed(4),                 // gap
    Fixed(18),                // sparkline
    Fill(1),                  // the rest
])
.align(2, Align::Right);      // or build `Column::titled("Count", Min(5)).right()` entries
let rows = species.iter().map(|s| Row::new()
    .cell(Text::new(format!(" {} ", s.glyph)).fg(s.color).bold())
    .cell(Text::new(s.name))
    .cell(Text::new(s.count.to_string()))
    .cell(Spacer::cols(1))
    .cell(TrendArrow::new(&s.series))
    .cell(Spacer::cols(4))
    .cell(Sparkline::new(&s.series).color(s.color))
    .cell(Spacer::cols(0)));
Block::new(cols).rows(rows).selected(None).render(buf, area)   // measures Min columns, then draws
```
A cell is a `Text` (taking the column's alignment unless it sets its own),
any component (`Cell::Widget`, with `From` impls for Bar, Range Bar,
Sparkline, Trend Arrow, Spacer and HStack), or `Cell::Blank`.
`Row::span(component)` makes a row that ignores the columns, such as a
Divider. A `Table` is `Block` with a header row, a Marker column and a
selected row.

## Gaps today
- The S01 Population rows still align by matching literals; they move to a
  `Block` when the sidebar becomes a VStack.

## Examples

### Fixed, the S01 Population rows (43 columns)
Columns `[3, 6, Min 5, 1, 1, 4, 18, Fill]`. The count column measures at 5
because `4133` fits its floor.
```
║─ Population ────────────────────────────║
║ V Vole      0 ↔    ░░░░░░░░░░░░░░░░░░   ║
║ H Hare      0 ↓    ██████████████░░░░   ║
║ D Deer   4133 ↔    ██▓▓▓▓▒▒▒▒▒▒▒▒▒░░░   ║
║ F Fox       0 ↔    ░░░░░░░░░░░░░░░░░░   ║
║ W Wolf      0 ↔    ░░░░░░░░░░░░░░░░░░   ║
║ L Lynx      0 ↔    ░░░░░░░░░░░░░░░░░░   ║
```

### Measured, one count wider than the floor (43 columns)
The same Columns with a Deer count of `413300`. The count column measures at
6, so every row shifts one cell and the trailing Fill shrinks to 2.
```
║ V Vole       0 ↔    ░░░░░░░░░░░░░░░░░░  ║
║ D Deer  413300 ↔    ██▓▓▓▓▒▒▒▒▒▒▒▒▒░░░  ║
```

### Grid, two Legend columns (43 columns)
Columns `[Fixed(20), Fill(1)]`, each cell a Legend entry.
```
║ ≈ deep water       ~ shallow water      ║
║ · sand             . bare dirt          ║
```

## Open questions
- Should `Min` measure only the rows on screen or every row of a
  `RowSource`? Every row keeps columns stable while scrolling; on-screen is
  cheaper. Every row is assumed.
- Should a totals row be allowed to align some cells to the columns and span
  the rest, or must it be all or nothing?
