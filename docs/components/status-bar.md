# Status Bar

Back to the [component index](README.md).

The bottom row of every screen: `[key] label` pairs from the left and a
right-aligned message. Every screen under [../screens](../screens/README.md)
draws one; modals repaint it undimmed over the backdrop.

## Anatomy
The bar is inherently wide. Its examples are 155 or 60 columns, stated per
heading.

```
 [<Key>] <Label>  [<Key>] <Label>                          <Right> 
```

## Slots
| Slot  | Required | Position                                                                    | Overflow                                           |
|-------|----------|-----------------------------------------------------------------------------|----------------------------------------------------|
| Key   | yes      | `[Key]`; the first pair starts at column 1, each later pair follows the previous Label's two trailing cells | cut at the right edge; Right overwrites it |
| Label | yes      | one cell after `]`, then two trailing cells                                 | same                                               |
| Right | no       | right-aligned with one trailing cell                                        | cut to the area width, losing its tail             |

Spacing is exactly ` [k] label  ` per pair: a single leading cell for the row,
then `[k]`, one cell, the label, two cells.

## Sizing
Height 1. Fills the area width; today always the terminal width, 155. The
pairs are one line clipped at the edge. Right takes `len + 1` cells, capped at
the area width, and is drawn last. No width parameters.

## Variants
| Variant        | What changes                                                       | Used by                          |
|----------------|--------------------------------------------------------------------|----------------------------------|
| Default        | Right in `theme::ACCENT`                                           | S00, S03, S04, S07, S11, S12     |
| Coloured right | Right in a caller colour, `render_colored`                         | S01 and S10 clock (day/night tint) |
| No right       | Right empty; pairs only                                            | any screen with nothing to report |
| Modal repaint  | the row is refilled with `theme::STATUS_BG` and redrawn after the modal box, so it is not dimmed | S10, S11, S12 |

## Interaction
None. Display only, but it is where rule 4 of [keys.md](keys.md) is met: every key the
screen handles appears here or in a [Key Hint](key-hint.md), spelled as that sheet's
table says.

## Styling
- Whole row filled with `theme::STATUS_BG`.
- Key: `theme::KEY`, bold, on `theme::STATUS_BG`. Built by hand because
  `theme::key()` carries `theme::PANEL_BG`.
- Label: `theme::TEXT` on `theme::STATUS_BG`.
- Right: `theme::ACCENT` on `theme::STATUS_BG`, or `right_fg`.

## Glyphs
None of its own. Key names carry `glyphs::UP` `↑`, `glyphs::DOWN` `↓`,
`glyphs::LEFT` `←` and `glyphs::RIGHT` `→` as literal strings; S12's Right
starts with `glyphs::PAUSE_STR` `││`; S01's Right ends with `glyphs::SUN` `☼`
or `glyphs::MOON` `○`. Brackets are ASCII.

## Composition
*Contains:* [Key Hint](key-hint.md) tokens.
*Contained by:* screen layouts, always the last row. [Modal](modal.md)
screens draw it again.

## API
### Today
```rust
widgets::status::render(f, area, keys: &[(&str, &str)], right: &str)
widgets::status::render_colored(f, area, keys: &[(&str, &str)], right: &str, right_fg: Color)
```

### Planned
```rust
StatusBar::new(&[("k", "look"), ("o", "overlay")])
    .right("Year 12, Day 4 of Autumn  14:00  ☼ day")   // optional
    .right_color(theme::ACCENT)                         // default
    .render(buf, row_area)
```
`height` is 1; `min_width` is Right plus one cell plus the first pair.

## Gaps today
- On collision Right overwrites the pairs under it. The spec drops whole
  pairs from the right until the rest fits, then cuts Right.
- Key style is hand-built rather than shared with [Key Hint](key-hint.md).
- Speed is written `[+/-] speed`; the standard spelling in [keys.md](keys.md)
  is `[-/=] speed`, since Shift is optional for both keys.
- Frame entry point only.
- Each modal refills the row itself before calling `render`.
- S01's live default key set differs from the S01a prototype render; the
  examples below use the render's set.

## Examples

### Default, from S00a (155 columns)
```
 [↑↓] select  [Enter] confirm  [q] quit                                                                                                    no world loaded 
```

### Coloured right, from S01a (155 columns)
Right is the clock in the day/night tint colour.
```
 [k] look  [o] overlay  [Space] pause  [+/-] speed  [s] species  [g] graphs  [e] events  [?] help                   Year 12, Day 4 of Autumn  14:00  ☼ day 
```

### Modal repaint, from S12a (155 columns)
```
 [Enter] select  [←→] move  [l] lineage  [Space] pause  [Esc] continue                                                             ││ paused on extinction 
```

### Default, from S03a (155 columns)
```
 [f] follow  [l] lineage  [Tab] next creature  [←→] panel  [↑↓] scroll  [Esc] back                             Sedge d#494  Year 2, Day 1 of Spring  06:00 
```

### Short (60 columns)
The S00 pairs and Right at a narrower width.
```
 [↑↓] select  [Enter] confirm  [q] quit     no world loaded 
```

### Narrow, pairs dropped [Planned] (60 columns)
Live S01 keys `k look, Tab wide, ←→↑↓ scroll, ...` with the clock. Only the
first pair fits beside Right; today the clock would overwrite the pairs.
```
 [k] look            Year 12, Day 4 of Autumn  14:00  ☼ day 
```

## Open questions
- When pairs are dropped, should the bar show a marker (a CP437 one such as
  `»`; `…` is not in CP437), or nothing, as the spec says now?
- Should Right ever wrap the pairs onto a second row on narrow terminals, or
  is 155 × 45 fixed for good?
