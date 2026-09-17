# Filter Strip

Back to the [component index](README.md).

A one-row strip of toggle chips, each a glyph and a name, with the active
chips highlighted and a right-aligned key hint. The event-kind filter at the
top of [S07](../screens/s07-event-log.md) is the only Filter Strip. It is
inherently wide: nine chips need 91 cells, and the hint another 28.

## Anatomy
Shown at 121 columns.

```
 <Glyph> <Name> <Glyph> <Name> ...                                                     <Hint>
```

One blank cell, then for each chip: the Glyph cell, one blank, Name, one
blank gap. Hint is right-aligned and ends one cell before the border.

## Slots
| Slot  | Required | Position                                                          | Overflow                                                   |
|-------|----------|-------------------------------------------------------------------|------------------------------------------------------------|
| Glyph | yes      | first cell of each chip                                           | never cut                                                  |
| Name  | yes      | one blank after Glyph, then `gap` blanks before the next chip     | chips past the right edge are cut; the row does not wrap   |
| Hint  | no       | right-aligned, one blank before the border                        | dropped when fewer than two blanks would separate it from the last chip |

## Sizing
Height 1. Width is the area width. The live chip set is 91 cells including
the leading blank and the trailing gap; with the 25-cell Hint and its
margins the row needs 119 cells, so the strip only shows the Hint in a
panel at least 121 columns wide. `gap` defaults to 1. `min_width` is the
chip set without Hint. Never writes outside its area.

## Variants
| Variant      | What changes                                                       | Used by                        |
|--------------|--------------------------------------------------------------------|--------------------------------|
| All          | only the `all` chip is active                                      | S07a full log                  |
| Filtered     | one or more kind chips active, `all` inactive                      | S07b, which also opens the detail panel |
| Hint dropped | the row is too narrow for Hint; chips unchanged                    | any panel narrower than 121 columns |

## Interaction
Standard keys are in [keys.md](keys.md); the table below is what the live code does,
and its deviations from the standard are collected there.

| Key     | Effect                                                                                  |
|---------|-----------------------------------------------------------------------------------------|
| `1`     | `all`: clears every kind chip                                                           |
| `2`-`8` | toggle births, deaths, mutations, migrations, extinctions, droughts, disease; the first toggle from `all` selects that kind alone; clearing the last kind returns to `all` |
| `f`     | cycle presets: all, deaths + extinctions, migrations + droughts, disease, all           |

Any change resets the list selection to the newest event. Chips are not a
focus list; `←` / `→` do nothing.

## Styling
- Glyph of `all`: `theme::key()`. Glyph of a kind chip: that kind's colour
  from `EventKindStyle::color()` (`theme::GOOD` births, `theme::BAD` deaths,
  `theme::INFO` mutations, `theme::ACCENT` migrations, `theme::MAGENTA`
  extinctions, `theme::WARN` droughts and wary, `theme::SICK` disease) on
  `theme::PANEL_BG`. The glyph colour does not change with the active state.
- Name, active: `theme::selected()`, including its leading blank.
- Name, inactive: `theme::text()`.
- Hint: `theme::dim_text()`.
- Gaps are not background-filled beyond `theme::PANEL_BG`.

## Glyphs
`glyphs::BIRTH` `♥`, `glyphs::DEATH` `x`, `glyphs::MUTATION` `§`,
`glyphs::MIGRATION` `→`, `glyphs::EXTINCTION` `‼`, `glyphs::DROUGHT` `¡`,
`glyphs::DISEASE` `☻`, `glyphs::ALERT` `!`. The `all` chip's `*` and the
`[` `]` in the Hint are plain ASCII.

## Composition
*Contains:* [Key Hint](key-hint.md) (the Hint slot).
*Contained by:* the first inner row of the S07 [Panel](panel.md), above the
[Table](table.md) header.

## API
### Today
```rust
ui::screens::s07_log::EventLog::chips(&self, f, inner)
ui::screens::s07_log::ChipFilter::{toggle(key), cycle(), matches(kind)}
```
`chips` draws the strip on inner row 0 and the table header on row 1.
`ChipFilter` is the state: `all: bool` and `kinds: [bool; 8]`.

### Planned
```rust
FilterStrip::new(&[
        ('*', "all"), (glyphs::BIRTH, "births"), (glyphs::DEATH, "deaths"),
        (glyphs::MUTATION, "mutations"), (glyphs::MIGRATION, "migrations"),
        (glyphs::EXTINCTION, "extinctions"), (glyphs::DROUGHT, "droughts"),
        (glyphs::DISEASE, "disease"), (glyphs::ALERT, "wary"),
    ])
    .active(&[true, false, false, false, false, false, false, false, false])  // caller-owned state
    .colors(&kind_colors)                // one per chip; default theme::TEXT
    .hint("[f] cycles, [1-9] toggles")   // optional
    .gap(1)                              // default shown
    .render(buf, row_area)
```
`height` is 1; `min_width` as in Sizing. The toggle rules stay in
`ChipFilter`; the component only draws.

## Gaps today
- The Hint says `[1-9]` but only `1` to `8` are handled, so the `wary` chip
  (key `9`) cannot be toggled from the keyboard. It is reachable by no key
  at all.
- Six of the chip glyphs are literal characters in the chip table rather
  than `glyphs::` constants; only `DISEASE` and `ALERT` use the constant.
- The prototype [S07a](../screens/renders/S07a.txt) row reads
  `filter:  all   ♥ births   x deaths … [f] cycles, [1-7] toggles`: a
  `filter:` prefix, three-cell gaps, seven chips. The live strip has no
  prefix, one-cell gaps and nine chips. The sheet follows the live strip;
  the prefix and wider gap are an open question.

## Examples

### All (121 columns)
The narrowest panel that still shows the Hint. `all` is active.
```
║ * all ♥ births x deaths § mutations → migrations ‼ extinctions ¡ droughts ☻ disease ! wary  [f] cycles, [1-9] toggles ║
```

### Filtered (121 columns)
Cell for cell the same; `deaths` and `extinctions` take `theme::selected()`
and `all` drops back to `theme::text()`.
```
║ * all ♥ births x deaths § mutations → migrations ‼ extinctions ¡ droughts ☻ disease ! wary  [f] cycles, [1-9] toggles ║
```

### Hint dropped (95 columns)
```
║ * all ♥ births x deaths § mutations → migrations ‼ extinctions ¡ droughts ☻ disease ! wary  ║
```

## Open questions
- Should the strip keep the prototype's `filter:` prefix and three-cell
  gaps? The live one-cell gap makes `x deaths § mutations` hard to scan;
  the wider layout needs 27 more cells, which S07's 153-cell inner width
  has.
- The `all` chip's `*` has no `glyphs::` constant (`SEED` and `SNOW` are
  also `*` but mean other things). Add `glyphs::ALL` or reuse
  `glyphs::BULLET` `•`?
- Should the active state also tint the Glyph cell, so a filtered chip
  stands out on terminals where `theme::SELECT_BG` is faint?
