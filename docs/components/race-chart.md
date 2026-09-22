# Race Chart

Back to the [component index](README.md).

Several running totals racing each other: one point per sample at a two-cell
pitch, joined by step lines, with one *lead* series drawn bold on top of dim
rivals. [S16 Top Dynasties](../screens/s16-top-dynasties.md) draws six of them,
one per stat, for the selected line against the region's other dynasties.

## Anatomy

```
<Title> <Values>
│<Max>              ┌■─■
│               ┌■─■┘
│         ┌■─■─■┘   ┌∙─∙
│     ┌■─■┘∙─∙─∙─∙─∙─∙─∙
│■─■─■┘∙─∙┘
└<Labels>──────────────
```

Row 0 is the Title row; the plot rows follow (5 by default), and the last row
is the Foot. Column 0 of the plot and the Foot is the axis.

## Slots
| Slot   | Required | Position                                                                 | Overflow |
|--------|----------|--------------------------------------------------------------------------|----------|
| Title  | yes      | row 0 from column 0, `theme::label()`                                    | cut at the right edge |
| Values | no       | after Title, one per series in the order given: a blank, the series' point glyph, its last value | cut at the right edge; a series with no values shows nothing |
| Max    | yes      | plot row 0, column 1: the y maximum through the value formatter, `theme::dim_text()` | a series that reaches the top in the first samples overwrites it |
| Points | yes      | sample `i` at column `1 + 2·(i − first)`, row `rows − 1 − round(v / max · (rows − 1))` from the top of the plot | only the newest `⌊width / 2⌋` samples are shown; older ones are dropped from the left |
| Steps  | yes      | the column before a point: `─` when the level is unchanged, else `┘` on the lower row, `┌` on the upper row and `│` between when rising, `┐` upper and `└` lower when falling | a step before the first visible sample is not drawn |
| Labels | yes      | Foot: `└` at column 0, `─` to the right edge, then every second visible sample's label (`first_label + i`) left-aligned at the sample's column | cut at the right edge |

Series are drawn rivals first, in the order given, then every lead series, so a
lead point or step always wins a cell.

## Sizing
Height is `rows + 2` (`rows` defaults to 5, minimum 1). Width fills the area;
minimum 5. The y maximum is the largest value over every series (never below
1), or the fixed `y_max`. A series with `start` `n` has its first value at
sample `n`. The component fills its area with `theme::PANEL_BG` and never
writes outside it.

## Variants
| Variant | What changes | Used by |
|---------|--------------|---------|
| Small   | 35 × 7, three rivals, values in the title row | S16 race grid |
| Wide    | full panel width, the same rules | S16 (a one-stat variant, not built) |
| Formatted | `value_fmt` writes the title values and the maximum, for example `6y` for ages in days | S16 age chart |

## Interaction
None. Display only. The keys of the screen around it are in [keys.md](keys.md).

## Styling
- Title: `theme::label()`. Max and Labels: `theme::dim_text()`. Axis and Foot
  rule: `theme::border()`.
- A lead series: its colour, bold. A rival: `theme::dim(colour, 0.55)`. Steps
  take their series' style.
- Background: `theme::PANEL_BG` everywhere.

## Glyphs
`glyphs::SQUARE` `■` lead points, `glyphs::TRAIL` `∙` rival points,
`glyphs::H_LINE` `─`, `glyphs::V_LINE` `│`, `glyphs::BOX_TL` `┌`,
`glyphs::BOX_TR` `┐`, `glyphs::BOX_BL` `└` (steps and the Foot corner),
`glyphs::BOX_BR` `┘`.

## Composition
*Contains:* nothing.
*Contained by:* a [Panel](panel.md) body, in a grid or alone.

## API
### Today
```rust
widgets::race_chart::RaceSeries::new(&[f32]).start(n).color(c).lead(true)
widgets::race_chart::RaceChart::new("kills").series(s).rows(5).first_label(1).y_max(v).value_fmt(&f)
impl Component for RaceChart<'_>   // height rows + 2, min_width 5
```

### Planned
As today; the sheet and the struct landed together.

## Gaps today
none

## Examples

### Lead and three rivals (35 columns)
Twelve samples of four running totals; the lead ends at 243. The rivals are
listed in the title in the order given, the lead last.
```
kills ∙102 ∙89 ∙87 ■243            
│243                ┌■─■           
│               ┌■─■┘              
│         ┌■─■─■┘   ┌∙─∙           
│     ┌■─■┘∙─∙─∙─∙─∙─∙─∙           
│■─■─■┘∙─∙┘                        
└1───3───5───7───9───11────────────
```

### Rising and falling steps, a late start (21 columns)
The lead rises, holds, falls, rises and falls again; the rival starts at
sample 2 and its early cells are overwritten by the lead drawn on top.
```
young ■3 ∙1          
│6      ┌■─■┐        
│ ┌■─■┐∙│   │        
│ │   │ │   └■       
│■┘   └■┘∙─∙─∙       
│                    
└1───3───5───7───────
```

## Open questions
- Whether a series should be able to opt out of the title values.
