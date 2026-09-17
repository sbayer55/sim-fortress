# Ticker

Back to the [component index](README.md).

The one-line latest-event row between the map panel and the
[Status Bar](status-bar.md) on [S01](../screens/s01-world-map.md): the event's
glyph, its text, and a dim pointer to the full log in
[S07](../screens/s07-event-log.md). It stays visible, dimmed, under every modal
drawn over the map.

## Anatomy
The row is never inside a panel, so it is shown bare. Shown at 43 columns; the
live row is the screen width.

```
 <Glyph> <Text>   (e: full log)
```

## Slots
| Slot    | Required | Position                                    | Overflow                                                        |
|---------|----------|---------------------------------------------|-----------------------------------------------------------------|
| Glyph   | yes      | column 1, one cell, one blank cell each side | never cut                                                       |
| Text    | yes      | after the Glyph's trailing blank            | cut at the right edge, no marker                                |
| Pointer | fixed    | three cells after Text, `(e: full log)`     | pushed off the row by a long Text; it goes before Text is cut   |

When no event qualifies the row is blank.

## Sizing
Height 1. Width is the area width, the full terminal width today. No
parameters.

## Variants
| Variant  | What changes                                            | Used by                        |
|----------|---------------------------------------------------------|--------------------------------|
| By kind  | Glyph and its colour follow the event kind, table below | every event                    |
| Empty    | blank row                                               | a fresh world with no events   |
| Filtered | Birth and Mutation events are skipped unless `log_births` is on, so the row shows the latest event of another kind | S10 option `[b]` off |

Glyph and colour per `EventKind`, from `ui::style::EventKindStyle`:

| Kind                                | Glyph               | Colour           |
|-------------------------------------|---------------------|------------------|
| Birth                               | `glyphs::BIRTH` `♥` | `theme::GOOD`    |
| Recovery                            | `glyphs::IMMUNE` `☺` | `theme::GOOD`   |
| DeathStarved, DeathThirst           | `glyphs::DEATH` `x` | `theme::WARN`    |
| DeathPredation                      | `glyphs::DEATH` `x` | `theme::BAD`     |
| DeathAge                            | `glyphs::DEATH` `x` | `theme::DIM`     |
| DeathDisease, Outbreak, Epidemic    | `glyphs::DISEASE` `☻` | `theme::SICK`  |
| EpidemicOver                        | `glyphs::DISEASE` `☻` | `theme::DIM`   |
| Spillover                           | `glyphs::DISEASE` `☻` | `theme::MAGENTA` |
| Mutation                            | `glyphs::MUTATION` `§` | `theme::INFO` |
| Migration                           | `glyphs::MIGRATION` `→` | `theme::ACCENT` |
| Extinction                          | `glyphs::EXTINCTION` `‼` | `theme::MAGENTA` |
| Drought                             | `glyphs::DROUGHT` `¡` | `theme::WARN`  |
| DroughtEased                        | `glyphs::DROUGHT` `¡` | `theme::DIM`   |
| Season                              | `glyphs::SUMMER` `☼` | `theme::TITLE`  |
| Wary                                | `glyphs::ALERT` `!` | `theme::WARN`    |
| Note                                | `glyphs::NOTE` `¶`  | `theme::TEXT`    |

## Interaction
None. Display only. The keys of the screen around it are in [keys.md](keys.md).

## Styling
- Row filled with `theme::BG`, not `theme::PANEL_BG`: the ticker sits on the
  screen ground between two panels.
- Glyph: the kind's colour from the table, bold, on `theme::BG`.
- Text: `theme::TEXT` on `theme::BG`.
- Pointer: `theme::DIM` on `theme::BG`.

## Glyphs
Every event glyph in the table above: `glyphs::BIRTH`, `glyphs::DEATH`,
`glyphs::MUTATION`, `glyphs::MIGRATION`, `glyphs::EXTINCTION`,
`glyphs::DROUGHT`, `glyphs::ALERT`, `glyphs::NOTE`, `glyphs::DISEASE`,
`glyphs::IMMUNE`, `glyphs::SUMMER`. Text may carry species letters from the
roster. All CP437.

## Composition
*Contains:* nothing.
*Contained by:* the S01 layout, row `map_rows` under the map panel, above the
Status Bar.

## API
### Today
```rust
ui::screens::s01_map::draw_ticker(f, area: Rect, map_rows: u16, sim: &Sim, app: &AppState)   // private
ui::style::EventKindStyle::glyph(&self) -> char      // on sim::EventKind
ui::style::EventKindStyle::color(&self) -> Color
```
`draw_ticker` picks the event itself: the newest entry in `sim.events` that is
not a Birth or Mutation unless `app.params.ui.log_births` is set.

### Planned
```rust
Ticker::new(event)                       // Option<&Event>: kind and text; None draws the Empty row
    .pointer("(e: full log)")            // default
    .render(buf, row_area)
```
`height` is 1; `min_width` is 3 (Glyph and its blanks). The Birth and Mutation
filter stays in the caller.

## Gaps today
- Private to S01 and chooses the event itself; the spec takes the event as a
  parameter.
- No cut marker on a long Text, and the Pointer is lost first.
- The Season kind uses `glyphs::SUMMER` for every season although
  `glyphs::SPRING`, `glyphs::AUTUMN` and `glyphs::WINTER` exist.

## Examples

### Note, from S01a row 44 (155 columns)
```
 ¶ Ashfang w#042 is stalking Bramble h#217   (e: full log)                                                                                                 
```

### Death by predation (155 columns)
The S07a predation event as the latest event; Glyph in `theme::BAD`.
```
 x Thistle d#133 was killed by Ashfang w#042 in the Fenlands   (e: full log)                                                                               
```

### Cut at width, Pointer lost (43 columns)
A birth text longer than the row; Glyph in `theme::GOOD`.
```
 ♥ Clover v#031 gave birth to 4 young in Re
```

### Empty (43 columns)
Forty-three blank cells on `theme::BG`.
```
                                           
```

## Open questions
- Should the Pointer be a [Key Hint](key-hint.md) `[e] full log` instead of
  the prose form `(e: full log)`?
- Should a cut Text end with `glyphs::DOT` `·` as `common::clip` does for
  other text?
- Should the row take `theme::PANEL_BG` like every other row component, or is
  the ground colour deliberate?
