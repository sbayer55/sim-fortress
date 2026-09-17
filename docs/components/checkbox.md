# Checkbox

Back to the [component index](README.md).

A one-row on/off toggle: a `[x]` or `[ ]` mark, a label, and the key that
flips it. The Options section of [S10](../screens/s10-simulation-controls.md)
is a stack of five Checkboxes, each driven by its own letter with no focus
and no cursor. The layer rows of [S14](../screens/s14-overlay-switcher.md)
are the other use: a stack the cursor moves through, one row Focused, the
base rows drawn as Radio marks, with a remembered Value and a cue in place
of the Key.

## Anatomy

```
 [<Mark>] <Label>                            [<Key>]
  (<Mark>) <Label>    <Value>       »
```

One blank cell, the three-cell mark, one blank, Label padded to `label_w`,
then the bracketed Key. Shown at the S10 default, `label_w` 36. The second
row is the S14 form: an `indent` of one, a Radio mark, `label_w` 11, the
Value one blank after the pad and cut to `value_w` 13, and the cue directly
after the Value slot.

## Slots
| Slot  | Required | Position                                       | Overflow                                           |
|-------|----------|------------------------------------------------|----------------------------------------------------|
| Mark  | yes      | columns 1 to 3 after the indent: `[x]` when on, `[ ]` when off; `(•)` / `( )` for Radio | never cut |
| Label | yes      | from column 5, padded or cut to `label_w`      | cut at `label_w`, no marker                        |
| Key   | no       | directly after the Label pad, as `[k]`         | dropped when it does not fit; Label is never cut for it |
| Value | no       | one blank after the Label pad, cut to `value_w`; a row has a Key or a Value, not both | cut at `value_w` |
| Cue   | no       | directly after the Value slot: `»` (`glyphs::CUE`) | dropped when it does not fit                   |

## Sizing
Height 1. Width is `label_w + 8` (blank, mark, blank, label, three-cell
key). Default `label_w` 36, so the S10 row is 44 cells inside a 58-cell
inner width. `min_width` is the indent plus 5 plus the longest label. The
S14 row is `indent 1 + 5 + 11 + 1 + 13 + 1` = 32 cells. Never writes outside
its area.

## Variants
| Variant  | What changes                                                      | Used by                   |
|----------|-------------------------------------------------------------------|---------------------------|
| Toggle   | two states; Mark is `[x]` or `[ ]`                                | S10 `a`, `b`, `c`, `d`; S14 Marks tab |
| Cycle    | three or more states; Mark is on unless the state is the first, Label carries the state name | S10 `t` day/night tint |
| No key   | Key slot empty                                                    | S14                       |
| Radio    | one of a set; Mark is `(•)` or `( )`                              | S14 Base heatmap tab      |
| Focused  | the cursor row: every cell of the row in `theme::selected()`, Label bold and bright, Cue in the accent | S14 |
| Disabled | cannot be turned on: the whole row in `theme::dim_text()`        | S14 Sense with no living predator |

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
rows are not a focus list. On S14 the rows *are* a focus list: `↑` `↓` move
the Focused row, `Space` toggles it (or makes it the base on the Radio tab)
and `→` opens its sub-pick list; see that screen's Interaction tables.

## Styling
- Mark on: `theme::GOOD` bold on the row background.
- Mark off: `theme::dim_text()`.
- Label: `theme::text()`; `theme::TEXT_BRIGHT` bold on the Focused row and
  on a row with a Value that is on.
- Key: `theme::key()`, the same style as a [Key Hint](key-hint.md).
- Value: `theme::text()` while the row is on, `theme::dim_text()` otherwise.
- Cue: the accent on the Focused row, `theme::dim_text()` elsewhere.
- Row background: `theme::PANEL_BG`, or `theme::SELECT_BG` over the whole
  area when Focused. Disabled draws every slot in `theme::dim_text()`.

## Glyphs
`glyphs::BULLET` `•` inside a Radio mark that is on and `glyphs::CUE` `»`
for the Cue; CP437 has no `›`, so the S14 mockup's cue is drawn as `»`.
`[`, `x`, `]`, `(` and `)` are plain ASCII. (`glyphs::DEATH` is also `x`;
the mark does not use it.)

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

Checkbox::new("Species", false).radio(true).label_w(11).indent(1)          // S14 row
    .value("hare").cue().focused(cursor == row).disabled(false)
```
`height` is 1; `min_width` is `label_w + 8` when a key is set. The key
binding stays with the screen; the component only shows it.

## Gaps today
- Cycle is not a distinct drawing path; the caller passes `on` as
  `state != Off` and bakes the state name into the label.
- S10's rows are still letter-driven only; the Focused look exists for S14
  and S10 has not adopted a cursor.

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

### Radio, the S14 Base heatmap tab (34 columns)
Rows of [S14a](../screens/s14-overlay-switcher.md): `indent` 1, `label_w`
11, `value_w` 13, the cursor not on these rows.
```
║  ( ) None                      ║
║  ( ) Vegetation                ║
║  (•) Moisture                  ║
║  ( ) Species     hare         »║
```

### Focused (34 columns)
The Marks tab with the cursor on Disease (its row is filled with the
selection background) and Sense disabled for want of a living predator.
```
║  [ ] Sense       no predators »║
║  [x] Regions                   ║
║  [x] Disease     All pathogens»║
```

## Open questions
- Should a Cycle row show its state in the mark (for instance `[1]`, `[2]`)
  instead of `[x]` plus a label suffix?
- Should S10 adopt the Focused variant and a cursor, so `Space` toggles
  there as [keys.md](keys.md) expects, or do its letter keys stay?
