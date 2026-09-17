# Sim Fortress — UI Components

Spec sheets for the reusable pieces every screen is built from. One file per
component, all in the format of [_template.md](_template.md). The
[screen requirements](../screens/README.md) say *what* each screen shows; these
sheets say *how* the shared pieces look and behave, so a screen file can say
"a Labeled Bar" and stop there.

**Status: implementing.** The trait, the stacks, Text, Panel and Divider have
shipped in `src/widgets`; `tests/components.rs` pins their examples and lists
the sheets still pending. Each sheet records the helper that draws the thing
*today* (`src/widgets/*.rs`, or an ad-hoc loop inside a screen) and the
*planned* first-class API. Where today's behaviour differs from the spec, the
sheet says so under **Gaps today**. The spec wins; the code moves toward it.

## Index

Widths in the *Shape* column are the canonical example width (see conventions).

| Component | File | Shape | Today | Planned |
|---|---|---|---|---|
| **Standards** | | | | |
| Keys | [keys.md](keys.md) | n/a | `Screen::handle_key` per screen | screens map keys to state with this table |
| **Layout** | | | | |
| VStack | [vstack.md](vstack.md) | area | `row` counters in every screen | `VStack` |
| HStack | [hstack.md](hstack.md) | area | `Rect::new` splits in every screen | `HStack` |
| Spacer | [spacer.md](spacer.md) | rows or cols | `row += 1` | `Spacer` |
| Columns | [columns.md](columns.md) | area | matching literal widths per row | `Columns`, `Block` |
| **Containers** | | | | |
| Panel | [panel.md](panel.md) | area | `panel::draw`, `draw_with_hint`, `Kind` | `Panel` |
| Divider | [divider.md](divider.md) | row | `panel::section` | `Divider` |
| Modal | [modal.md](modal.md) | area | `util::centered` + `util::dim_area` + `panel::draw(.., Kind::Focus)` | `Modal` |
| Scroll Region | [scroll-region.md](scroll-region.md) | area | `scroll::draw`, `Overflow` | `ScrollRegion` |
| **Rows and indicators** | | | | |
| Labeled Bar | [labeled-bar.md](labeled-bar.md) | row | `bars::labeled`, `bars::bar`, `bars::vital_color` | `LabeledBar`, `Bar` |
| Range Bar | [range-bar.md](range-bar.md) | row | `bars::range` | `RangeBar` |
| Sparkline | [sparkline.md](sparkline.md) | row | `bars::sparkline` | `Sparkline` |
| Trend Arrow | [trend-arrow.md](trend-arrow.md) | cell | `ui::screens::common::trend_arrow`, `arrow_color` | `TrendArrow` |
| Key Hint | [key-hint.md](key-hint.md) | inline | inline spans in `status::render` and screens | `KeyHint` |
| Text | [text.md](text.md) | row | `util::line`, `set_stringn` | `Text` |
| **Chrome** | | | | |
| Status Bar | [status-bar.md](status-bar.md) | row | `status::render`, `render_colored` | `StatusBar` |
| Ticker | [ticker.md](ticker.md) | row | `s01_map::draw_ticker` | `Ticker` |
| **Data** | | | | |
| Table | [table.md](table.md) | area | hand-laid columns in S04, S06, S07 | `Table` |
| Histogram | [histogram.md](histogram.md) | area | `bars::histogram` + S04b axis rows | `Histogram` |
| Chart | [chart.md](chart.md) | area | hand-drawn `▀▄` line charts in `s05_charts/{time,infections,phase,stacked,groups}.rs` | `Chart`, `StackedChart` |
| Legend | [legend.md](legend.md) | area | `map::legend()` + `s01_map::base::legend_section` | `Legend` |
| **Input** | | | | |
| Menu | [menu.md](menu.md) | area | loop in S00 title, `load_world` | `Menu` |
| Stepper | [stepper.md](stepper.md) | row | loop in S09 worldgen | `Stepper` |
| Text Field | [text-field.md](text-field.md) | row | loop in S09 worldgen | `TextField` |
| Checkbox | [checkbox.md](checkbox.md) | row | loop in S10 controls | `Checkbox` |
| Button Row | [button-row.md](button-row.md) | row | loops in S09, S12, `confirm` | `ButtonRow` |
| Filter Strip | [filter-strip.md](filter-strip.md) | row | loop in S07 log | `FilterStrip` |

## Conventions shared by every sheet

- **Slot notation.** Templates write slots as `<Name>`. Whether a slot is
  required, where it aligns, and what happens when there is no room is stated in
  the sheet's Slots table, never by punctuation in the template.
- **Canonical width.** Examples are 43 columns wide, the width of the S01 status
  sidebar, so they can be copied straight from
  [docs/screens/renders](../screens/renders). A component that is inherently
  wider (Table, Chart, Status Bar) states its example width instead. Row
  components shown inside a panel are drawn *with* the panel border so the
  reader sees the real spacing; the inner width is then 41.
- **Pixel-exact examples.** Every example must be something the component can
  actually render, cell for cell. They are the fixtures for the pinned-render
  tests described below, so an example that cannot be reproduced is a bug in the
  doc.
- **Colour comes from `src/theme.rs`.** Sheets name the theme item, never an RGB
  value. Species colours are the one exception and come from the roster.
- **Glyphs come from `src/glyphs.rs`.** Sheets list the constants they use. A
  glyph a sheet needs that has no constant yet is called out under Open
  questions so it gets added, and stays CP437 and one cell wide.
- **Keys are standard.** Every interactive sheet has an Interaction section
  that lists its keys against [keys.md](keys.md): `Enter` and `Space` select,
  `Esc` closes, `q` exits, `←` `→` step values, `↑` `↓` move between rows,
  `Tab` moves between fields and panels. Deviations in the live code are
  collected in that sheet.
- **Two APIs.** *Today* is the helper that draws the thing now, with its exact
  path. *Planned* is the first-class API below. Screens keep calling *Today*
  until the sheet's component ships.
- **Prototype renders are not authoritative.** Some renders under
  `docs/screens/renders` predate the live code (for example the event log's
  `35 more ↓` foot versus the live `↓35`). The sheet describes the live shape and
  notes the prototype variant only when it is worth keeping.

## The planned system

### Goals

1. **Draw once, use everywhere.** A bar in the inspector and a bar in the
   ecology table are the same code with the same spacing rules.
2. **Buffer-based.** Every component renders into a `Buffer`, not a `Frame`, so
   it works on screen and inside the off-screen canvas that
   [Scroll Region](scroll-region.md) uses.
3. **Sized before drawn.** Stacking sections in a sidebar needs to know how tall
   each one is. Components report their height for a given width.
4. **Testable.** Each sheet's examples become pinned renders.

### API shape

```rust
/// Implemented by every component in `src/widgets`.
pub trait Component {
    /// Rows needed at `width`. Row components return 1.
    fn height(&self, width: u16) -> u16;
    /// Narrowest width that still shows every required slot.
    fn min_width(&self) -> u16;
    /// Draw into `area`. Never writes outside it.
    fn render(&self, buf: &mut Buffer, area: Rect);
}
```

- Components are plain structs with builder methods:
  `LabeledBar::new("vegetation", 0.63).color(theme::VEGETATION).label_w(12).bar_w(20)`.
- Containers return the area they leave for content:
  `Panel::new("Status").kind(Kind::Outer).render(buf, area) -> Rect`.
- Layout is two components, [VStack](vstack.md) and [HStack](hstack.md), with
  three constraints between them: `Auto` (ask the child), `Fixed(n)` and
  `Fill(weight)`. They replace today's hand-threaded `row` counters and
  `Rect::new` splits. Scroll Region takes a VStack as its body. Rows that
  must line up across a block use [Columns](columns.md), a spec resolved once
  for the block; it adds `Min(n)`, measured over every row before drawing. ratatui's
  `Layout` is not used, because it cannot ask a child for its height.
- Row components take their whole row and lay slots out inside it. Callers
  position with `Rect`, not with `x`/`y` pairs.
- Free functions in `src/widgets` stay as thin wrappers until every screen has
  moved, then go.

### Migration

1. **Docs** (this folder). Agree the sheets; fix examples against the live app.
2. **Builders beside helpers.** Add each struct next to its helper; the helper
   calls the struct. Pinned-render tests land with the struct.
3. **Screens move section by section.** Start with the S01 sidebar as a
   VStack of Divider, Labeled Bar, Sparkline, Trend Arrow and Legend, then the
   inspector's three columns as an HStack, then the inputs in S09 and S10.
4. **Delete the helpers** once `grep` finds no callers.

### Testing

- `tests/components.rs` renders every example in every sheet at its stated
  width into a `Buffer` and compares cell for cell. The example blocks are the
  fixtures; `tests/components/registry.rs` maps each heading to the builder
  calls that draw it. The test names the sheet and example heading on failure,
  checks that nothing is written outside the area, and fails when a heading
  is neither registered nor listed as unreproducible (`PENDING_SHEETS` covers
  components that have not shipped yet).
- The CP437 and file-size invariants in `tests/` already cover glyphs and
  module size; no change.

## Open questions

- Layout candidates not yet written up: `Pad` (inset a child by n cells, as
  modal bodies indent by 2), and `Align` (centre or right-align a child narrower
  than its area, as modal hints and the title screen do). Grids are the
  [Columns](columns.md) Grid variant. Each is a VStack or HStack special case; write a sheet only when a second
  screen needs it.
- Box-drawing glyphs (`═ ║ ╔ ─ │ ┌`) come from ratatui's `BorderType` today and
  are not listed in `src/glyphs.rs`. Should they be, so the CP437 test covers
  them?
- Should sheets carry an *ASCII-fallback* variant for terminals without CP437
  glyphs, or is that out of scope for a truecolor, CP437-only app? Current
  answer: out of scope, see the UI decisions in the repo memory.
- Should Panel own the Divider variant that spans its border columns, or is it
  purely a Divider concern? Decided: it is a Divider variant, see
  [divider.md](divider.md).
