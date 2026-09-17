# Labeled Bar

Back to the [component index](README.md).

A one-row horizontal gauge: a label, a bracketed bar, and a value. Shows
vitals and traits in [S03](../screens/s03-creature-inspector.md), resource
levels in [S01](../screens/s01-world-map.md) and [S06](../screens/s06-ecology.md),
and shares in [S04](../screens/s04-species-browser.md) and
[S09](../screens/s09-world-generation.md). The bare bar without label or value
is the same component and appears inside [Table](table.md) rows.

## Anatomy

```
<Label>     [<Bar>            ]   <Value> <Suffix>
```

Label occupies `label_w` cells. Bar occupies `bar_w` cells including its
brackets. Three blank cells follow, then Value right-aligned in 4 cells, then an
optional Suffix after one space.

## Slots
| Slot   | Required | Position                                        | Overflow                                            |
|--------|----------|-------------------------------------------------|-----------------------------------------------------|
| Label  | yes      | left, padded or cut to `label_w`                | cut at `label_w`, no marker                         |
| Bar    | yes      | after Label, `bar_w` cells                      | never cut; the row is not drawn if `bar_w` does not fit |
| Value  | no       | 3 blank cells after Bar, right-aligned in 4     | dropped before Bar shrinks                          |
| Suffix | no       | one space after Value, free text                | cut at the row's right edge                         |

## Sizing
Height 1. Width is `label_w + bar_w + 7` plus Suffix if any. Defaults:
`label_w` 12, `bar_w` 20 (the S01 sidebar sizes). Minimum `bar_w` is 3 (one
fill cell between brackets). Fill cells are `bar_w − 2`; filled count is
`round(value × fill cells)` with `value` clamped to `0..=1`.

## Variants
| Variant  | What changes                                                     | Used by                                |
|----------|------------------------------------------------------------------|----------------------------------------|
| Percent  | Value is `{:>3}%` of the 0..1 value (default)                    | S01 resources, S03 vitals, S06 totals  |
| Count    | Value is a supplied integer, right-aligned in 4; bar is the share of a maximum | S06 carcasses / dens / regrowth |
| Decimal  | Value is a supplied string such as `0.57`                        | S04 base genome, S06 biomass            |
| Suffix   | free text after Value                                            | S03 `848 / 858 days`, `8 times`         |
| Vital    | fill colour from the value: `> 0.6` good, `> 0.3` warn, else bad; `inverted` flips the scale for hunger and thirst | S03 vitals, S01e following |
| Bare     | Bar only, no Label or Value                                      | S04 habitat, S06 regions, S01 health   |
| Marker   | Bare bar with a `│` at a second value (the species base)         | S04 base genome vs current mean         |

## Interaction
None. Display only. The keys of the screen around it are in [keys.md](keys.md).

## Styling
- Label: `theme::text()`.
- Brackets and empty cells: `theme::DIM` on `theme::PANEL_BG`.
- Filled cells: the caller's colour, or the Vital rule (`theme::GOOD`,
  `theme::WARN`, `theme::BAD`).
- Value and Suffix: `theme::text()`.
- Marker: `theme::TEXT_BRIGHT`.

## Glyphs
`glyphs::BAR_L` `[`, `glyphs::BAR_R` `]`, `glyphs::BAR_FILL` `█`,
`glyphs::BAR_EMPTY` `░`, `glyphs::V_LINE` `│` for Marker.

## Composition
*Contains:* nothing.
*Contained by:* [Panel](panel.md) bodies, [Table](table.md) cells (Bare),
[Modal](modal.md) bodies, [Stepper](stepper.md) rows in S09.

## API
### Today
```rust
widgets::bars::labeled(buf, area, row, label, value, color, label_w, bar_w)
widgets::bars::bar(buf, x, y, w, value, color)          // Bare
widgets::bars::vital_color(value, inverted) -> Color    // Vital colour rule
widgets::util::pct(value) -> String                     // " 63%"
```

### Planned
```rust
LabeledBar::new("vegetation", 0.63)
    .color(theme::VEGETATION)
    .label_w(12).bar_w(20)                   // defaults shown
    .render(buf, row_area)

LabeledBar::new("carcasses", 8.0 / 60.0).value_text("8")       // Count
LabeledBar::new("biomass", 0.57).value_text("0.57")            // Decimal
LabeledBar::new("age", 0.99).suffix("848 / 858 days")          // Suffix
LabeledBar::vital("hunger", 0.41, Inverted::Yes)               // Vital

Bar::new(0.70).color(c).render(buf, cell_area)                 // Bare
Bar::new(0.82).marker(0.80).render(buf, cell_area)             // Marker
```
`height` is 1; `min_width` is `label_w + bar_w + 7` for LabeledBar and
`bar_w` for Bar.

## Gaps today
- Count, Decimal and Suffix are hand-rolled per screen; `bars::labeled` only
  does Percent.
- Marker is drawn by hand in `s04_species/table.rs`.
- `bars::bar` takes `x, y, w` instead of a `Rect`.

## Examples

### Percent (43 columns)
Value 0.63, `label_w` 12, `bar_w` 20: 11 of 18 cells filled.
```
║ vegetation [███████████░░░░░░░]    63%  ║
```

### Vital, inverted (43 columns)
Hunger 0.41, `label_w` 9, `bar_w` 24. Inverted, so the fill is `theme::WARN`.
```
║ hunger  [█████████░░░░░░░░░░░░░]    41% ║
```

### Count (43 columns)
8 carcasses against a maximum of 60.
```
║ carcasses  [███░░░░░░░░░░░░░░░]      8  ║
```

### Suffix (43 columns)
Age 0.99, `label_w` 5, `bar_w` 14.
```
║ age [████████████]    99% 848 / 858 days║
```

### Bare, inside a table row (43 columns)
Share 0.70 of the largest region, `bar_w` 14.
```
║ Sunfall Coast    [████████░░░░]   28    ║
```

### Marker (43 columns)
Current mean 0.82 with the base value 0.80 marked, `bar_w` 14.
```
║ Speed  0.80  0.82 [█████████│░░]  +0.02 ║
```

## Open questions
- Should Value width grow past 4 for counts above 9999, or should callers
  format thousands themselves?
- Vital thresholds (0.6 / 0.3) are shared with the map health overlay; should
  they move to `theme` as named constants?
