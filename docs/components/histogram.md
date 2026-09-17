# Histogram

Back to the [component index](README.md).

A small vertical bar chart: one column per bucket, built from `█` and `▄`
stacks so each row carries two levels, over an axis row with a `┼` at the
mean and a footer with the bucket count, sample size and mode. Twelve of them
fill the left panel of [S04b](../screens/s04-species-browser.md) (one per
trait); a wider variant with tick labels shows group sizes in
[S05e](../screens/s05-population-charts.md).

## Anatomy

```
<Header> 
<Columns>
<Columns>
<Axis>   
<Footer> 
```

Header is `<Name> <Min> <Mean> <Max>`; Axis carries the `<Mark>` cell; Footer
is `0 <N> <Mode> 1`. Columns rows repeat for the chart height.

## Slots
| Slot    | Required | Position                                                        | Overflow                                              |
|---------|----------|-----------------------------------------------------------------|-------------------------------------------------------|
| Header  | no       | row 0; Name padded to 11, then `.mm .mm .mm` for min, mean, max | Name cut at 11; the figures cut at 14                 |
| Columns | yes      | `h` rows; bucket `i` at `x = i × col_w`, `col_w − 1` cells wide | buckets whose `x + col_w` passes the right edge are dropped |
| Axis    | no       | row after Columns, `─` across the full width                     | never cut; spans the width                           |
| Mark    | no       | on Axis at `round(mean × (width − 1))`, clamped to the last cell | never cut                                             |
| Footer  | no       | last row: `0` at 0, `n=…` at 2 (7 cells), `mode .mm` at 11 (11 cells), `1` at the last cell | each figure cut at its own cell budget |

## Sizing
Width is `values.len() × col_w`; in S04b that is `12 × 2 = 24`. Height is
`h + 3` with Header, Axis and Footer, or `h` for the bare Columns (`h` is the
area height given to the helper; 4 in S04b, so each block is 7 rows). Minimum
`col_w` is 1, which gives no gutter. Minimum `h` is 1.

Column height uses half cells:

- `halves = round(v / max × (h × 2))`, where `max` is the largest bucket, at
  least 1.
- For row `r` counted from the bottom, `level = halves − 2r`.
- `level >= 2` draws `█`, `level == 1` draws `▄`, anything else draws nothing.

So a column shows `halves / 2` full cells and one `▄` on top when `halves` is
odd. A bucket smaller than `max / (4h)` rounds to zero halves and draws
nothing. The mode in Footer is the centre of the tallest bucket,
`(peak + 0.5) / 12`, and ties go to the last bucket.

## Variants
| Variant  | What changes                                                                          | Used by                     |
|----------|---------------------------------------------------------------------------------------|-----------------------------|
| Trait    | `col_w` 2, `h` 4, twelve buckets over `0..1`, Header, Mark and Footer as above         | S04b, one block per trait   |
| Group    | `col_w` 4, `h` from the area, sixteen buckets; Axis carries `┼` ticks with labels `1 4 8 12 16+` and the peak count sits left of the chart; no Header or Footer | S05e group sizes |
| Bare     | Columns only                                                                          | what `bars::histogram` draws today |
| Empty    | no Columns when the species count is 0; Header, Axis and Footer still drawn           | S04b for an extinct species |

## Interaction
None. Display only. The keys of the screen around it are in [keys.md](keys.md).

## Styling
- Columns: the caller's colour on `theme::PANEL_BG`. Trait blocks use
  `common::trait_color(t)`; S05e uses the species colour from the roster.
- Header Name: the same colour, `Modifier::BOLD`. Header figures:
  `theme::dim_text()`.
- Axis: `theme::border()`. Mark: `theme::TEXT_BRIGHT` on `theme::PANEL_BG`.
  S05e ticks take `theme::border()` and their labels `theme::dim_text()`.
- Footer: `theme::dim_text()`.

## Glyphs
`glyphs::FULL_BLOCK` `█`, `glyphs::HALF_LOWER` `▄`, `glyphs::H_LINE` `─`,
`glyphs::CROSS` `┼`.

## Composition
*Contains:* nothing.
*Contained by:* [Panel](panel.md) bodies, laid out by S04b as a
[Columns](columns.md) Grid of three `Fixed(26)` columns, and stacked per
species in S05e.

## API
### Today
```rust
widgets::bars::histogram(buf, area, values: &[u16], color, col_w)
```
Draws Columns only. Header, Axis, Mark and Footer are hand-drawn in
`ui::screens::s04_species::histograms::histograms`; the S05e axis with ticks is
hand-drawn in `ui::screens::s05_charts::groups::species_block`.

### Planned
```rust
Histogram::new(&buckets)                 // &[u16]
    .color(trait_color(0))
    .col_w(2).rows(4)                    // defaults shown
    .header("Speed", 0.67, 0.72, 0.73)   // Name, min, mean, max
    .axis().mark(0.72)                   // Axis with Mark at the mean
    .footer()                            // 0  n=  mode  1
    .render(buf, area)

Histogram::new(&sizes).col_w(4).rows(h).ticks(&[(0, "1"), (3, "4"), (7, "8")])   // Group; ticks imply the Axis
```
`height` is `rows` plus one for each of header, axis and footer; `min_width`
is `buckets.len() × col_w`. The Footer mode is `(peak + 0.5) / buckets.len()`,
so a twelve-bucket trait histogram reads as the sheet says and S05e's
sixteen buckets map to their own scale.

## Gaps today
- S04b and S05e still draw their header, axis and footer rows by hand around
  `bars::histogram` (now the Bare wrapper) until those screens move.
- A bucket with a few members next to a large peak disappears (rounds to zero
  halves) with nothing to say it is non-empty.
- `col_w` 1 columns touch, so neighbouring buckets are not separable.

## Examples
Each block is a real S04b block (24 cells wide) placed in a 43-column panel
with one cell of left margin, which is how S04b places its first column.

### Trait, one bucket (43 columns)
Buckets `0 0 0 0 0 0 0 0 10 0 0 0`, `col_w` 2, `h` 4, min .67, mean .72, max
.73. Bucket 8 is at offset 16; Mark at `round(0.72 × 23) = 17`.
```
╔ Speed ══════════════════════════════════╗
║ Speed      .67 .72 .73                  ║
║                 █                       ║
║                 █                       ║
║                 █                       ║
║                 █                       ║
║ ─────────────────┼──────                ║
║ 0 n=10     mode .71    1                ║
╚═════════════════════════════════════════╝
```

### Trait, half cells (43 columns)
Buckets `0 0 0 0 0 1 5 1 3 0 0 0`, min .45, mean .60, max .68. Bucket 8 is
`round(3 / 5 × 8) = 5` halves: two `█` and a `▄`. Buckets 5 and 7 are 2
halves each.
```
║ Metabolism .45 .60 .68                  ║
║             █                           ║
║             █   ▄                       ║
║             █   █                       ║
║           █ █ █ █                       ║
║ ──────────────┼─────────                ║
║ 0 n=10     mode .54    1                ║
```

### Trait, two buckets (43 columns)
Buckets `0 0 0 4 0 6 0 0 0 0 0 0`, min .31, mean .39, max .50. Bucket 3 is
`round(4 / 6 × 8) = 5` halves.
```
║ Size       .31 .39 .50                  ║
║           █                             ║
║       ▄   █                             ║
║       █   █                             ║
║       █   █                             ║
║ ─────────┼──────────────                ║
║ 0 n=10     mode .46    1                ║
```

### Bare, `col_w` 4 (24 columns)
Six buckets `5 3 1 0 2 1`, `h` 3. Each column is 3 cells wide with a one-cell
gutter; bucket 2 is `round(1 / 5 × 6) = 1` half.
```
███                     
███ ███                 
███ ███ ▄▄▄     ███ ▄▄▄ 
```

## Open questions
- Should a non-empty bucket that rounds to zero halves draw a `▄` anyway, so
  the eye can tell "few" from "none"?
- The Footer `mode` assumes twelve buckets over `0..1`; the planned struct
  needs a bucket-to-value mapping so S05e can share it.
- `glyphs::FULL_BLOCK` and `glyphs::SHADE_4` are the same `█`; which name
  should the planned component use?
