# Key Hint

Back to the [component index](README.md).

The inline `[k] label` token: a key name in brackets and a short label. It
makes up the [Status Bar](status-bar.md), sits in panel bodies (the
[S01](../screens/s01-world-map.md) sidebar), in menus
([S00](../screens/s00-title.md) `[Enter]`), and in the hint lines and option
rows of [S10](../screens/s10-simulation-controls.md) and
[S12](../screens/s12-alert-modal.md).

## Anatomy
Inline; shown at 43 columns inside a panel border.

```
║ [<Key>] <Label>                         ║
```

## Slots
| Slot  | Required | Position                                                  | Overflow                 |
|-------|----------|-----------------------------------------------------------|--------------------------|
| Key   | yes      | in brackets; one name or a group such as `-/=`, `←→`, `1-5`, `Shift+Tab` | never cut     |
| Label | no       | one cell after `]`                                        | cut at the row's right edge |

A row of hints joins tokens with a separator: two cells in the Status Bar and
in S10's hint line, ` · ` in the S01 sidebar, three cells in S12's hint line.

## Sizing
Height 1. Width is `len(Key) + 2`, plus `1 + len(Label)` when there is a
Label. The Menu variant is placed by the container at `inner.right() − 8`,
seven cells for `[Enter]` and one trailing cell.

## Variants
| Variant     | What changes                                                              | Used by                                   |
|-------------|---------------------------------------------------------------------------|-------------------------------------------|
| Status bar  | Key bold `theme::KEY`, Label `theme::TEXT`, both on `theme::STATUS_BG`    | every screen's bottom row                 |
| In panel    | Key `theme::key()`, Label `theme::dim_text()`                             | S10 hint line                             |
| Key only    | no Label                                                                  | S10 option rows `[a]`, autosave `[←→]`    |
| Menu        | Key only, bold `theme::KEY` on `theme::SELECT_BG`, right-aligned on the selected row | S00 `[Enter]`                  |
| Dim         | the whole token `theme::dim_text()`, Key not highlighted                  | S01 sidebar `[k] look · [e] events · [g] charts` |
| Bracketless | no brackets, whole line `theme::dim_text()`                               | S12 and confirm hint lines, S11 key column |

## Interaction
None. Display only. It is the spelling half of [keys.md](keys.md): the token must match
the *Written as* column there (`[Enter]`, `[Esc]`, `[←→]`, `[-/=]`, `[a/b/c]`).

## Styling
- Key: `theme::KEY`, bold, on the row's background. `theme::key()` gives this
  on `theme::PANEL_BG`; the Status Bar and Menu variants build the same style
  on `theme::STATUS_BG` and `theme::SELECT_BG` by hand.
- Label: `theme::TEXT` in the Status Bar; `theme::dim_text()` in panel hint
  lines.
- Dim and Bracketless: `theme::dim_text()` throughout.

## Glyphs
Brackets are ASCII. Key names may carry `glyphs::UP` `↑`, `glyphs::DOWN` `↓`,
`glyphs::LEFT` `←` and `glyphs::RIGHT` `→`, written as literal strings today.

## Composition
*Contains:* nothing.
*Contained by:* [Status Bar](status-bar.md), [Modal](modal.md) hint lines,
[Menu](menu.md) rows, [Checkbox](checkbox.md) rows, [Panel](panel.md) bodies.

## API
### Today
Inline spans, one per site:
```rust
// widgets/status.rs
Span::styled(format!("[{k}]"), bg.fg(theme::KEY).add_modifier(Modifier::BOLD))
Span::styled(format!(" {label}  "), bg.fg(theme::TEXT))
// ui/screens/s10_controls.rs
Span::styled("[Space]", theme::key())
Span::styled(" pause  ", theme::dim_text())
// ui/screens/s00_title.rs
f.buffer_mut().set_stringn(inner.right() - 8, inner.y + row, "[Enter]", 7, Style::default().fg(theme::KEY).bg(theme::SELECT_BG).add_modifier(Modifier::BOLD))
// theme.rs
theme::key() -> Style
```

### Planned
```rust
KeyHint::new("k", "look")                    // "[k] look"
    .bg(theme::PANEL_BG)                     // default; STATUS_BG or SELECT_BG for the bar and menus
    .label_style(theme::dim_text())          // default theme::text()
    .render(buf, cell_area)
KeyHint::key("Enter")                        // Key only
KeyHint::row(&[("Space", "pause"), ("Esc", "close")])   // joined with two cells
    .render(buf, row_area)
```
`height` is 1; `min_width` is the token width.

## Gaps today
- The S01 sidebar draws the whole line in `theme::dim_text()`, so its keys
  are not highlighted.
- S12 and confirm write their hints without brackets.
- Three separators are in use: two cells, ` · `, three cells.
- `theme::key()` carries `theme::PANEL_BG`, so every other background rebuilds
  the style.
- S00 hard-codes the `[Enter]` offset instead of measuring the token.

## Examples

### In a panel body, S01 sidebar (43 columns)
```
║ [k] look · [e] events · [g] charts      ║
```

### Modal hint line, S10 (43 columns)
```
║ [Space] pause  [Esc] close              ║
```

### Menu, right-aligned on the selected row (43 columns)
The S00 `New World` row at the canonical width.
```
║   ► New World                   [Enter] ║
```

### Bracketless, centred, S12 (43 columns)
```
║  Enter select   ←→ move   Esc continue  ║
```

### In the status bar (43 columns)
Bare, on `theme::STATUS_BG`, with a Right message.
```
 [k] look  [e] events  [g] charts     help 
```

## Open questions
- One separator for every site, or keep ` · ` for prose-like rows in panel
  bodies?
- Should S12's hint gain brackets so every hint line reads the same?
- `←→` and `↑↓` pairs have no `glyphs::` constants; add `ARROWS_LR` and
  `ARROWS_UD`?
