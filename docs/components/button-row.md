# Button Row

Back to the [component index](README.md).

A one-row set of bracketed buttons, one of which has focus. The action row
at the foot of the [S09](../screens/s09-world-generation.md) form, the
choices under an [S12](../screens/s12-alert-modal.md) alert, and the
`[ Yes ]  [ No ]` of the confirm dialog are Button Rows.

## Anatomy

```
            [ <Button> ]    [ <Button> ]
```

Buttons are `[ text ]`, separated by `gap` blank cells and centred as a
group in the row. Shown at the S12 default, `gap` 4.

## Slots
| Slot   | Required | Position                                                  | Overflow                                                            |
|--------|----------|-----------------------------------------------------------|---------------------------------------------------------------------|
| Button | yes, one or more | left to right in the order given, `gap` cells apart; the group is centred, or starts at `indent` when left-aligned | the row is cut at its right edge; buttons are never shortened |

## Sizing
Height 1. Width is the area width; the group needs
`sum(button widths) + gap × (n − 1)` cells, which is `min_width`. `gap`
defaults to 4; `indent` (Left variant only) defaults to 4. Never writes
outside its area.

## Variants
| Variant  | What changes                                                                    | Used by                       |
|----------|---------------------------------------------------------------------------------|-------------------------------|
| Centred  | group centred in the row; focused button `theme::selected()`, others `theme::text()` | S12 extinction and epidemic alerts |
| Left     | group starts at `indent`; `gap` 3                                               | S09 form foot                 |
| Primary  | one button keeps a filled background when unfocused (`theme::WARN`) so it reads as the default action | S09 `[ Generate ]` |
| Trailing hint | dim key hint after the last button on the same row                        | confirm `←→ move  Enter select  Esc no` |

## Interaction
Standard keys are in [keys.md](keys.md); the table below is what the live code does,
and its deviations from the standard are collected there.

| Key               | S09 form                                  | S12 alert                | confirm                         |
|-------------------|-------------------------------------------|--------------------------|---------------------------------|
| `←` / `→`         | not handled on a button                   | move focus; wraps        | swap focus                      |
| `Tab` / `BackTab` | move focus through every form field, buttons last | not handled      | swap focus                      |
| `Enter`           | activate the focused button               | activate                 | activate                        |
| `Esc`             | back to the title screen                  | continue (same as `[ Continue ]`) | no                    |
| letters           | `q` back (when no text field is focused)  | `l` lineage, `Space` pause | `y` yes, `n` no               |

## Styling
- Focused button: `theme::selected()` (`theme::TEXT_BRIGHT` bold on
  `theme::SELECT_BG`), brackets included.
- Other buttons: `theme::text()`.
- Primary, unfocused: `theme::CURSOR_FG` bold on `theme::WARN`; focused it
  takes `theme::selected()` like any other button.
- Gaps and padding: `theme::text()` on `theme::PANEL_BG`; nothing else is
  background-filled.
- Trailing hint: `theme::dim_text()`.

## Glyphs
None from `glyphs::`. `[` and `]` are plain ASCII. The confirm hint's
`←→` are `glyphs::LEFT` and `glyphs::RIGHT`.

## Composition
*Contains:* [Key Hint](key-hint.md) (Trailing hint).
*Contained by:* [Modal](modal.md) bodies (S12, confirm), the last inner row
of a [Panel](panel.md) (S09).

## API
### Today
```rust
ui::screens::s09_worldgen::draw_form_buttons(f, inner, form)        // Left + Primary
ui::screens::s12_alert::AlertModal::render(..)                       // Centred, `BUTTONS`
ui::screens::s12_alert::draw_epidemic_buttons(f, inner, row, focus)  // Centred, `EPIDEMIC_BUTTONS`
ui::screens::confirm::ConfirmModal::render(..)                       // Left, indent 2, Trailing hint
```
`BUTTONS` is `["[ Continue ]", "[ View lineage ]", "[ Pause ]"]`; the
epidemic set swaps the middle one for `[ Show outbreak ]`.

### Planned
```rust
ButtonRow::new(&["[ Continue ]", "[ View lineage ]", "[ Pause ]"])
    .focused(0)                          // caller-owned state
    .gap(4)                              // default shown
    .render(buf, row_area)

ButtonRow::new(&["[ Generate ]", "[ Randomize seed ]", "[ Back ]"])
    .focused(9).primary(0).left(4).gap(3)                  // Left + Primary
ButtonRow::new(&["[ Yes ]", "[ No ]"]).focused(0).left(2)
    .hint("←→ move  Enter select  Esc no")                 // Trailing hint
```
`height` is 1; `min_width` as in Sizing. `focused` is an index into the
button list; the screen maps it to its own focus state.

## Gaps today
- S12 counts a trailing `gap` after the last button when centring, so the
  group sits two cells left of centre in the 62-cell inner width. The
  prototype render is centred correctly; the spec follows the render.
- S09 and confirm draw the focused button as `theme::CURSOR_FG` bold on
  `theme::ACCENT`, and S09's unfocused buttons as `theme::TEXT_BRIGHT` on
  `theme::HEADER_BG`. The spec uses `theme::selected()` and `theme::text()`
  like S12.
- Three copies of the layout loop with three different gaps (3, 4, 4) and
  indents (4, centred, 2).
- Every button text carries its own `[ ` and ` ]`; the planned builder could
  add them.
- S12 dismisses on `Space` instead of activating the focused button; see
  [keys.md](keys.md).

## Examples

### Centred, the S12 extinction alert (64 columns)
Copied from [S12a](../screens/renders/S12a.txt); `[ Continue ]` is focused.
```
║        [ Continue ]    [ View lineage ]    [ Pause ]         ║
```

### Centred, the confirm dialog (43 columns)
`[ Yes ]` is focused. The confirm dialog is 50 columns wide; this is the
same row in the canonical width.
```
║            [ Yes ]    [ No ]            ║
```

### Left with Primary, the S09 form foot (66 columns)
Copied from [S09a](../screens/renders/S09a.txt). `[ Generate ]` is the
Primary; focus is on a form field above, so no button is selected.
```
║    [ Generate ]   [ Randomize seed ]   [ Back ]                ║
```

## Open questions
- Should Primary exist at all once focus is always visible, or is the filled
  `theme::WARN` button worth keeping as the "Enter does this" cue on S09,
  where `Enter` generates from any field?
- Should the confirm dialog's trailing hint move to the
  [Status Bar](status-bar.md), leaving the row to the buttons?
