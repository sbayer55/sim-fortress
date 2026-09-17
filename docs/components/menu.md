# Menu

Back to the [component index](README.md).

A vertical list of choices with one selected row. The selected row is filled
with the selection background and carries a `►` marker and an optional
right-aligned key hint. The main menu on [S00](../screens/s00-title.md) and
the save list in the Load World modal (also reached from S00) are Menus.

## Anatomy

```
║   ► <Label>                    <Hint> ║
║     <Label>                           ║
║     <Label>                    <Note> ║
```

One row per item. Three blank cells, the Marker cell, one blank, then Label.
Hint and Note share the right edge and end one cell before the border.

## Slots
| Slot   | Required | Position                                                        | Overflow                                          |
|--------|----------|-----------------------------------------------------------------|---------------------------------------------------|
| Marker | yes      | column 3 of the row; `►` on the selected row, blank elsewhere   | never cut                                         |
| Label  | yes      | from column 5, padded to `label_w`                              | cut at the row's right edge, no marker            |
| Hint   | no       | right-aligned, one blank before the border, selected row only   | dropped when it would overwrite Label             |
| Note   | no       | same place as Hint, disabled rows only (`(empty)`)              | dropped when it would overwrite Label             |

## Sizing
Height is the number of items; `Component::height` ignores the width. Width is
the area width; every row is filled to it so the selection background spans
the row. `label_w` defaults to 24. `min_width` is 5 plus the longest label,
or 5 plus `label_w` plus 8 when a Hint is set. Never writes outside its area.

## Variants
| Variant       | What changes                                                              | Used by                |
|---------------|---------------------------------------------------------------------------|------------------------|
| Marker        | `►` at column 3 on the selected row; `[Enter]` Hint at the right edge     | S00 main menu          |
| Disabled row  | row in `theme::dim_text()`, never selectable, Note `(empty)` at the right | S00 `Load World` with no saves |
| Row highlight | no Marker column; Label from column 1; whole row filled with `theme::SELECT_BG` | Load World list  |

## Interaction
Standard keys are in [keys.md](keys.md); the table below is what the live code does,
and its deviations from the standard are collected there.

| Key        | S00 main menu                                            | Load World                    |
|------------|----------------------------------------------------------|-------------------------------|
| `↑` / `↓`  | move; wraps at both ends; skips disabled rows            | move; stops at the ends       |
| `Enter`    | activate the selected row                                | load the selected save        |
| `Del`      | not handled                                              | confirm, then delete the save |
| `Esc`      | not handled                                              | close the modal               |
| `q`, `w`   | quit (with a confirm when the world is dirty)            | not handled                   |

Load World also keeps a scroll offset so the selection stays inside 14
visible rows; see [Scroll Region](scroll-region.md).

## Styling
- Selected row: the whole row is `theme::selected()` (`theme::TEXT_BRIGHT`
  bold on `theme::SELECT_BG`), Marker included.
- Other rows: `theme::text()`.
- Disabled rows: `theme::dim_text()`, Note included.
- Hint: `theme::KEY` bold on `theme::SELECT_BG`, so it reads as a
  [Key Hint](key-hint.md) sitting on the selection bar.
- Background: rows are padded to the area width, so the selected row is a
  continuous bar and the others are `theme::PANEL_BG`.

## Glyphs
`glyphs::PLAY` `►` for Marker. The `[` `]` around the Hint are plain ASCII.

## Composition
*Contains:* [Key Hint](key-hint.md) (the Hint slot).
*Contained by:* [Panel](panel.md) bodies, [Modal](modal.md) bodies
(Load World is a 70 × 20 Focus panel).

## API
### Today
```rust
ui::screens::s00_title::menu_box(f, area, newest: Option<&SaveHeader>, selection: usize)
ui::screens::load_world::LoadWorld::render(&self, app, f, area)   // list loop
ui::screens::load_world::highlight_row(f, inner, y)               // Row highlight fill
```
`menu_box` draws the whole 34 × 8 Focus panel and the four `MENU` rows.
The Load World loop formats each save as
`{:<22}  Year {}, Day {:<3}  {:>3} prey /{:>3} pred` and highlights the
selected row by hand.

### Planned
```rust
Menu::new(&["New World", "Load World", "Options", "Quit"])
    .selected(0)                         // caller-owned state
    .disabled(&[1])                      // rows that cannot be selected
    .hint("[Enter]")                     // optional, selected row only
    .note("(empty)")                     // optional, disabled rows only
    .label_w(24)                         // default shown
    .render(buf, area)

Menu::new(&rows).selected(sel).marker(false)   // Row highlight
```
`height` is the item count; `min_width` as in Sizing. Key handling stays with
the screen; the component only draws.

## Gaps today
- The disabled row appends ` (empty)` after the 24-cell label pad instead of
  right-aligning it, and the 32-cell inner width of the S00 box cuts it to
  `(e`. The spec right-aligns it like Hint.
- Load World draws Label from column 0, flush against the border. The spec
  starts it at column 1.
- S00 wraps at the ends and Load World stops; the spec leaves wrap as a
  builder flag and does not settle a default (see Open questions).
- The row loop and the highlight fill are two separate steps in Load World;
  in S00 the padding does the fill. Planned API does one thing.
- `q` and `w` from a world screen return to the title instead of exiting the
  application; [keys.md](keys.md) makes `q` exit everywhere.

## Examples

### Marker, the S00 main menu (34 columns)
Copied from [S00a](../screens/renders/S00a.txt). The blank rows are the
caller's: the menu starts at inner row 1.
```
╔ Main Menu ═════════════════════╗
║                                ║
║   ► New World          [Enter] ║
║     Load World                 ║
║     Options                    ║
║     Quit                       ║
║                                ║
╚════════════════════════════════╝
```

### Marker with a disabled row (43 columns)
No saves on disk: `Load World` is dim and `↓` skips it.
```
╔ Main Menu ══════════════════════════════╗
║   ► New World                   [Enter] ║
║     Load World                  (empty) ║
║     Options                             ║
║     Quit                                ║
╚═════════════════════════════════════════╝
```

### Row highlight, the Load World list (70 columns)
No prototype render exists; the rows follow the live format string. The
first row is selected and its full width is `theme::SELECT_BG`.
```
╔ Load World ════════════════════ 3 saves  ·  saved 2026-09-14 21:12 ╗
║ The Valley of Sunfall   Year 12, Day 4    587 prey / 79 pred       ║
║ Ashen Hollow            Year 3, Day 117  402 prey / 51 pred        ║
║ Reedwater               Year 1, Day 9    310 prey / 40 pred  v2    ║
╚════════════════════════════════════════════════════════════════════╝
```

## Open questions
- Should `↑` at the top wrap to the bottom (S00) or stop (Load World)? One
  rule for both would be simpler; wrap is friendlier for a four-item menu,
  stop is safer for a long list.
- Should the Hint be a fixed `[Enter]` owned by the component, or stay a
  caller string?
