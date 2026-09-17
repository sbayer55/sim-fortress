# Divider

Back to the [component index](README.md).

A one-row horizontal rule with an optional label, used to split a panel body
into named sections: `─ Clock ───`. Every sidebar section in
[S01](../screens/s01-world-map.md), every inspector section in
[S03](../screens/s03-creature-inspector.md), and the sub-chart titles in
[S05](../screens/s05-population-charts.md) are Dividers.

## Anatomy

```
─ <Text> ────────────────────────────────
```

## Slots
| Slot | Required | Position                                  | Overflow                                                        |
|------|----------|-------------------------------------------|-----------------------------------------------------------------|
| Text | no       | starts at column 1, drawn as ` Text `     | cut at `width − 2` so the first and last cell stay rule glyphs  |

## Sizing
Height 1. Width is the area width; the rule always spans it. Minimum width 3
(one rule cell, one text cell, one rule cell). No other parameters.

## Variants
| Variant   | What changes                                                          | Used by                          |
|-----------|-----------------------------------------------------------------------|----------------------------------|
| In-panel  | spans the panel's inner width, so the panel's `║` columns bracket it and it reads as `║─ Text ──║` | S01 sidebar, S03, S06, S10 |
| Bare      | any width inside a wider row, not touching a border                   | S04 species detail right column  |
| No text   | rule only                                                             | S03 timeline spacer, S11 columns |

The in-panel look is not a separate drawing path. The rule starts at the first
inner cell, so the `─` lands directly against the `║`. That is why the user-
facing shape is `║─ Clock ──║` and not `║ ─ Clock ──║`.

## Interaction
None. Display only. The keys of the screen around it are in [keys.md](keys.md).

## Styling
- Rule: `theme::border()`.
- Text and its padding spaces: `theme::label()` (`theme::ACCENT` on
  `theme::PANEL_BG`).

## Glyphs
`glyphs::H_LINE` `─`.

## Composition
*Contains:* nothing.
*Contained by:* [Panel](panel.md) bodies, [Modal](modal.md) bodies,
[Scroll Region](scroll-region.md) stacks, any row of a wider layout (Bare).

## API
### Today
```rust
widgets::panel::section(f, area, row, title)
widgets::panel::section_in(buf, area, row, title)
```
`area` is the panel inner area and `row` the row offset inside it.

### Planned
```rust
Divider::new("Clock").render(buf, row_area)   // row_area is 1 row high
Divider::rule().render(buf, row_area)         // No text variant
```
`height` is 1; `min_width` is 3.

## Gaps today
- `panel::section` / `section_in` remain as thin wrappers until every screen
  has moved.

## Examples

### In-panel (43 columns)
```
║─ Clock ─────────────────────────────────║
```

### In a panel body (43 columns)
```
╔ Greeting ═══════════════════════════════╗
║ Hello!                                  ║
║─ Metadata ──────────────────────────────║
║ Sent Sep 9th, 2026                      ║
╚═════════════════════════════════════════╝
```

### No text (43 columns)
```
║─────────────────────────────────────────║
```

### Bare, 41 columns wide
```
─ Population, last 240 days ─────────────
```

### Text cut at width − 2 (43 columns)
Text is `Offspring forecast (with an average mate) tonight`.
```
║─ Offspring forecast (with an average ma─║
```

## Open questions
- The cut leaves no marker. Should long text end with a single `~` or be cut
  silently as today?
