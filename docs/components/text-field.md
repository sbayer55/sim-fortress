# Text Field

Back to the [component index](README.md).

A one-row free-text field: a label, the text in a bracketed box, and a dim
hint. The world name and seed on [S09](../screens/s09-world-generation.md)
are the only Text Fields today. It shares its geometry with
[Stepper](stepper.md); the brackets replace the arrows.

## Anatomy

```
 <Label>            [ <Text>                  ] <Hint>
```

One blank cell, Label padded to `label_w`, then the box of `box_w` cells:
`[`, one blank, Text padded to `box_w − 3`, `]`. One blank, then Hint to the
right edge. Shown at the S09 defaults, `label_w` 20 and `box_w` 26.

## Slots
| Slot  | Required | Position                                             | Overflow                                                    |
|-------|----------|------------------------------------------------------|-------------------------------------------------------------|
| Label | yes      | from column 1, padded or cut to `label_w`            | cut at `label_w`, no marker                                 |
| Text  | yes      | inside the box after `[` and one blank, left-aligned | cut at `box_w − 3`; the screen caps input so this never happens |
| Hint  | no       | one blank after `]`, free text                       | cut at the row's right edge; dropped before the box shrinks |

## Sizing
Height 1. Width is `label_w + box_w + 2` plus Hint. Defaults: `label_w` 20,
`box_w` 26. Minimum `box_w` is 4. S09 caps the name at 23 characters, which
is exactly the 23-cell text area, and the seed at 20. Never writes outside
its area.

## Variants
| Variant  | What changes                                                                   | Used by        |
|----------|--------------------------------------------------------------------------------|----------------|
| Plain    | Label `theme::text()`, brackets dim, box `theme::TEXT_BRIGHT` on `theme::BG`   | unfocused S09 name and seed |
| Focused  | Label bold `theme::label()`, brackets `theme::KEY` bold, box `theme::selected()` | the focused S09 field |

## Interaction
Standard keys are in [keys.md](keys.md); the table below is what the live code does,
and its deviations from the standard are collected there.

There is no cursor and no insertion point; editing is type-to-replace at the
end of the string:

| Key               | Effect                                                                      |
|-------------------|-----------------------------------------------------------------------------|
| `Tab` / `BackTab` | move focus; wraps; arms type-to-replace for the next field                  |
| any printable     | first key after focusing clears the field, then appends; later keys append; ignored at the cap |
| `Backspace`       | delete the last character; also disarms type-to-replace                     |
| `←` / `→`         | nothing (no cursor movement)                                                |
| `Enter`, `Esc`, `q` | nothing while a text field is focused; `Esc` still leaves the screen via the screen handler |

Typing in the seed field marks the preview dirty; an unparseable seed shows
`invalid seed` in the [Status Bar](status-bar.md) and `Generate` refuses.
`[ Randomize seed ]` writes a decimal seed into the field.

## Styling
- Label: `theme::text()`; focused `theme::label()` plus bold.
- Brackets: `theme::dim_text()`; focused `theme::KEY` bold on
  `theme::PANEL_BG`.
- Box (blank, Text and padding): `theme::TEXT_BRIGHT` on `theme::BG`;
  focused `theme::selected()`.
- Hint: `theme::dim_text()`.
- No cursor cell is drawn; `theme::CURSOR_BG` is unused here.

## Glyphs
None from `glyphs::`. `[` and `]` are plain ASCII.

## Composition
*Contains:* nothing.
*Contained by:* [Panel](panel.md) bodies, [Modal](modal.md) bodies; sits in
the same column as [Stepper](stepper.md) rows on S09.

## API
### Today
```rust
ui::screens::s09_worldgen::draw_field(f, area, row, label, value, adjustable: false, hint, focused: bool)
ui::screens::s09_worldgen::handle_text_field(form, focus, code) -> Action
```
The `('[', ']')` branch of `draw_field`. `handle_text_field` owns the
type-to-replace flag (`WorldGenForm::text_edited`) and the length caps.

### Planned
```rust
TextField::new("World name", "The Valley of Sunfall")
    .hint("text")
    .focused(true)                       // caller-owned state
    .label_w(20).box_w(26)               // defaults shown
    .render(buf, row_area)
```
`height` is 1; `min_width` is `label_w + box_w + 2`. Editing stays with the
screen; the component draws the current string.

## Gaps today
- No cursor: the field cannot show where the next character goes, and there
  is no way to edit the middle of the string.
- Type-to-replace is invisible. Nothing marks that the next key will wipe
  the field.
- The box uses `theme::BG` inside a `theme::PANEL_BG` panel, shared with
  [Stepper](stepper.md).
- `label_w` and `box_w` are constants in `draw_field`.
- `←` `→` do nothing; the standard moves a cursor. See [keys.md](keys.md).

## Examples

### Plain, the S09 form (66 columns)
Copied from [S09a](../screens/renders/S09a.txt).
```
║ World name          [ The Valley of Sunfall  ] text            ║
║ Seed                [ 0xC0FFEE               ] hex/decimal     ║
```

### Focused (66 columns)
Cell for cell the same as Plain; only Label, brackets and box change colour.
```
║ Seed                [ 0xC0FFEE               ] hex/decimal     ║
```

### After the first key (66 columns)
The name field was focused and `T` was typed: the old text is gone.
```
║ World name          [ T                      ] text            ║
```

### Narrow, `label_w` 8 and `box_w` 14 (43 columns)
```
║ Seed    [ 0xC0FFEE   ] hex/decimal      ║
```

## Open questions
- Should the component draw a cursor cell (`theme::CURSOR_FG` on
  `theme::CURSOR_BG`) after the text when focused, so typing has a visible
  insertion point? The screen would need to own a cursor index for that.
- Should type-to-replace be shown, for instance by drawing the text in
  `theme::dim_text()` until the first key, or dropped in favour of a plain
  append?
