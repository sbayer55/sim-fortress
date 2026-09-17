# VStack

Back to the [component index](README.md).

A vertical stack: lays its children top to bottom, each as tall as it asks to
be, and reports the total. It replaces the hand-threaded `row` counters that
every panel body uses today. The S01 sidebar, every inspector column in
[S03](../screens/s03-creature-inspector.md), every [Modal](modal.md) body and
every [Scroll Region](scroll-region.md) body is a VStack.

## Anatomy

```
<Child 1>
<Child 2>
<Child 3>
```

Each child takes the full width and starts on the row after the previous
child ends, plus `gap` blank rows.

## Slots
| Slot     | Required | Position                                            | Overflow                                                      |
|----------|----------|-----------------------------------------------------|---------------------------------------------------------------|
| Children | yes, 1+  | top to bottom in order; each gets `(width, h)` where `h` is its constraint | children past the bottom edge are not drawn; a child that straddles it is clipped by its own area |

## Sizing
Width is the area width and every child gets all of it. Height is the sum of
the children's heights plus the gaps, so `Component::height(width)` is
computable before drawing and a Scroll Region can use it.

Each child has a constraint:

| Constraint | Height given to the child                                                     |
|------------|-------------------------------------------------------------------------------|
| `Auto`     | `child.height(width)` (default; row components return 1)                      |
| `Fixed(n)` | exactly `n` rows                                                              |
| `Fill(w)`  | a share of whatever is left after `Auto` and `Fixed` children, by weight `w`; the remainder goes to the last `Fill` child |

`gap` defaults to 0. `min_width` is the largest `min_width` among the children.
A stack with a `Fill` child fills the area; without one it is as tall as its
content and leaves the rest of the area untouched.

## Variants
| Variant  | What changes                                                 | Used by                               |
|----------|--------------------------------------------------------------|---------------------------------------|
| Content  | all children `Auto`; height is the content height           | sidebars, inspector columns, modal bodies |
| Filled   | one `Fill` child takes the rest (a Table, Chart or map)      | S04 table over detail, S05 charts     |
| Gapped   | `gap` blank rows between children                            | S09 form groups                       |

## Interaction
None. Display only. Keys reach the children through the screen; see
[keys.md](keys.md).

## Styling
None of its own. A VStack paints nothing; the background is whatever the
[Panel](panel.md) or [Modal](modal.md) around it filled. Use
[Spacer](spacer.md) for deliberate blank rows.

## Glyphs
None.

## Composition
*Contains:* any component, including [HStack](hstack.md), another VStack,
[Spacer](spacer.md), and a [Columns](columns.md) block for rows that must
align.
*Contained by:* [Panel](panel.md), [Modal](modal.md), [Scroll Region](scroll-region.md),
[HStack](hstack.md) cells.

## API
### Today
No helper. Screens keep a `row: u16`, call a helper with `(area, row)`, and
add the rows used:
```rust
// src/ui/screens/s01_map/base.rs
pub(super) fn clock_section(f, inner, mut row, app, time) -> u16   // returns the next free row
```
About 600 lines across `src/ui/screens` follow this pattern.

### Planned
```rust
VStack::new()
    .child(Divider::new("Clock"))                     // Auto
    .child(Text::new("Year 12  Day 4     ♫ Autumn"))
    .child(Spacer::rows(1))
    .child(Divider::new("Resources"))
    .child(LabeledBar::new("vegetation", 0.44).color(theme::VEGETATION))
    .child_with(Fill(1), table)                       // takes the rest
    .gap(0)                                           // default
    .render(buf, area)
```
`height(width)` is the content height: the `Auto` and `Fixed` children plus
gaps. A `Fill` child counts 0 there and takes what the area leaves at render
time. Children are `&dyn Component`; `VStack::from_boxes(&rows)` takes a
`Vec<Box<dyn Component>>` for sections that return their rows.

## Gaps today
- Nothing shared; every screen threads its own row counter.
- Section helpers return the next row instead of a height, so a caller cannot
  ask how tall a section is without drawing it.

## Examples

### Content, the S01 sidebar head (43 columns)
Children: Divider, Text, Text, Spacer(1), Divider, Labeled Bar. Height 6 in an
8-row panel.
```
╔ Status ═════════════════════════════════╗
║─ Clock ─────────────────────────────────║
║ Year 12  Day 4     ♫ Autumn             ║
║ 14:00  ☼ day      ►► x2  running        ║
║                                         ║
║─ Resources ─────────────────────────────║
║ vegetation [████████░░░░░░░░░░]    44%  ║
╚═════════════════════════════════════════╝
```

### Clipped, the same stack in a 3-row area (43 columns)
The last three children are not drawn. A [Scroll Region](scroll-region.md)
around the stack would add `↓3` to the bottom edge instead.
```
╔ Status ═════════════════════════════════╗
║─ Clock ─────────────────────────────────║
║ Year 12  Day 4     ♫ Autumn             ║
║ 14:00  ☼ day      ►► x2  running        ║
╚═════════════════════════════════════════╝
```

## Open questions
- Should a child that does not fit at all be skipped so the next shorter one
  can be tried, or is strict order always right? Strict order is assumed.
