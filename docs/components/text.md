# Text

Back to the [component index](README.md).

One row of styled text, the leaf every panel body is made of: a clock line in
the [S01](../screens/s01-world-map.md) sidebar, a stat in the
[S03](../screens/s03-creature-inspector.md) inspector, a cell in a
[Table](table.md) row. It replaces the `util::line` calls that every screen
makes today.

## Anatomy

```
<Text>
```

## Slots
| Slot | Required | Position                                                  | Overflow                                  |
|------|----------|-----------------------------------------------------------|-------------------------------------------|
| Text | yes      | left-aligned from column 0 by default; `right()` ends at the last cell, `center()` splits the slack (odd cell to the right) | cut at the right edge with no marker, whatever the alignment |

## Sizing
Height 1. Width is the row it is given; `min_width` is the width of the text.
A Text wider than its row is drawn left-aligned and cut, so the start is always
visible. Paints only the cells it writes; the rest of the row keeps whatever
the enclosing [Panel](panel.md) filled. No other parameters.

## Variants
| Variant  | What changes                                        | Used by                          |
|----------|-----------------------------------------------------|----------------------------------|
| Plain    | one span in `theme::text()`                         | every screen                     |
| Styled   | several spans, each with its own style              | S01 clock rows, S03 stats        |
| Dim      | `theme::dim_text()`                                 | hints, tick counters, notes      |
| Right    | ends at the last cell of the row                    | numeric cells, `44%` values      |
| Centred  | centred in the row                                  | modal hints, the title screen    |

## Interaction
None. Display only.

## Styling
`Text::new` uses `theme::text()`; `style()` replaces the style of every span;
`fg()` and `bold()` adjust it. `Text::spans` keeps each span's own style.
Nothing is background-filled beyond the cells the text occupies.

## Glyphs
Whatever the text contains; callers pass `glyphs::` constants where a glyph is
meant.

## Composition
*Contains:* nothing.
*Contained by:* [VStack](vstack.md), [HStack](hstack.md) cells,
[Columns](columns.md) cells, [Table](table.md) cells, [Panel](panel.md) and
[Modal](modal.md) bodies.

## API
### Today
```rust
widgets::util::line(f, area, row, line: Line<'_>)
widgets::util::line_in(buf, area, row, line: Line<'_>)
buf.set_stringn(x, y, text, max_width, style)
```
`line_in` now wraps `Text`; `set_stringn` is used at about 160 sites.

### Planned
```rust
Text::new(" vegetation")                       // theme::text()
    .style(theme::dim_text())                  // whole-row style
    .fg(color).bold()                          // adjustments
    .right()                                   // or .center()
    .render(buf, row_area)
Text::spans(vec![Span::styled(..), ..])        // multi-style row
Text::line(line)                               // adapter for a ratatui Line
```
`height` is 1; `min_width` is the text width.

## Gaps today
- Screens build a `Line` per row and pass `(area, row)` pairs instead of a
  one-row `Rect`; `set_stringn` callers place text by absolute `x`.

## Examples

### Plain (43 columns)
```
║ Hello!                                  ║
```

### Right-aligned (43 columns)
Text ` 44%`.
```
║                                      44%║
```

### Centred (43 columns)
Text `Enter select   ←→ move   Esc continue`, 37 cells in 41: two blanks
each side.
```
║  Enter select   ←→ move   Esc continue  ║
```

### Cut at the right edge (43 columns)
Text `The quick brown fox jumps over the lazy dog and on`, 50 cells.
```
║The quick brown fox jumps over the lazy d║
```

### Styled spans (43 columns)
Two spans: ` Year 12  Day 4` in `theme::text()`, `     ♫ Autumn` in
`theme::ACCENT`.
```
║ Year 12  Day 4     ♫ Autumn             ║
```

### Dim (43 columns)
`theme::dim_text()`.
```
║ tick 5,184                              ║
```

## Open questions
- Should a cut Text end with `glyphs::DOT` `·` as `common::clip` does, or stay
  silent like Divider and Table cells? Silent, pending the shared cut-marker
  decision in [divider.md](divider.md).
