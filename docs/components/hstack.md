# HStack

Back to the [component index](README.md).

A horizontal stack: lays its children left to right across one area, each as
wide as its constraint says. It replaces the hand-built `Rect::new` splits
that divide screens into panels and the fixed column offsets inside rows. The
map beside the sidebar on [S01](../screens/s01-world-map.md), the three
columns of [S03](../screens/s03-creature-inspector.md), the two-column
[Legend](legend.md), and every [Table](table.md) row are HStacks.

## Anatomy

```
<Child 1    ><Child 2          ><Child 3      >
```

Each child takes the full height of the area and starts on the column after
the previous child ends, plus `gap` blank columns.

## Slots
| Slot     | Required | Position                                                          | Overflow                                                 |
|----------|----------|-------------------------------------------------------------------|----------------------------------------------------------|
| Children | yes, 1+  | left to right in order; each gets `(w, height)` where `w` is its constraint | children past the right edge are not drawn; a child that straddles it is clipped by its own area |

## Sizing
Height is the area height and every child gets all of it. Width is the sum of
the children's widths plus the gaps.

| Constraint | Width given to the child                                                      |
|------------|-------------------------------------------------------------------------------|
| `Auto`     | `child.min_width()` (default)                                                 |
| `Fixed(n)` | exactly `n` columns                                                           |
| `Fill(w)`  | a share of whatever is left after `Auto` and `Fixed` children, by weight `w`; the remainder goes to the last `Fill` child |

`gap` defaults to 0 because panels abut edge to edge in this UI.
`height(width)` is the tallest child's height at its own width. `min_width` is
the sum of the `Auto` and `Fixed` widths.

## Variants
| Variant   | What changes                                                  | Used by                                     |
|-----------|---------------------------------------------------------------|---------------------------------------------|
| Split     | children are Panels; `Fixed` sidebar beside a `Fill` map      | S01 map + 43-column sidebar, S03 columns, S04b |
| Row       | children are cells of one row: label, bar, value              | [Labeled Bar](labeled-bar.md) internals, Table rows |
| Columns   | equal `Fill(1)` children holding VStacks                       | Legend, S06 Terrain, S11 help               |

## Interaction
None. Display only. Keys reach the children through the screen; see
[keys.md](keys.md).

## Styling
None of its own. An HStack paints nothing. Use [Spacer](spacer.md) for
deliberate blank columns.

## Glyphs
None.

## Composition
*Contains:* any component, including [VStack](vstack.md), another HStack,
[Panel](panel.md) and [Spacer](spacer.md).
*Contained by:* screen layouts, [Panel](panel.md), [Modal](modal.md),
[VStack](vstack.md) rows, [Table](table.md) rows.

## API
### Today
No helper. Screens compute rectangles by hand:
```rust
// src/ui/screens/s03_inspector.rs
let left  = Rect::new(area.x, area.y, LEFT_W, body_h);
let mid   = Rect::new(area.x + LEFT_W, area.y, MID_W, body_h);
let right = Rect::new(area.x + LEFT_W + MID_W, area.y, area.width - LEFT_W - MID_W, body_h);
```
About 50 such splits exist under `src/ui/screens`. Row cells use fixed
`inner.x + n` offsets. ratatui `Layout` is not used anywhere.

### Planned
```rust
HStack::new()
    .child_with(Fill(1), map_panel)       // takes the rest
    .child_with(Fixed(43), sidebar_panel)
    .gap(0)                               // default
    .render(buf, area)

HStack::new()                             // a row
    .child_with(Fixed(12), Text::new(" vegetation"))
    .child_with(Fixed(20), Bar::new(0.44).color(theme::VEGETATION))
    .child(Spacer::cols(3))
    .child_with(Fixed(4), Text::new(" 44%").right())
```
Constraints are the same enum VStack uses. `Auto` asks `min_width()`. Rows
that must line up with other rows do not use their own constraints; they draw
against a shared [Columns](columns.md) spec instead.

## Gaps today
- Nothing shared; every split is arithmetic in the screen, and the three
  S03 column widths are constants the screen owns.
- Row cells are placed by literal offsets, so a change to `label_w` in one
  place does not move the cells after it.

## Examples

### Split, two Fill(1) panels (43 columns)
Inner width 41 splits 20 and 21; the odd cell goes to the last Fill child.
```
╔ Split ══════════════════════════════════╗
║┌ Left ────────────┐┌ Right ────────────┐║
║│ a                ││ b                 │║
║└──────────────────┘└───────────────────┘║
╚═════════════════════════════════════════╝
```

### Row, a Labeled Bar as cells (43 columns)
Fixed(12), Fixed(20), Spacer(3), Fixed(4), then Fill for the rest.
```
║ vegetation [████████░░░░░░░░░░]    44%  ║
```

### Columns, one Legend row (43 columns)
Fixed(20) and Fill(1), each holding a glyph and a name.
```
║ ≈ deep water       ~ shallow water      ║
```

## Open questions
- Should `Fill` children be allowed a minimum (`Fill(w).min(n)`) so a map
  panel can refuse to shrink below a usable width on a small terminal, or is
  the fixed 155 × 45 frame enough to make that moot?
