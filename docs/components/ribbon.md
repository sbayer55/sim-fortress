# Ribbon

Back to the [component index](README.md).

One value per tick of a chase, plotted over the chase clock: the top row is
the escape edge, the bottom row the kill edge, so a hunter closing in is a
line diving and a prey holding on is a line refusing to fall. A few stalk
ticks on a grey ground lead up to the rule where the clock started; the clock
zone that follows ends at `┤`, the clock expiring. [S17 Hunt
Watch](../screens/s17-hunt-watch.md) draws one per lane at two cells per tick.

## Anatomy

```
<stalk>│<clock zone ................>┤
■─■─   │■─■─■─■─                    ┤
       │        ■─■─►─              ┤
       │                            ┤
       │                    10      ┤
```

Every tick is a Mark in its first cell and a Connector in the rest of its
pitch. Row 0 is the escape edge (value 0), the last row the kill edge
(value 1). The rule `│` separates the last stalk ticks from the clock; `┤`
closes the clock.

## Slots
| Slot      | Required | Position                                                                        | Overflow |
|-----------|----------|---------------------------------------------------------------------------------|----------|
| Stalk     | no       | the last `stalk_ticks` values, right-aligned to the rule, `seg` cells per tick   | older stalk values are dropped from the left |
| Rule      | yes      | one column after the stalk zone, `│` on every row                               | — |
| Clock     | no       | value `j` at column `rule + 1 + seg·j`; `chase_max` ticks wide                  | values past `chase_max` are not drawn |
| Clock out | yes      | `┤` on every row, one column after the clock zone                               | — |
| Labels    | yes      | every tenth tick's number on the kill edge, at the tick's first cell            | cut at the clock out |
| Mark      | yes      | `■` at row `round(value × (rows − 1))`; the newest tick is `►` while live       | a value outside 0..1 is clamped |
| Connector | yes      | `─` in the remaining cells of the pitch; `│` between rows when the value jumps  | — |
| Outcome   | no       | the final tick's glyph on its edge: `x` kill (bottom), `→` escaped or timed out (top), `·` lost (top) | — |
| Caption   | no       | centred on the middle row over the clock zone                                   | cut to the clock zone |

## Sizing
Height is `rows` (default 5, minimum 2). Minimum width is
`seg × (stalk_ticks + chase_max) + 2`: 74 for the S17 defaults (6 stalk ticks,
a 30-tick clock, two cells per tick). The component never writes outside its
area; a narrower area cuts the clock zone on the right.

## Variants
| Variant   | What changes                                                            | Used by |
|-----------|-------------------------------------------------------------------------|---------|
| Live      | full colours, the newest tick is `►`                                    | S17 lanes while the hunt is open |
| Resolved  | the final tick shows the outcome glyph on its coloured edge             | S17 on the tick a hunt ends |
| Remembered| `live(false)`: every colour pulled .45 toward `theme::PANEL_BG`, usually with a Caption of the hunter's state | S17 lanes between hunts |
| Narrow    | fewer stalk ticks, a shorter clock (`stalk_ticks`, `chase_max`)         | the sheet examples; a future S03b strip |

## Interaction
None. Display only. The keys of the screen around it are in [keys.md](keys.md).

## Styling
- Grounds: stalk zone `theme::lerp(PANEL_BG, DIM, .35)`, clock zone
  `lerp(PANEL_BG, INFO, .25)`; the escape row is tinted toward `theme::GOOD`
  (.15 stalk, .18 clock) and the kill row toward `theme::BAD` (.18, .22).
- Rule: `theme::WARN`. Clock out: `theme::BAD`, bold. Labels: `theme::DIM`.
- Mark: `theme::lerp(GOOD, BAD, value)` on the cell's ground; Connector the
  same colour pulled .5 toward the ground; a row connector .4.
- Newest live tick: `theme::TEXT_BRIGHT`, bold. Outcome: `TEXT_BRIGHT` bold on
  the ground pulled .6 toward `BAD` (kill), `GOOD` (escaped, timed out) or `DIM`
  (lost).
- Caption: `theme::TEXT` on the ground. Remembered: every colour above through
  `lerp(colour, PANEL_BG, .45)`.

## Glyphs
`glyphs::SQUARE` `■` marks, `glyphs::PLAY` `►` the newest tick,
`glyphs::H_LINE` `─` and `glyphs::V_LINE` `│` connectors and the rule,
`glyphs::CLOCK_OUT` `┤`, `glyphs::DEATH` `x`, `glyphs::RIGHT` `→`,
`glyphs::DOT` `·` outcomes.

## Composition
*Contains:* nothing.
*Contained by:* a lane of [S17](../screens/s17-hunt-watch.md), between two
one-cell rules.

## API
### Today
```rust
widgets::ribbon::Ribbon::new(&stalk, &chase).outcome(Some(RibbonEnd::Kill)).live(false).caption("cooldown 5 h").chase_max(30).stalk_ticks(6).seg(2).rows(5)
impl Component for Ribbon<'_>   // height rows, min_width seg × (stalk_ticks + chase_max) + 2
```

### Planned
As today; the sheet and the struct landed together.

## Gaps today
none

## Examples

### A live chase closing on the kill (30 columns)
Two stalk ticks, then seven chase ticks of a twelve-tick clock; the values
rise from .30 to .85, so the line dives toward the kill edge and the newest
tick is `►`.
```
    │                        ┤
■─■─│                        ┤
    │■─■─■─■─                ┤
    │        ■─■─►─          ┤
    │                    10  ┤
```

### Resolved and remembered (30 columns)
The prey got away: the final tick is pinned to the escape edge as `→`, every
colour is pulled toward the panel ground, and the hunter's state is the
caption on the middle row.
```
    │        →─              ┤
■─■─│■─■─■─■─                ┤
    │      cooldown 5 h      ┤
    │                        ┤
    │                    10  ┤
```

### The S17 lane (74 columns)
The defaults: six stalk ticks, a thirty-tick clock labelled at 10 and 20, two
cells per tick, eight chase ticks in.
```
            │                                                            ┤
■─          │                                                            ┤
  ■─■─■─■─■─│■─■─■─■─                                                    ┤
            │        ■─■─■─►─                                            ┤
            │                    10                  20                  ┤
```

## Open questions
- Whether the clock zone should shade its remaining ticks darker as the clock
  runs down, so time pressure shows without reading the labels.
