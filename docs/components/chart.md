# Chart

Back to the [component index](README.md).

A time-series plot: one half-block cell per column over a labelled y-axis
gutter, an x axis with date labels, and shaded annotation bands for droughts and
epidemics. Only [S05](../screens/s05-population-charts.md) draws it, in five
variants a to e. Series are drawn with `▀ ▄ █` because Braille is banned: the
app is CP437 only.

## Anatomy
A Chart is inherently wide. The anatomy is shown at 63 columns inside the Panel
that contains it; the border is the Panel's.

```
║ <Legend>                                                    ║
║<YLab>┼<Plot>                            <Band><Annotation>  ║
║      │<Plot>                                                ║
║<YLab>┼<Plot>                                                ║
║      │<Plot>                                                ║
║<YLab>┼<Plot>                                                ║
║      ─┼─────────<Axis>──────┼──────────────────────────<Now>║
║<XLab>          <XLab>          <XLab>            <XLab>     ║
```

The gutter is six cells: a five-cell right-aligned label and the axis column.
The Plot fills the columns from there to one cell short of the right edge, and
the rows down to two short of the bottom. The Axis row and the XLab row are the
last two rows.

## Slots
| Slot       | Required | Position                                                                     | Overflow                                                            |
|------------|----------|------------------------------------------------------------------------------|---------------------------------------------------------------------|
| Legend     | no       | row 0, ` <swatch> name` groups, swatch `▀▄` in the series colour             | cut at the right edge                                               |
| YLab       | yes      | five-cell labels at the left edge on five rows (0, ¼, ½, ¾, max); six rows in Stacked | never dropped; below 12 columns the Chart is not drawn      |
| Plot       | yes      | `width − 7` columns by `height − 2` rows, one bin of days per column          | not drawn when the area is under 12 × 4                             |
| Band       | no       | every column whose bin holds a flagged day, background-tinted                | none                                                                |
| Annotation | no       | `<glyph> <word>` at the first Band column + 1, top Plot row                  | cut at 10 to 12 cells (`set_stringn` n) by variant                  |
| Axis       | yes      | row `height − 2`, `─` from the axis column to the Plot's right edge, `┼` under each XLab | never                                                   |
| Now        | no       | the last three cells of the Axis row, `now`                                  | overwrites the last `┼` tick                                        |
| XLab       | yes      | row `height − 1`, seven `Y## D###` labels centred under their ticks           | clamped inside the area; may touch at narrow widths                 |

## Sizing
Fills the area. `label_w` is 6 (five label cells and the axis column); the Plot
is `area.width − 7` columns wide, leaving one blank column at the right, and
`area.height − 2` rows tall. Minimum size 12 × 4; below that nothing is drawn.
Stacked adds a six-cell right axis (`R_AXIS_W`), so its Plot is `width − 12`,
and takes a Legend row plus a blank row above and two note rows below the XLab
row. The y maximum is the series maximum rounded up to `step` (prey 100,
predators 20, stacked 50, cases 10, resistance 1.0). Each column averages the
days in its bin `(c·n)/cols .. ((c+1)·n)/cols`; the value maps to half rows,
odd halves draw `▄` and even halves `▀`, and zero draws `▄` on the bottom row.
The window is 60, 240 or 720 days (`=` in, `-` out).

## Variants
| Variant       | What changes                                                                                                    | Used by |
|---------------|------------------------------------------------------------------------------------------------------------------|---------|
| a Time        | two single-series charts: prey (`theme::PREY`, step 100) above a [Divider](divider.md) `Predators (...)`, predators (`theme::PRED`, step 20) below; drought Band `¡ drought` | S05a |
| b Phase       | a scatter, x prey and y predators: older days `•` dim, the last 40 days `▄` in `theme::ACCENT`, today `█` in `theme::TEXT_BRIGHT`; a `•` crosshair through the means with `♦ equilibrium (p, q)`; four quadrant captions; four-cell x labels; no Now; three note rows under a Divider `Reading the orbit` | S05b |
| c Stacked     | per-species half-block columns stacked bottom-up in roster order, in roster colours; a cell whose halves share a species is `█`, otherwise `▄` with the lower species as fg and the upper as bg; vegetation `·` in `theme::VEGETATION` on the right axis `┼ NN%`; six YLab rows; drought Band | S05c |
| d Infections  | two multi-series charts split by a Divider: active cases per pathogen (step 10, one colour per slot) and mean Resistance per species (roster colours, max 1.0, labels `0.25` style) with a dotted base reference row `·` on every other column; epidemic Band `☻ epidemic` | S05d |
| e Groups      | not a time series: one six-row block per living species, a numbers row, a four-row `bars::histogram` (16 buckets of 4 columns) and a `─` axis with `┼` ticks labelled `1 4 8 12 16+`; six-cell gutter | S05e, see [Histogram](histogram.md) |
| Empty         | no samples yet: labels `0 .. step`, no XLab, Now still drawn                                                      | every variant before the first midnight sample |

## Interaction
| Key       | Standard                    | Here                                                     |
|-----------|-----------------------------|----------------------------------------------------------|
| `1`–`5`   | pick the n-th item          | pick variant a–e                                         |
| `g`       | shown in a Key Hint         | next variant                                             |
| `=` `-`   | more / less                 | zoom the window in / out; `+` and `_` also accepted. The same keys set speed on the main screen and do nothing elsewhere |
| `Esc`     | back                        | leave the screen                                         |

Today only `+` and `-` are handled, so zooming in needs Shift; the standard
in [keys.md](keys.md) accepts `=` as well.

## Styling
- Series: the caller's colour, bold. Totals use `theme::PREY` and
  `theme::PRED`; per-species series use the roster colour; vegetation
  `theme::VEGETATION`; pathogen slots rotate through `PATHOGEN_COLORS` in
  `infections.rs` (`theme::SICK`, `WARN`, `MAGENTA`, `INFO`, `ACCENT`, `ROSE`,
  `TAN`, `PREY`).
- YLab and XLab: `theme::dim_text()`.
- Axis, `│ ┼ ─`: `theme::border()` in Time, Stacked and Infections;
  `theme::DIM` in Phase.
- Now: `theme::ACCENT` on `theme::PANEL_BG`.
- Band: the column background becomes `theme::lerp(theme::PANEL_BG, theme::WARN, 0.22)`
  for a drought and `theme::lerp(theme::PANEL_BG, theme::SICK, 0.22)` for an
  epidemic; series cells drawn over it keep that background.
- Annotation: `theme::WARN` or `theme::SICK` on the Band background.
- Legend: swatch in the series colour, text `theme::text()`; the ` no predators
  yet` legend is `theme::dim_text()`.
- Phase: older `•` `theme::DIM`; recent `▄` `theme::ACCENT` bold; today `█`
  `theme::TEXT_BRIGHT` bold; crosshair `•` `theme::dim(theme::TEXT, 0.5)`;
  equilibrium label `theme::INFO`; captions `theme::dim_text()`; axis titles
  `theme::PRED` and `theme::PREY`.
- Reference row (d): `·` in `theme::dim(colour, 0.55)`.
- Right axis (c): `theme::VEGETATION`.

## Glyphs
`glyphs::HALF_UPPER` `▀`, `glyphs::HALF_LOWER` `▄`, `glyphs::FULL_BLOCK` `█`
for series and swatches; `glyphs::DOT` `·` for the vegetation line and
reference rows; `glyphs::BULLET` `•` for the Phase scatter and crosshair;
`glyphs::DIAMOND` `♦` for the equilibrium label; `glyphs::DROUGHT` `¡` and
`glyphs::DISEASE` `☻` for Annotations; `glyphs::H_LINE` `─`, `glyphs::V_LINE`
`│` and `glyphs::CROSS` `┼` for the axes. Groups uses `glyphs::SHADES` through
the Histogram. No glyph comes from ratatui today, and the planned `Chart` keeps it that
way unless the open question on wrapping ratatui is answered yes. `└` has no
`glyphs::` constant (Open questions).

## Composition
*Contains:* a Legend row; [Divider](divider.md) between the two sub-charts of
Time and Infections (drawn by the screen through `panel::section`);
[Histogram](histogram.md) in Groups.
*Contained by:* [Panel](panel.md), the 112-column S05 chart panel (inner 110).

## API
### Today
The line chart is a component (`src/widgets/chart.rs`):
```rust
widgets::chart::Series::new(&[f32]).color(c)
widgets::chart::Band::new(&[bool]).color(c).label(glyph, "drought")
widgets::chart::Chart::new().series(s).band(b).reference(v, c).y_step(100.0).y_max(v).y_label(&f).x_label(&f).legend(text)
impl Component for Chart<'_>          // pub const LABEL_W: u16 = 6
```
Stacked, Phase and Groups are still hand-drawn and private to the S05 module,
as are the older `line_chart` / `multi_line_chart` helpers they share:
```rust
// src/ui/screens/s05_charts/time.rs
fn line_chart(f: &mut Frame<'_>, area: Rect, w: &Window<'_>, series: &[f32], color: Color, step: f32)
// src/ui/screens/s05_charts/infections.rs
fn multi_line_chart(f: &mut Frame<'_>, area: Rect, w: &Window<'_>, series: &[(Vec<f32>, Color)], reference: &[(f32, Color)], y_max: f32, y_label: impl Fn(f32) -> String)
// src/ui/screens/s05_charts/phase.rs
pub(super) fn phase_chart(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>)
// src/ui/screens/s05_charts/stacked.rs
pub(super) fn stacked_chart(f: &mut Frame<'_>, area: Rect, sim: &Sim, w: &Window<'_>)
// src/ui/screens/s05_charts/groups.rs
pub(super) fn group_chart(f: &mut Frame<'_>, area: Rect, sim: &Sim)
// src/ui/screens/s05_charts.rs
struct Window<'a>                       // the visible slice: samples, prey, pred, veg, drought, epidemic
fn round_up(v: f32, step: f32) -> f32
```
`Window` supplies `bin(c, cols)`, `mean_over(v, c, cols)`, `drought_in`,
`epidemic_in` and `day_label(i)`; `day_label` goes through
`ui::screens::common::day_stamp`.

### Planned
One `Chart` struct that lifts the existing hand-drawn renderer out of S05, so
the half-row precision and CP437 rules stay exactly as they are. Whether its
line pass should be a ratatui `Chart` with `Marker::HalfBlock` underneath is an
open question below; the builder is the same either way.
```rust
Chart::new()
    .series(Series::new(&prey).color(theme::PREY))            // one or more
    .series(Series::new(&pred).color(theme::PRED))
    .y_step(100.0)                                            // y max = max rounded up to step
    .y_label(&|v| format!("{v:.0}"))                          // default; "{v:.2}" for resistance
    .y_max(1.0)                                               // or a fixed maximum
    .x_label(&|i| window.day_label(i))                        // the label for sample i; seven are placed
    .band(Band::new(&drought).color(theme::WARN).label(glyphs::DROUGHT, "drought"))
    .reference(0.35, color)                                   // dotted row, variant d
    .legend(Text::new(" Prey (voles + hares + deer)"))       // optional row 0
    .render(buf, area)
```
Series are drawn with `glyphs::HALF_UPPER` / `glyphs::HALF_LOWER` as
`line_chart` does today; both axes take `theme::border()` with
`theme::dim_text()` labels. Bands, Annotations, reference rows and Now are
painted after the series pass. `height` is the minimum, 4 (a chart takes
what its container gives; use `Fill`); `min_width` is 12. Stacked (c) and
Groups (e) do not fit a `Dataset`; they keep their own structs, `StackedChart`
and [Histogram](histogram.md). `line_chart` and `multi_line_chart` in S05
are thin adapters over `Chart` now.

## Gaps today
- Every variant is a private function in S05; nothing is shared with the
  other screens.
- The prototype renders S05a and S05b draw a seven-cell gutter (`   400 │`), a
  `└` corner, no `┼` ticks and no `now` overlap; the live code draws a six-cell
  gutter (`  400┼`), `┼` at each label row and each x tick, and `─` in the
  corner. The S05c render matches the live axis. The first three examples below
  are cut from the renders; the last follows the live rule.
- Axis colour is `theme::border()` in a, c and d but `theme::DIM` in b.
- The lower Time chart's row 0 is a Legend row that only ever holds ` ` or
  ` no predators yet`.
- Annotation width is 10, 11 or 12 cells by variant.
- The S05a render's `[1-3] pick` and `chart 1/3` predate variants d and e.
- Only Stacked computes the "each column averages ~N days" note.
- `=` and `_` are not handled; only `+` and `-` zoom. See [keys.md](keys.md).

## Examples

### Time, upper chart (63 columns)
Cut from the S05a render: Legend row, five YLab rows for step 100, the Plot in
`theme::PREY`, the Axis and four of the seven XLabs.
```
║  Prey (voles + hares + deer)                                ║
║   400 │                                                     ║
║       │▄▄▄▄▄                                                ║
║       │    ▀▀█▄▄                                            ║
║       │        ▀▀▄                                          ║
║   300 │           ▀▄                                        ║
║       │             █▄                                      ║
║       │              ▀▄                                     ║
║       │               ▀█                                    ║
║   200 │                 █▄                                  ║
║       │                  ▀█                                 ║
║       │                    ▀▄                               ║
║       │                     ▀▄▄                             ║
║   100 │                        █▄▄                          ║
║       │                           ▀▀▀▄▄▄    ▄▄  ▄           ║
║       │                                 ▀▀▀▀▀ ▀▀▀▀▀▀▀▀▀▀▀▀█▀║
║       │                                                     ║
║     0 │                                                     ║
║       └─────────────────────────────────────────────────────║
║Y11 D124                 Y11 D164      Y11 D204      Y11 D244║
```

### Time, Annotation and Now, columns 63 to 112 of S05a (50 columns)
The top three Plot rows and the last three rows of the same chart. The drought
Band tints the columns under `¡ drought`; the tint does not show in text.
```
           ¡ drought                             ║
                                                 ║
                                                 ║
                                                 ║
──────────────────────────────────────────────now║
      Y11 D284      Y11 D324             Y12 D004║
```

### Stacked, top and bottom rows (61 columns)
Cut from the S05c render: the Legend row, the blank row, the top two Plot rows,
the bottom three Plot rows, the Axis with `┼` ticks and the XLab row.
```
║ █ Voles   █ Hares   █ Deer   █ Foxes   █ Wolves   █ Lynxes║
║                                                           ║
║  450┼                                                     ║
║     │    ▄▄                                               ║
║     │█████████████████████████████▄▄▄█████▄▄█▄▄▄███▄▄███▄▄║
║     │█████████████████████████████████████████████████████║
║    0┼█████████████████████████████████████████████████████║
║     ─┼───────────────┼───────────────┼────────────────┼───║
║  Y11 D124        Y11 D164        Y11 D204         Y11 D244║
```

### Empty, live axis rule (43 columns)
An area of 41 × 7 with no samples and step 100, as `line_chart` draws it: five
YLab rows with `┼`, the Axis with the `┼` tick of the first label and `now`
over the last one, and a blank XLab row.
```
║  100┼                                   ║
║   75┼                                   ║
║   50┼                                   ║
║   25┼                                   ║
║    0┼                                   ║
║     ─┼──────────────────────────────now ║
║                                         ║
```

## Open questions
- Corner glyph: `└` as in the prototype and ratatui's `Axis`, or `─` as live?
  `glyphs::BOX_BL` `└` exists since the [Race Chart](race-chart.md) landed, so
  the choice is now only which look the pinned tests fix.
- Which axis look do the pinned tests fix, the prototype's or the live one? The
  S05a and S05b renders need regenerating either way.
- Fixture series: a pinned test needs the 240-day series behind each example.
  The S05 screen doc points to `src/fixtures/series.rs`, which no longer exists.
- Can ratatui 0.30's `Chart` reproduce the hand code's half-row rounding and
  the flat-zero `▄`? If not, the wrapper keeps the hand path and borrows only
  the marker.
- Should a Band tint the series cells too (a, d) or only the empty cells (c)?
