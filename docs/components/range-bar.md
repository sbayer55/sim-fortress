# Range Bar

Back to the [component index](README.md).

A one-row bracketed bar that shows a minimum, a mean and a maximum on a 0..1
scale: `[░░░▒▒█▒▒░░░]`. The span between min and max is shaded and the mean is
a solid marker inside it. Used for trait distributions in the
[S03](../screens/s03-creature-inspector.md) genome column (`species range`
and the `Offspring forecast` rows), the `spread` column of the
[S04](../screens/s04-species-browser.md) base-genome table, and the
ancestor-to-children spread in [S08](../screens/s08-lineage.md).

## Anatomy

```
[<Below><Span><Mark><Span><Above>] <Text>
```

Below is every cell before the min cell, Span every cell from the min cell to
the max cell, Mark the single cell at the mean, Above every cell after the max
cell. Text is the optional `lo..hi` figure after the bar.

## Slots
| Slot  | Required | Position                                             | Overflow                                                  |
|-------|----------|------------------------------------------------------|-----------------------------------------------------------|
| Below | yes      | inside the brackets, cells `0 .. a`                  | empty when min rounds to cell 0                           |
| Span  | yes      | cells `a ..= b`, where `a`/`b` are the min/max cells | never shorter than one cell; Mark replaces one of its cells |
| Mark  | yes      | cell `m`, the mean cell                              | always drawn, even outside Span when mean is not between min and max |
| Above | yes      | cells after `b`                                      | empty when max rounds to the last cell                    |
| Text  | no       | one space after `]`, `{lo:.2}..{hi:.2}`, 10 cells    | cut at the row's right edge                               |

## Sizing
Height 1. Width `w` is a parameter and includes both brackets; inner cells are
`w − 2`. Minimum `w` is 3. The three values are clamped to `0..=1` and mapped
to a cell index with

```
cell(v) = min( round( clamp(v) × (inner − 1) ), inner − 1 )
```

so `0.0` is the first inner cell and `1.0` the last. `round` is `f32::round`,
half away from zero. Widths in use: 11 (S03 species range), 13 (S04 spread),
22 (S03 forecast), 26 (S08 lineage). Text adds 11 cells. The bar never writes
outside `w` cells; Text is written by the caller after it.

## Variants
| Variant       | What changes                                                        | Used by                          |
|---------------|---------------------------------------------------------------------|----------------------------------|
| Species range | `w` 11, min/mean/max of the living species, trait colour            | S03 genome column, right of the own-value bar |
| Spread        | `w` 13, same values, trait colour                                   | S04 `Base genome vs current mean` table |
| Forecast      | `w` 22, span is `min(a,b) − sd .. max(a,b) + sd` and Mark is `(a+b)/2`; Text shows the clamped `lo..hi` | S03 `Offspring forecast (with an average mate)` |
| Lineage       | `w` 26, span covers the ancestor, parent and children means, Mark at the creature's own value, species colour | S08 trait rows |

## Interaction
None. Display only. The keys of the screen around it are in [keys.md](keys.md).

## Styling
- Whole bar drawn first in `theme::DIM` on `theme::PANEL_BG`: brackets, Below,
  Above.
- Span and Mark then recoloured in the caller's colour on `theme::PANEL_BG`.
  Trait rows use `common::trait_color(t)`; S08 uses the species colour from the
  roster. Mark and Span share a colour and differ only by glyph.
- Text: `theme::dim_text()`.

## Glyphs
`glyphs::BAR_L` `[`, `glyphs::BAR_R` `]`, `glyphs::SHADE_1` `░` for Below and
Above, `glyphs::SHADE_2` `▒` for Span, `glyphs::SHADE_4` `█` for Mark.

## Composition
*Contains:* nothing.
*Contained by:* [Table](table.md) rows (S03 genome, S04 spread), plain rows in
a [Panel](panel.md) body (S03 forecast, S08). Sits beside a
[Labeled Bar](labeled-bar.md) in the S03 genome row.

## API
### Today
```rust
widgets::bars::range(buf, x, y, w, min, mean, max, color)
```
Label and Text are written by the callers (`s03_inspector/genome.rs`,
`s04_species/summary.rs`, `s08_lineage.rs`) with `set_stringn`.

### Planned
```rust
RangeBar::new(0.62, 0.66, 0.75)          // min, mean, max
    .color(trait_color(0))
    .width(11)                            // default 13
    .render(buf, cell_area)

RangeBar::new(0.61, 0.71, 0.81).width(22).text()   // Forecast: adds " 0.61..0.81"
```
`height` is 1; `min_width` is `width`, plus 11 with `.text()`.

## Gaps today
- Takes `x, y, w` instead of a one-row `Rect`.
- Text is formatted by the S03 caller, not by the helper.
- No check that `min <= mean <= max`; a mean outside the span still draws
  Mark, and `min > max` draws no Span at all.
- Mark and Span share a colour, so on a one-cell span the mean is not
  distinguishable from the ends.

## Examples

### Species range (43 columns)
`w` 11, min 0.62, mean 0.66, max 0.75: cells 5, 5 and 6 of 9.
```
║ Speed       0.75 ±+0.09 [░░░░░█▒░░]     ║
```

### Spread (43 columns)
`w` 13, min 0.72, mean 0.82, max 0.88: cells 7, 8 and 9 of 11.
```
║ Speed       0.82   +0.02  [░░░░░░░▒█▒░] ║
```

### Forecast with Text, real S03 row (52 columns)
`w` 22, lo 0.61, centre 0.71, hi 0.81: cells 12, 13 and 15 of 20. The row is
the S03a Speed forecast row at the genome column's own width.
```
║ Speed       [░░░░░░░░░░░░▒█▒▒░░░░] 0.61..0.81    ║
```

### Lineage (43 columns)
`w` 26, min 0.66, mean 0.79, max 0.80: cells 15, 18 and 18 of 24. Mark lands
on the max cell, so Span shows on one side only.
```
║ +0.09      [░░░░░░░░░░░░░░░▒▒▒█░░░░░]   ║
```

### Narrowest, `w` 5 (43 columns)
min 0.2, mean 0.5, max 0.9: cells 0, 1 and 2 of 3.
```
║ Speed [▒█▒]                             ║
```

## Open questions
- Should Mark take `theme::TEXT_BRIGHT` like the base marker in
  [Labeled Bar](labeled-bar.md), so it reads on a one-cell span?
- Should the planned struct clamp mean into `min..=max`, or is drawing an
  out-of-span Mark a useful signal that the inputs disagree?
