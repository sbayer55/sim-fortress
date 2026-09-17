# Panel

Back to the [component index](README.md).

The bordered box every screen is built from: the map, the status sidebar, the
three inspector columns, every modal. Draws a border and a title, clears the
area, and hands back the inner rectangle for content. Used by every screen under
[../screens](../screens/README.md).

## Anatomy

```
╔ <Title> ════════════════════════ <Info> ╗
║ <Content>                               ║
╚════════════════════════════════ <Foot> ═╝
```

## Slots
| Slot    | Required | Position                                   | Overflow                                              |
|---------|----------|--------------------------------------------|-------------------------------------------------------|
| Title   | no       | top edge, starts at column 1 as ` Title `  | cut at the available width; an empty title draws a continuous border |
| Info    | no       | top edge, right-aligned as ` Info `        | dropped entirely before Title is cut                  |
| Content | yes      | inner area, `(w−2) × (h−2)`                | clipped by the border; see Scroll Region for overflow |
| Foot    | no       | bottom edge, right-aligned, one cell of border kept after it | dropped when the bottom edge is narrower than the text plus 2 |

Foot is not set by callers directly. [Scroll Region](scroll-region.md) writes
`↑n ↓m` there when the content is taller than the panel.

## Sizing
Fills the area it is given. Minimum useful size is 3 × 3; below that the border
is drawn as far as it fits and the inner area is empty. Inner area is
`Rect { x+1, y+1, w−2, h−2 }`. No parameters other than the area.

## Variants
| Variant  | What changes                                  | Used by                         |
|----------|-----------------------------------------------|---------------------------------|
| Outer    | double border `═ ║`, `theme::border()`        | top-level panels on every screen |
| Focus    | double border, `theme::border_focus()`        | the focused panel in S03/S04/S06, every modal |
| Inner    | single border `─ │`, `theme::border()`        | nested boxes: S03 Surroundings, S09 preview |
| Untitled | no Title, continuous top edge                 | `confirm` dialog                |
| With Info| dim right-aligned text in the top edge        | S01 map scroll hint, S04 counts, S09 field counter |
| Scrolled | Foot shows `↑n ↓m`                            | any panel drawn through Scroll Region |

## Interaction
None. Display only. The keys of the screen around it are in [keys.md](keys.md).

## Styling
- Whole area filled with `theme::PANEL_BG` before the border is drawn, so a
  panel always clears what was under it.
- Border: `theme::border()` for Outer and Inner, `theme::border_focus()` for
  Focus.
- Title: `theme::title()` (bold, `theme::TITLE`). The padding spaces either side
  take the border style so the rule appears to pass behind the text.
- Info and Foot: `theme::dim_text()`.

## Glyphs
Borders come from ratatui `BorderType::Double` (`╔ ╗ ╚ ╝ ═ ║`) and
`BorderType::Plain` (`┌ ┐ └ ┘ ─ │`). Foot uses `glyphs::UP` `↑` and
`glyphs::DOWN` `↓`. All CP437.

## Composition
*Contains:* anything. Content is a fresh inner `Rect`; the usual body is a
[VStack](vstack.md) of [Divider](divider.md) sections and row components, or
a [Table](table.md), [Chart](chart.md), or map. Panels sit side by side in an
[HStack](hstack.md).
*Contained by:* screen layouts; [Modal](modal.md) (a centred Focus panel);
another Panel (Inner inside Outer).

## API
### Today
```rust
widgets::panel::draw(f, area, title, kind) -> Rect
widgets::panel::draw_in(buf, area, title, kind) -> Rect
widgets::panel::draw_with_hint(f, area, title, hint, kind) -> Rect
widgets::panel::draw_with_hint_in(buf, area, title, hint, kind) -> Rect
widgets::panel::Kind::{Outer, Inner, Focus}
```
`hint` is the Info slot. Foot is written separately by `widgets::scroll::draw`.

### Planned
```rust
Panel::new("Status")                 // Title; Panel::untitled() for none
    .kind(Kind::Outer)               // default Outer
    .info("Esc closes")              // optional
    .foot("↑3 ↓12")                  // optional, normally set by ScrollRegion
    .render(buf, area) -> Rect       // returns the inner area
```
`Component::height` returns the area height; `min_width` is 3.

## Gaps today
- Title and Info are both ratatui block titles. When they collide the
  right-aligned Info overwrites the Title instead of being dropped.
- Frame and Buffer entry points are duplicated (`draw` / `draw_in`). Planned
  API is buffer-only.
- Foot is only reachable through Scroll Region.

## Examples

### Simple (43 columns)
```
╔ Greeting ═══════════════════════════════╗
║ Hello!                                  ║
╚═════════════════════════════════════════╝
```

### With Info (43 columns)
```
╔ Status ═════════════════════ Esc closes ╗
║ Year 12  Day 4     ♫ Autumn             ║
╚═════════════════════════════════════════╝
```

### Focus (43 columns)
Cell for cell identical to Outer; only the border colour changes to
`theme::BORDER_FOCUS`.
```
╔ Simulation Controls ════════ Esc closes ╗
║ state   ► RUNNING      ►► x2            ║
╚═════════════════════════════════════════╝
```

### Inner (43 columns)
```
┌ Surroundings ───────────────────────────┐
│ tile contents                           │
└─────────────────────────────────────────┘
```

### Nested, Inner inside Outer (43 columns)
```
╔ Life ═══════════════════════════════════╗
║ ┌ Surroundings ───────────────────────┐ ║
║ │~~~~~♠♣"""""""***,,*,♣♣♠♠♠♠""""""""**│ ║
║ └─────────────────────────────────────┘ ║
╚═════════════════════════════════════════╝
```

### Scrolled, Foot set by Scroll Region (43 columns)
```
╔ Legend ═════════════════════════════════╗
║ ≈ deep water       ~ shallow water      ║
║ · sand             . bare dirt          ║
╚═════════════════════════════════ ↑3 ↓12 ╝
```

### Divider inside a panel (43 columns)
See [divider.md](divider.md) for the row itself.
```
╔ Greeting ═══════════════════════════════╗
║ Hello!                                  ║
║─ Metadata ──────────────────────────────║
║ Sent Sep 9th, 2026                      ║
╚═════════════════════════════════════════╝
```

## Open questions
- Should Focus also change the title colour, so focus is visible on terminals
  where the two border colours are hard to tell apart?
- Should the box-drawing glyphs get `glyphs::` constants so the CP437 test
  covers them? Raised in the [README](README.md#open-questions).
