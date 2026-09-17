# Scroll Region

Back to the [component index](README.md).

A panel body taller than its panel. The body is drawn into a tall off-screen
canvas, the window at `offset` is copied into the panel, and `↑n ↓m` is written
into the panel's bottom border. Used by the three inspector panes in
[S03](../screens/s03-creature-inspector.md), the species summary in
[S04](../screens/s04-species-browser.md), and the help modal in
[S11](../screens/s11-legend-help.md).

## Anatomy

```
╔ <Title> ════════════════════════════════╗
║ <Window>                                ║
║                                         ║
╚══════════════════════════════ ↑<n> ↓<m> ╝
```

## Slots
| Slot   | Required          | Position                                                     | Overflow                                                          |
|--------|-------------------|--------------------------------------------------------------|-------------------------------------------------------------------|
| Window | yes               | the panel's inner area; canvas rows `offset .. offset + h`   | rows outside the window are hidden, not cut; rows past `CANVAS_H` are never drawn |
| n      | when `offset > 0` | in the Foot as `↑n`                                          | Foot dropped when the bottom edge is narrower than the text plus 2 |
| m      | when rows remain  | in the Foot as `↓m`, after `↑n`                              | same                                                              |

The Foot text is ` ↑n ↓m `: a leading space, then each part followed by a
space. A part whose count is 0 is omitted. It is right-aligned in the bottom
border with one border cell kept after it, and only written when the content is
taller than the window.

## Sizing
Fills the panel's inner area. The canvas is `inner.width × CANVAS_H` cells,
`CANVAS_H` being 160, and shares the inner area's `x` and `y` so absolute
drawing keeps working. The body reports the rows it used; `content` is that
count capped at `CANVAS_H`. `max_offset = content − inner.height`, saturating
at 0, and `offset` is clamped to it at draw time. The call returns
`Overflow { content, offset }` so the caller can size PageUp and PageDown and
clamp the offset it stores. No parameters other than `offset`.

## Variants
| Variant  | What changes                                              | Used by                         |
|----------|-----------------------------------------------------------|---------------------------------|
| Fits     | content ≤ window height; no Foot                          | any short body                  |
| Top      | `offset` 0, Foot ` ↓m `                                   | S04a summary at rest, `↓2`      |
| Middle   | Foot ` ↑n ↓m `                                            | any scrolled body               |
| Bottom   | `offset` at max, Foot ` ↑n `                              | S11 after End                   |
| Per-pane | three regions side by side, each with its own offset      | S03 inspector                   |
| In modal | Foot lands in the modal's bottom edge                     | S11 help                        |

## Interaction
| Key           | Standard              | Here                                                    |
|---------------|-----------------------|---------------------------------------------------------|
| `↑` `↓`       | scroll one row        | offset −1 / +1, clamped to `Overflow::max_offset`       |
| `PgUp` `PgDn` | scroll one page       | offset ∓ the visible height                             |
| `Home` `End`  | first / last          | offset 0 / `max_offset`                                 |

The screen keeps the offset and passes it in; see [keys.md](keys.md). S03 uses
`←` `→` to switch which panel the arrows scroll, which is a deviation listed
there.

## Styling
- Canvas: filled with `theme::PANEL_BG` before the body draws, so unused rows
  match the panel.
- Foot: `theme::dim_text()`, written over the border cells.
- Everything else is the body's own styling.

## Glyphs
`glyphs::UP` `↑` and `glyphs::DOWN` `↓`. The border is the
[Panel](panel.md)'s.

## Composition
*Contains:* a [VStack](vstack.md) of any row or area components; its
`height(width)` is the content height the foot reports. Today the body is a
closure that draws rows and returns the count.
*Contained by:* [Panel](panel.md) bodies, [Modal](modal.md) bodies.

## API
### Today
```rust
widgets::scroll::CANVAS_H: u16 = 160
widgets::scroll::Overflow { content: u16, offset: u16 }
widgets::scroll::Overflow::max_offset(content: u16, visible: u16) -> u16
widgets::scroll::draw(f, panel_area: Rect, inner: Rect, offset: u16, body: impl FnOnce(&mut Buffer, Rect) -> u16) -> Overflow
```
`panel_area` is the bordered panel whose bottom edge receives the Foot;
`inner` is its inner area. Callers: `s03_inspector.rs`, `s04_species.rs`,
`s11_help.rs`.

### Planned
```rust
ScrollRegion::new(&stack)                 // a VStack
    .offset(self.offset)
    .render(buf, inner) -> Overflow       // the container writes Panel::foot from it
```
`height` is the area height; `min_width` is the stack's widest `min_width`.

## Gaps today
- Takes a closure that reports its rows after drawing, not a VStack, so the
  height is not known before the blit and a stored offset is clamped one frame
  late. S11 keeps a `measured` cell to work around this.
- `CANVAS_H` silently drops rows past 160.
- Frame entry point only; a region cannot nest inside another canvas.
- Writes the Foot straight into the border; Panel has no Foot slot yet.
- The event log in [S07](../screens/s07-event-log.md) pages by hand and does
  not use it.

## Examples
The body is the nine-row S01 legend, shown in a panel with a two-row window.

### Fits, no Foot (43 columns)
Two rows of content in a two-row window.
```
╔ Legend ═════════════════════════════════╗
║ ≈ deep water       ~ shallow water      ║
║ · sand             . bare dirt          ║
╚═════════════════════════════════════════╝
```

### Top, offset 0 (43 columns)
```
╔ Legend ═════════════════════════════════╗
║ ≈ deep water       ~ shallow water      ║
║ · sand             . bare dirt          ║
╚═════════════════════════════════════ ↓7 ╝
```

### Middle, offset 3 (43 columns)
```
╔ Legend ═════════════════════════════════╗
║ ♣ meadow           ♠ forest             ║
║ ▲ rock             Ω den / burrow       ║
╚══════════════════════════════════ ↑3 ↓4 ╝
```

### Bottom, offset 7 (43 columns)
```
╔ Legend ═════════════════════════════════╗
║ Ff Fox       Ww Wolf      Ll Lynx       ║
║ UPPER adult   lower juvenile            ║
╚═════════════════════════════════════ ↑7 ╝
```

## Open questions
- The S07a prototype render ends with `35 more ↓` in the bottom border and a
  one-cell `█░` scrollbar column on the right edge (`glyphs::BAR_FILL`,
  `glyphs::BAR_EMPTY`). Neither is built. Keep the scrollbar as a variant for
  long tables, or is the Foot enough?
- Should the canvas grow with the content instead of capping at 160 rows?
- Should the Foot show a percentage or a page count for very long bodies?
