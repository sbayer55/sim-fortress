# Checkbox

Back to the [component index](README.md).

A one-row on/off toggle: a `[x]` or `[ ]` mark, a label, and the key that
flips it. The Options section of [S10](../screens/s10-simulation-controls.md)
is a stack of five Checkboxes. Each row is driven by its own letter; there is
no focus and no cursor.

## Anatomy

```
 [<Mark>] <Label>                            [<Key>]
```

One blank cell, the three-cell mark, one blank, Label padded to `label_w`,
then the bracketed Key. Shown at the S10 default, `label_w` 36.

## Slots
| Slot  | Required | Position                                       | Overflow                                           |
|-------|----------|------------------------------------------------|----------------------------------------------------|
| Mark  | yes      | columns 1 to 3: `[x]` when on, `[ ]` when off  | never cut                                          |
| Label | yes      | from column 5, padded or cut to `label_w`      | cut at `label_w`, no marker                        |
| Key   | no       | directly after the Label pad, as `[k]`         | dropped when it does not fit; Label is never cut for it |

## Sizing
Height 1. Width is `label_w + 8` (blank, mark, blank, label, three-cell
key). Default `label_w` 36, so the S10 row is 44 cells inside a 58-cell
inner width. `min_width` is 5 plus the longest label. Never writes outside
its area.

## Variants
| Variant | What changes                                                      | Used by                   |
|---------|-------------------------------------------------------------------|---------------------------|
| Toggle  | two states; Mark is `[x]` or `[ ]`                                | S10 `a`, `b`, `c`, `d`    |
| Cycle   | three or more states; Mark is on unless the state is the first, Label carries the state name | S10 `t` day/night tint |
| No key  | Key slot empty                                                    | none yet                  |

## Interaction
Standard keys are in [keys.md](keys.md); the table below is what the live code does,
and its deviations from the standard are collected there.

| Key  | Row                                | Effect                                            |
|------|------------------------------------|---------------------------------------------------|
| `a`  | auto-pause on extinction           | toggle                                            |
| `b`  | log births to the event log        | toggle                                            |
| `c`  | pause when a followed creature dies| toggle                                            |
| `t`  | day/night tint: `off` / `map` / `status text` | cycle three states; the mark is on for anything but `off` |
| `d`  | auto-pause on epidemic             | toggle                                            |

Every change is written to `ui.toml` at once. `↑` / `↓` do nothing; the
rows are not a focus list.

## Styling
- Mark on: `theme::GOOD` bold on `theme::PANEL_BG`.
- Mark off: `theme::dim_text()`.
- Label: `theme::text()`.
- Key: `theme::key()`, the same style as a [Key Hint](key-hint.md).
- Nothing is background-filled beyond `theme::PANEL_BG`.

## Glyphs
None from `glyphs::`. `[`, `x` and `]` are plain ASCII. (`glyphs::DEATH`
is also `x`; the mark does not use it.)

## Composition
*Contains:* [Key Hint](key-hint.md) (the Key slot).
*Contained by:* [Panel](panel.md) and [Modal](modal.md) bodies, under a
[Divider](divider.md) section.

## API
### Today
```rust
ui::screens::s10_controls::options_panel(f, inner, row, app)
```
The `toggles` loop inside it draws the five rows as one `Line` each:
`" "`, the mark, `format!(" {label:<36}")`, `format!("[{key}]")`.

### Planned
```rust
Checkbox::new("auto-pause on extinction", true)   // label, on
    .key("a")                                     // optional
    .label_w(36)                                  // default shown
    .render(buf, row_area)

Checkbox::new("day/night tint: map", tint != DayNightTint::Off).key("t")   // Cycle
```
`height` is 1; `min_width` is `label_w + 8` when a key is set. The key
binding stays with the screen; the component only shows it.

## Gaps today
- `label_w` is a literal `36` in the format string.
- Cycle is not a distinct drawing path; the caller passes `on` as
  `state != Off` and bakes the state name into the label.
- No focus or arrow-key navigation, so the rows cannot be reached from a
  keyboard without a letter to spare. The spec does not add one (see Open
  questions).
- Rows cannot be focused, so `Space` cannot toggle them as [keys.md](keys.md)
  expects; each row needs its own letter.

## Examples

### Toggle, the S10 Options section (60 columns)
Copied from [S10a](../screens/renders/S10a.txt).
```
║ [x] auto-pause on extinction            [a]              ║
║ [ ] log births to the event log         [b]              ║
║ [x] pause when a followed creature dies [c]              ║
```

### Cycle (60 columns)
The live rows added after S10a was drawn. `[t]` cycles `off`, `map`,
`status text`.
```
║ [x] day/night tint: map                 [t]              ║
║ [x] auto-pause on epidemic              [d]              ║
```

### Narrow, `label_w` 32 (43 columns)
```
║ [x] auto-pause on extinction        [a] ║
║ [ ] log births to the event log     [b] ║
║ [x] day/night tint: map             [t] ║
```

## Open questions
- Should a Cycle row show its state in the mark (for instance `[1]`, `[2]`)
  instead of `[x]` plus a label suffix?
- Should Checkbox gain a focused look (`theme::selected()` like
  [Menu](menu.md)) so a screen can stack them under `↑` / `↓` with `Space`
  toggling, or do letter keys stay the only binding?
