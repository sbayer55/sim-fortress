# Stepper

Back to the [component index](README.md).

A one-row numeric or choice field adjusted with `←` / `→`: a label, a boxed
value between `◄` and `►` arrows, and a dim hint. Every adjustable field on
the [S09](../screens/s09-world-generation.md) form is a Stepper, the species
count column in the same form is the compact variant, and the autosave row on
[S10](../screens/s10-simulation-controls.md) is the inline variant.

## Anatomy

```
 <Label>            ◄ <Value>                 ► <Hint>
```

One blank cell, Label padded to `label_w`, then the value box of `box_w`
cells: `◄`, one blank, Value padded to `box_w − 3`, `►`. One blank, then Hint
to the right edge. Shown at the S09 defaults, `label_w` 20 and `box_w` 26.

## Slots
| Slot  | Required | Position                                             | Overflow                                                  |
|-------|----------|------------------------------------------------------|-----------------------------------------------------------|
| Label | yes      | from column 1, padded or cut to `label_w`            | cut at `label_w`, no marker                               |
| Value | yes      | inside the box after `◄` and one blank, left-aligned | cut at `box_w − 3`; the arrows are never moved            |
| Hint  | no       | one blank after `►`, free text                       | cut at the row's right edge; dropped before the box shrinks |

## Sizing
Height 1. Width is `label_w + box_w + 2` plus Hint. Defaults: `label_w` 20,
`box_w` 26 (the S09 form, 66 columns wide). Minimum `box_w` is 4 (arrows,
blank, one value cell). Never writes outside its area: Hint is cut at
`width − label_w − box_w − 2`.

## Variants
| Variant     | What changes                                                                 | Used by                    |
|-------------|------------------------------------------------------------------------------|----------------------------|
| Numeric     | Value is a number; `Space` opens typed entry                                 | S09 map size, percentages, rates |
| Choice      | Value is one of a fixed list of names; no typed entry                        | S09 rainfall, predation difficulty |
| Typing      | Value is the typed buffer followed by `_`                                    | S09 while a numeric field is being typed |
| Focused     | Label bold `theme::label()`, arrows `theme::KEY` bold, Value `theme::selected()` | the focused S09 field  |
| Compact     | no Label or Hint; `◄`, Value right-aligned in 5 plus one blank, `►`; the row around it belongs to a table | S09 species counts |
| Inline      | arrows bracket label and value together: `◄ autosave every 7 days   ►`, followed by `[←→]` | S10 autosave row |
| Non-adjustable | `[` `]` instead of the arrows; this is a [Text Field](text-field.md)      | S09 name and seed          |

## Interaction
Standard keys are in [keys.md](keys.md); the table below is what the live code does,
and its deviations from the standard are collected there.

| Key              | Effect                                                              |
|------------------|---------------------------------------------------------------------|
| `Tab` / `BackTab`| move focus to the next / previous field; wraps                      |
| `←` / `→`        | step the value down / up by the field's step, clamped to its range  |
| `Space`          | numeric fields only: open typed entry with an empty buffer          |
| `0-9`, `.`       | while typing: append, up to 9 characters                            |
| `Backspace`      | while typing: delete the last character                             |
| `Enter`          | while typing: apply; otherwise generate the world                   |
| `Tab`            | while typing: apply, then move focus                                |
| `Esc`            | while typing: cancel; otherwise back to the title screen            |
| `q`              | back to the title screen (only when no text field is focused)       |

Steps and ranges: width ±10 in 100..=1000, height ±5 in 30..=1000, water,
forest, rock and age ±1, season length ±10 in 30..=180, species counts ±10
in 0..=999, mutation rate and strength ±0.01, regrowth ±0.1. Rainfall and
difficulty cycle through three names and wrap. On S10 `←` / `→` step the
autosave interval by one day in 0..=365 with no focus at all.

## Styling
- Label: `theme::text()`; focused `theme::label()` plus bold.
- Arrows: `theme::dim_text()`; focused `theme::KEY` bold on
  `theme::PANEL_BG`. Inline arrows are always `theme::key()`.
- Value box (the blank, Value and its padding): `theme::TEXT_BRIGHT` on
  `theme::BG`, a darker well than the panel; focused `theme::selected()`.
- Hint: `theme::dim_text()`.
- Compact: the whole table row is filled with `theme::SELECT_BG` when
  focused and the value takes `theme::selected()`; unfocused arrows are
  `theme::DIM`.
- Nothing else is background-filled; the row shows `theme::PANEL_BG`.

## Glyphs
`glyphs::REWIND` `◄` and `glyphs::PLAY` `►`. The typing caret `_` is plain
ASCII. The Inline variant's `[←→]` hint uses `glyphs::LEFT` `←` and
`glyphs::RIGHT` `→`; its brackets are plain ASCII.

## Composition
*Contains:* [Key Hint](key-hint.md) (the `[←→]` of the Inline variant).
*Contained by:* [Panel](panel.md) bodies, [Modal](modal.md) bodies,
[Table](table.md) rows (Compact, next to a [Labeled Bar](labeled-bar.md)).

## API
### Today
```rust
ui::screens::s09_worldgen::draw_field(f, area, row, label, value, adjustable: bool, hint, focused: bool)
ui::screens::s09_worldgen::shown(form, focus, value) -> String     // Typing: "{buf}_"
ui::screens::s09_worldgen::draw_species_section(f, inner, form, row) // Compact
ui::screens::s10_controls::options_panel(f, inner, row, app)        // Inline, last row
```
`draw_field` with `adjustable == true` is the Stepper; with `false` it is the
Text Field. Label width 20 and box width 26 are hard-coded.

### Planned
```rust
Stepper::new("Water %", "20")
    .hint("lakes + rivers")
    .focused(true)                       // caller-owned state
    .typing(Some("15"))                  // Typing variant; draws "15_"
    .label_w(20).box_w(26)               // defaults shown
    .render(buf, row_area)

Stepper::compact("240").value_w(5).focused(false)      // Compact
Stepper::inline("autosave every 7 days").key("[←→]")   // Inline; `.text_w(24)` pads the text inside the arrows
```
`height` is 1; `min_width` is `label_w + box_w + 2`. The value is a
pre-formatted string; ranges, steps and typed-entry parsing stay with the
screen.

## Gaps today
- The value well uses `theme::BG`, the only place a row component paints the
  screen background inside a panel. Keep it or move it to `theme::PANEL_BG`
  (see Open questions).
- Compact and Inline are separate hand-drawn rows with their own spacing;
  `draw_field` only does the full row.
- `label_w` and `box_w` are constants in `draw_field`.
- The prototype [S09a](../screens/renders/S09a.txt) shows one
  `Size  ◄ 150 x 40  ►` row; the live form has separate `Map width` and
  `Map height` rows. The sheet follows the live form.
- Keys: `Space` opens typed entry and `Enter` generates the world from any
  field. The standard has `Space` select and `Enter` act on the focused field
  only, with typing starting on the first digit key. See [keys.md](keys.md).

## Examples

### Numeric, the S09 form (66 columns)
Copied from [S09a](../screens/renders/S09a.txt).
```
║ Water %             ◄ 20                     ► lakes + rivers  ║
║ Season length       ◄ 90 days                ► 30 - 180 days   ║
```

### Choice (66 columns)
```
║ Rainfall            ◄ normal                 ► dry/normal/wet  ║
```

### Focused (66 columns)
Cell for cell the same as Numeric; Label, arrows and Value change colour.
The hint fills the row to the border exactly.
```
║ Map width           ◄ 150                    ► 100 - 1000 cells║
```

### Typing (66 columns)
`Space` opened the field, `1` and `5` were typed.
```
║ Water %             ◄ 15_                    ► lakes + rivers  ║
```

### Compact, inside the species table (66 columns)
Copied from S09a. The bar is a [Labeled Bar](labeled-bar.md) (Bare).
```
║ V Voles     prey     ◄  240 ►   [████████░░░░░░░░░░░░]  42%    ║
║ F Foxes     predator ◄   30 ►   [█░░░░░░░░░░░░░░░░░░░]   5%    ║
```

### Inline, the S10 autosave row (60 columns)
```
║  ◄ autosave every 7 days   ► [←→]                        ║
║  ◄ autosave every off      ► [←→]                        ║
```

### Narrow, `label_w` 8 and `box_w` 14 (43 columns)
```
║ Water % ◄ 20         ► lakes + rivers   ║
```

## Open questions
- Should the value well stay `theme::BG` (a visible sunken box) or use
  `theme::PANEL_BG` like every other row? The well is the only cue that a
  field is editable when it is not focused.
- The Compact variant right-aligns the value and the full row left-aligns
  it. One rule, or is the difference worth keeping for the table column?
- Should typed entry show a real cursor cell (`theme::CURSOR_BG`) instead of
  the `_` character?
