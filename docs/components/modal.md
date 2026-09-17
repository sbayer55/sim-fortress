# Modal

Back to the [component index](README.md).

A Focus [Panel](panel.md) centred over the screen it interrupts, with everything
beneath it dimmed. Holds a body, an optional row of buttons, and a hint line.
The confirm dialog, [S10](../screens/s10-simulation-controls.md),
[S11](../screens/s11-legend-help.md), [S12](../screens/s12-alert-modal.md) and
the Load World list on [S00](../screens/s00-title.md) are Modals.

## Anatomy
The backdrop is every cell outside the box. Shown at 43 columns; real modals
are wider, see Sizing.

```
╔ <Title> ════════════════════════ <Info> ╗
║ <Body>                                  ║
║                                         ║
║        <Button>    <Button>             ║
║                 <Hint>                  ║
╚═════════════════════════════════════════╝
```

## Slots
| Slot    | Required | Position                                                        | Overflow                                                  |
|---------|----------|-----------------------------------------------------------------|-----------------------------------------------------------|
| Title   | no       | top edge, as [Panel](panel.md)                                  | as Panel                                                  |
| Info    | no       | top edge, right-aligned, as Panel                               | as Panel                                                  |
| Body    | yes      | inner rows from the top down to the row above Buttons           | clipped by the border; S11 wraps it in a [Scroll Region](scroll-region.md) |
| Button  | no       | one row of `[ Label ]` tokens, four cells between, centred      | never cut; the modal is sized to fit them                 |
| Hint    | no       | last inner row, centred                                         | cut at the inner width                                    |

The backdrop is not a slot. Before a non-opaque screen is drawn,
`ui::screens::render_stack` dims every cell of the whole screen by 0.55 toward
`theme::BG`, foreground and background alike. The modal then draws over it and
the panel fill restores full colour inside the box. Every modal except confirm
repaints the [Status Bar](status-bar.md) row undimmed afterwards.

## Sizing
Fixed `w × h` chosen by the caller, clamped to the area. Callers pass
`w.min(area.width − 2)` and `h.min(area.height − 2)` so a one-cell margin
survives on a small terminal; S12 passes 64 × 12 unclamped. The box sits at
`x + (area.width − w) / 2`, `y + (area.height − h) / 2` with integer division,
so an odd remainder puts the spare cell on the right and the bottom. Inner area
is `(w − 2) × (h − 2)`. Sizes today:

| Modal                   | Size     | Top edge                                  |
|-------------------------|----------|-------------------------------------------|
| confirm                 | 50 × 7   | untitled                                  |
| S10 Simulation Controls | 60 × 21  | `Simulation Controls`, Info `Esc closes`  |
| S12 Alert               | 64 × 12  | untitled, Banner `‼ EXTINCTION ‼`         |
| S11 Legend & Help       | 120 × 38 | `Legend & Help`, Info `? or Esc closes`   |
| Load World              | 70 × 20  | titled, Info names the selected save      |

Minimum is the Panel minimum, 3 × 3. A modal shorter than Body plus Buttons
plus Hint loses rows from the bottom, Hint first.

## Variants
| Variant      | What changes                                                                       | Used by              |
|--------------|------------------------------------------------------------------------------------|----------------------|
| Titled       | Title and Info in the top edge                                                     | S10, S11, Load World |
| Untitled     | continuous top edge                                                                | confirm              |
| Banner       | untitled edge with a centred bold heading written over it in the caller's colour   | S12a `‼ EXTINCTION ‼`, S12b `☻ EPIDEMIC ☻` |
| With buttons | Button row, Hint under it                                                          | confirm, S12         |
| Hint only    | no Buttons; Hint is a row of [Key Hint](key-hint.md) tokens                        | S10                  |
| Scrolling    | Body is a Scroll Region; `↑n ↓m` Foot in the bottom edge                           | S11                  |

## Interaction
| Key       | Standard                         | Here                                                       |
|-----------|----------------------------------|------------------------------------------------------------|
| `Esc`     | close                            | closes the modal without acting                            |
| `Enter`   | select                           | activates the focused button when there is a Button Row    |
| `Space`   | select                           | as `Enter`                                                 |
| `←` `→`   | move along a row                 | move focus along the Button Row                            |
| `Tab`     | next field                       | as `→` when the only focus list is the Button Row          |
| `↑` `↓`   | scroll                           | scroll the Body when it is a Scroll Region                 |

See [keys.md](keys.md) for the full table. Today S10 uses `Space` for pause,
S12 uses `Space` to dismiss, and confirm accepts `n`; those are listed there.

## Styling
- Backdrop: every cell of the screen passed through `theme::dim(c, 0.55)` by
  `util::dim_area`, so it is a darker copy of the screen below, not a flat fill.
- Box: Panel `Kind::Focus`, so `theme::border_focus()` and a `theme::PANEL_BG`
  fill.
- Title: `theme::title()`. Info: `theme::dim_text()`. Banner: bold in the
  caller's colour on `theme::PANEL_BG` (`theme::MAGENTA` for extinction,
  `theme::SICK` for epidemic).
- Body: the caller's rows. S12's headline is bold in the Banner colour;
  confirm's question is `theme::TEXT_BRIGHT` on `theme::PANEL_BG`.
- Buttons: `theme::text()`; the focused button `theme::selected()`.
- Hint: `theme::dim_text()`. S10's Hint uses `theme::key()` for the key tokens.
- Status Bar row: refilled with `theme::STATUS_BG` and redrawn after the box.

## Glyphs
Box from ratatui `BorderType::Double`. Banner: `glyphs::EXTINCTION` `‼`,
`glyphs::DISEASE` `☻`. Hints write `glyphs::LEFT` `←` and `glyphs::RIGHT` `→`
as the literal string `←→`. Foot glyphs come from Scroll Region.

## Composition
*Contains:* [Panel](panel.md) (Focus). Body is a [VStack](vstack.md) of: [Divider](divider.md),
[Labeled Bar](labeled-bar.md), [Checkbox](checkbox.md), [Stepper](stepper.md),
[Scroll Region](scroll-region.md), [Table](table.md), [Menu](menu.md).
[Button Row](button-row.md). [Key Hint](key-hint.md).
*Contained by:* the screen stack only. A modal is a `Screen` whose `opaque()`
returns `false`; it is never nested inside a panel.

## API
### Today
```rust
widgets::util::centered(area: Rect, w: u16, h: u16) -> Rect
widgets::util::dim_area(buf: &mut Buffer, area: Rect, amount: f32)   // render_stack calls it with 0.55
widgets::panel::draw(f, modal, "", panel::Kind::Focus) -> Rect
widgets::panel::draw_with_hint(f, modal, title, hint, panel::Kind::Focus) -> Rect
```
plus `fn opaque(&self) -> bool` on the `Screen` trait returning `false`.
Buttons and Hint are hand-laid spans in `confirm.rs`, `s12_alert.rs` and
`s10_controls.rs`.

### Planned
```rust
Modal::new(60, 21)                                  // size, clamped to the area
    .title("Simulation Controls")                   // optional; untitled by default
    .info("Esc closes")                             // optional
    .banner("‼ EXTINCTION ‼", theme::MAGENTA)       // Banner variant
    .buttons(&["[ Continue ]", "[ View lineage ]", "[ Pause ]"], focus)
    .hint("Enter select   ←→ move   Esc continue")
    .render(buf, area) -> Rect                      // dims the backdrop, draws the box, returns the Body area
```
`height` is `h`; `min_width` is the widest of the Button row, the Hint, and
Title plus Info.

## Gaps today
- confirm puts Buttons and Hint on one left-aligned row. That row,
  `  [ Yes ]    [ No ]    ←→ move  Enter select  Esc no`, is 52 cells in a
  48-cell inner width, so the hint is cut after `Es`. The spec centres them on
  two rows.
- confirm draws its question at inner column 0; every other modal indents by
  two cells.
- confirm's focused button is `theme::CURSOR_FG` on `theme::ACCENT`, not
  `theme::selected()` as in S12.
- S10's Hint is left-aligned with a leading space, not centred.
- confirm does not repaint the Status Bar, so the bar stays dimmed under it.
- The backdrop dim lives in `render_stack`; a modal cannot choose its own
  amount.
- S12 and confirm each do their own button padding arithmetic; there is no
  shared [Button Row](button-row.md).
- The S12a prototype render pads the button row with eight cells; the live
  code centres it with six.
- Keys: S10 uses `Space` for pause and S12 uses `Space` to dismiss, where the
  standard says `Space` selects; confirm accepts `n` with no Key Hint for it.
  S10 also handles `+` `-` for speed and `.` to step, and the app-level
  fallback does the same from any screen; the standard keeps speed and step
  keys on the main screen only.
  See [keys.md](keys.md).

## Examples

### Simple (43 columns)
```
╔ Greeting ═══════════════════════════════╗
║ Hello!                                  ║
║                                         ║
║        [ OK ]    [ Cancel ]             ║
║        Enter select   Esc close         ║
╚═════════════════════════════════════════╝
```

### Confirm, untitled (50 columns)
Spec layout for the 50 × 7 confirm dialog. See Gaps today for how the live
dialog differs.
```
╔════════════════════════════════════════════════╗
║                                                ║
║World has unsaved changes. Return to title?     ║
║                                                ║
║             [ Yes ]    [ No ]                  ║
║         ←→ move  Enter select  Esc no          ║
╚════════════════════════════════════════════════╝
```

### Alert, Banner (64 columns)
S12a rows 18 to 29, with the button row padded as the live code does.
```
╔═══════════════════════ ‼ EXTINCTION ‼ ═══════════════════════╗
║                                                              ║
║                     The Lynx are extinct                     ║
║                                                              ║
║  Year 12, Day 4 of Autumn ♫   last individual: Gloam l#088   ║
║  x starved in Sunfall Coast at (131, 9), age 412 days        ║
║                                                              ║
║  peak population 32   generations survived 19   years 12     ║
║                                                              ║
║      [ Continue ]    [ View lineage ]    [ Pause ]           ║
║            Enter select   ←→ move   Esc continue             ║
╚══════════════════════════════════════════════════════════════╝
```

### Controls, titled with Info and Hint only (60 columns)
S10a rows 15 to 32. The prototype body is 18 rows; the live modal is 21 rows
with two more toggles and an autosave row above the same Hint.
```
╔ Simulation Controls ═════════════════════════ Esc closes ╗
║ state   ► RUNNING      ►► x2  ││ Space toggles           ║
║                                                          ║
║ speed    x1   x2   x5   x10   x25     +/- or 1-5         ║
║ step     1 tick   6 hours   1 day     →│ . steps once    ║
║                                                          ║
║─ Clock ──────────────────────────────────────────────────║
║ tick 1,064,772    day 4 of Autumn ♫    year 12    14:00 ☼║
║ 1 tick = 1 hour   1 day = 24 ticks   x2 = 4 ticks/s      ║
║                                                          ║
║─ Options ────────────────────────────────────────────────║
║ [x] auto-pause on extinction            [a]              ║
║ [ ] log births to the event log         [b]              ║
║ [x] pause when a followed creature dies [c]              ║
║                                                          ║
║                                                          ║
║ [Space] pause  [+/-] speed  [.] step  [Esc] close        ║
╚══════════════════════════════════════════════════════════╝
```

### Scrolling
The 120 × 38 help modal is too wide to reproduce here; see
[S11a](../screens/renders/S11a.txt) and [Scroll Region](scroll-region.md).

## Open questions
- Should the backdrop amount (0.55) become a `theme` constant?
- `←→` has no `glyphs::` constant as a pair; hints write the literal string.
  Add one, or build it from `glyphs::LEFT` and `glyphs::RIGHT`?
- Is Banner a Modal concern or a Panel title variant?
- Should confirm keep a 50 × 7 box once its hint moves to its own row, or
  shrink to 50 × 6?
