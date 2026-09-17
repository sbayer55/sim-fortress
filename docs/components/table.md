# Table

Back to the [component index](README.md).

Rows of aligned columns under a dim header row. One row may be selected: it
takes `theme::selected()` and a `►` in column 0. An optional totals row closes
the table, and the sort state is shown in the enclosing Panel's Info slot. The
species table in [S04](../screens/s04-species-browser.md), the regions table in
[S06](../screens/s06-ecology.md), the event list in
[S07](../screens/s07-event-log.md) and the save list in the Load World modal of
[S00](../screens/s00-title.md) are Tables.

## Anatomy
A Table is inherently wide. The anatomy is shown at 60 columns inside the
[Panel](panel.md) that always contains it; the border, Title, Sort and Foot
belong to the Panel.

```
╔ <Title> ═════════════════════════════════════════ <Sort> ╗
║ <Header>                                                 ║
║►<Cell>   <Cell> <Cell>  <Cell>   <Cell>   <Cell>  <Cell> ║
║ <Cell>   <Cell> <Cell>  <Cell>   <Cell>   <Cell>  <Cell> ║
║ <Totals>                                                 ║
╚══════════════════════════════════════════════════ <Foot> ╝
```

Column 0 is the Marker column, one cell wide. Every other column has a fixed
`width` and an alignment. Numbers are right-aligned and carry their gap as a
leading blank; text is left-aligned and carries its gap as a trailing blank.

## Slots
| Slot   | Required | Position                                                                 | Overflow                                                            |
|--------|----------|--------------------------------------------------------------------------|---------------------------------------------------------------------|
| Sort   | no       | the Panel Info slot, as `sorted by <column> ↓`                           | Panel rule: dropped before the Title is cut                         |
| Header | yes      | row 0, one title per column, aligned like that column's cells            | columns that do not fit are dropped from the right, cells with them |
| Marker | no       | column 0 of every row: `►` on the selected row, blank elsewhere          | never dropped, it is one cell                                       |
| Cell   | yes      | column `i` of a row, padded or cut to the column width                   | cut at the column width, no marker; whole columns drop before cells are cut |
| Totals | no       | last row; label in the first text column, values under their columns     | cut at the right edge like any row                                  |
| Foot   | no       | Panel bottom edge, `↑n ↓m`, written by [Scroll Region](scroll-region.md) | Panel rule                                                          |

A Cell is plain text by default. It may instead be a Bare [Bar](labeled-bar.md),
a [Sparkline](sparkline.md), a [Trend Arrow](trend-arrow.md), or a species glyph
in the roster colour. Those cells take the column width like any other.

## Sizing
Width is the area width. Columns are laid left to right from column 1 and each
takes exactly its `width`; the last column may be `Fill` and takes the rest.
Height is `1 + rows + 1 if totals`; the Spaced variant adds a blank row after
the Header and before Totals. Minimum width is `1 + width of the first column`.
Rows that do not fit the area are not drawn; a screen that can have more rows
than height wraps the Table in a [Scroll Region](scroll-region.md). The
component never writes outside its area.

## Variants
| Variant        | What changes                                                                        | Used by                          |
|----------------|-------------------------------------------------------------------------------------|----------------------------------|
| Plain          | Header row, then rows                                                               | S06 regions, S07 log, Load World |
| Spaced         | one blank row after the Header and one before Totals                                | S04 species                      |
| With totals    | Totals row, label in `theme::label()`                                               | S04 species                      |
| Selectable     | Marker column and `theme::selected()` on one row                                    | S04, S06, S07, Load World        |
| Sorted         | Sort text in the Panel Info slot                                                    | S04, S06                         |
| Absent row     | a row whose subject is gone (count 0) drawn in `theme::dim_text()`                  | S04 extinct species              |
| Coloured cells | a cell with its own foreground: births `theme::GOOD`, deaths `theme::BAD`, sick `theme::SICK`, prey `theme::PREY`, predators `theme::PRED`, water `theme::SHALLOW_FG`, traits via `common::trait_color` | S04, S06 |
| Embedded cells | Bare Bar, Sparkline, Trend Arrow or species glyph inside a cell                     | S04 trend, S06 vegetation and moisture, S04b habitat |

## Interaction
| Key           | Standard              | Here                                                    |
|---------------|-----------------------|---------------------------------------------------------|
| `↑` `↓`       | move the selection    | move the `►` row; clamped at the ends                   |
| `PgUp` `PgDn` | page                  | move the selection a page                               |
| `Home` `End`  | first / last          | select the first / last row                             |
| `Enter`       | select                | open the selected row (S04 detail, S07 jump, S06 region)|
| `Tab`         | next panel            | move focus to the next pane on multi-pane screens       |
| letter        | shown in a Key Hint   | `s` / `r` cycle the sort; `f` cycles filters on S07     |

See [keys.md](keys.md). S03 and S04 switch panes with `←` `→` and S04 detail
changes species with `↑` `↓`; both are deviations listed there.

## Styling
- Header: `theme::dim_text()`. Nothing else is background-filled; `theme::HEADER_BG`
  is the app header bar, not a table header.
- Cells: `theme::text()`. Secondary text cells (S04 Kind and Diet) use
  `theme::dim_text()`. Absent rows use `theme::dim_text()` for every cell.
- Selected row: every cell across the inner width is filled with
  `theme::SELECT_BG`; text cells take `theme::selected()`. Coloured cells keep
  their foreground and take `theme::SELECT_BG`.
- Marker: `glyphs::PLAY` in `theme::KEY`, bold, on `theme::SELECT_BG`.
- Totals: label `theme::label()`, values `theme::text()`, notes
  `theme::dim_text()`; births and deaths in `theme::GOOD` and `theme::BAD`.
- Embedded cells keep their own rules: the Bar per [labeled-bar.md](labeled-bar.md),
  the Sparkline in the species colour (`theme::DIM` when absent), the Trend
  Arrow in `common::arrow_color` (`theme::GOOD`, `theme::BAD`, `theme::DIM`), bold.
- Species colours and glyphs come from the roster, not from `theme`.

## Glyphs
`glyphs::PLAY` `►` for the Marker and `glyphs::DOWN` `↓` in the Sort text.
Embedded cells bring their own: `glyphs::BAR_L` `BAR_R` `BAR_FILL` `BAR_EMPTY`
(Bar), `glyphs::SHADES` `░▒▓█` (Sparkline), `glyphs::UP` `DOWN` `FLAT`
(Trend Arrow). The border is the Panel's, from ratatui.

## Composition
Each row is an [HStack](hstack.md) of cells drawn against one
[Columns](columns.md) spec; the rows form a [VStack](vstack.md). A Table is
the Headed variant of Columns with a Marker column and a selected row.
*Contains:* [Bar](labeled-bar.md) (Bare, Marker), [Sparkline](sparkline.md),
[Trend Arrow](trend-arrow.md), species glyph cells.
*Contained by:* [Panel](panel.md) always; [Scroll Region](scroll-region.md)
when the rows can exceed the height; [Modal](modal.md) for Load World. A
[Filter Strip](filter-strip.md) may sit above the Header (S07) but is a separate
row owned by the screen.

## API
### Today
No shared helper; each screen lays its columns by hand with `set_stringn` or
`Line` spans through `util::line`.
```rust
// src/ui/screens/s04_species/table.rs
pub(super) fn table(f: &mut Frame<'_>, area: Rect, sim: &Sim, sort: SortCol, selected: SpeciesId, kind: panel::Kind)
fn table_row(f: &mut Frame<'_>, inner: Rect, row: u16, sim: &Sim, i: usize, selected: SpeciesId)
fn table_totals(f: &mut Frame<'_>, inner: Rect, mut row: u16, sim: &Sim)
// src/ui/screens/s06_ecology.rs
fn regions(f: &mut Frame<'_>, area: Rect, app: &AppState, world: &World, time: &crate::sim::Time, screen: &Ecology)
fn region_row(buf: &mut ratatui::buffer::Buffer, inner: Rect, y: u16, ri: usize, r: &(String, usize, usize, usize, usize), world: &World, app: &AppState, selected: bool, th: &ScarcityThresholds, prey_configured: bool)
// src/ui/screens/s07_log.rs
fn list(&self, f: &mut Frame<'_>, area: Rect, sim: &Sim, events: &[&Event], compact: bool)
// src/ui/screens/load_world.rs: the save rows inside `Screen::render`
fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect)
```
Embedded cells use `widgets::bars::sparkline(buf, x, y, w, values, color)`,
`widgets::bars::bar(buf, x, y, w, value, color)`,
`ui::screens::common::trend_arrow(counts) -> char` and
`ui::screens::common::arrow_color(a) -> Color`. Sort labels come from
`s04_species::SortCol::label()`.

### Planned
```rust
pub struct Column { pub title: &'static str, pub width: Constraint, pub align: Align }  // a columns.md entry

pub enum TableCell<'a> {
    Text(Cow<'a, str>),              // row style
    Styled(Cow<'a, str>, Color),     // own foreground, keeps the row background
    Dim(Cow<'a, str>),               // secondary text, theme::dim_text()
    Glyph(char, Color),              // a species glyph, bold in the roster colour
    Widget(Box<dyn Component + 'a>), // a Bare Bar, Sparkline, Trend Arrow, or an HStack of them
    Blank,
}
pub struct TableRow<'a> { pub cells: Vec<TableCell<'a>>, pub absent: bool, .. }
TableRow::new(cells).absent(true).tail(Text::new(" prey 368  pred 24"))   // tail spans the columns after the cells

/// Rows on demand, so a screen never builds every row to draw a window.
pub trait RowSource { fn len(&self) -> usize; fn with_row(&self, i: usize, f: &mut dyn FnMut(&TableRow<'_>)); }

Table::new(&COLUMNS, &rows)          // Columns, see columns.md; column 0 (Marker) is implicit; rows are a slice
Table::from_source(&COLUMNS, &source) // or any RowSource, built on demand
    .selected(Some(0))               // Marker and theme::selected() on that row
    .totals(TableRow { .. })         // optional; its first text cell is the label
    .spaced(true)                    // default false
    .render(buf, area)
Table::sort_info("count") -> String  // "sorted by count ↓", for Panel::info
```
`height(width)` is `1 + rows`, plus one blank row after the header when
spaced, plus the totals row and one blank before it when both are set;
`min_width` is `1 + COLUMNS[0].width`. Columns past the width are dropped
from the right. When `height` exceeds the area the screen wraps the Table in
a Scroll Region, which writes `↑n ↓m` into the Panel Foot.

## Gaps today
- No shared column spec; every screen hand-lays its columns.
- Marker column: S04 and S06 put `►` in inner column 0; S07 and Load World put
  it in column 1 after a blank cell. S06 draws it in the text colour as part of
  the name string, S07 in `theme::TEXT_BRIGHT`; the spec says `theme::KEY`.
- Sort: S04 writes `sorted by count ↓` into the status bar right text and its
  Panel Info shows counts (`6 species, 3 prey / 3 predator`). S06's Info is the
  fixed string `sorted by name   [r] cycle sort` and does not follow `[r]`.
  Load World's Info shows the save count and the selected save's timestamp.
- S04's totals value sits one cell right of the Count column: the label is
  `   {:<14}` (17 cells) while Count starts at cell 16.
- S04's totals row is one free-text line, not cells under the columns.
- S07 draws only the first `area.height` events (`take`) with no Foot. The
  prototype render S07a shows a `█░` scrollbar column and `35 more ↓` in the
  Foot; neither is live, but the live code still leaves the last inner column
  blank for that scrollbar and fills the selected row's background across all
  but that column. S07 is the only screen that ends a cut text cell with `~`.
- No Table is wrapped in a Scroll Region: S04 uses a fixed `TABLE_H`, S06 has
  eight regions that always fit, S07 clips.
- The S06a render lays the regions out differently from the live code (18-cell
  bars, no Sick column, a pressure bar). The S06 example below follows the
  render.
- Keys: S03 and S04 switch panes with `←` `→` and S04 detail changes species
  with `↑` `↓`; the standard reserves `Tab` for panes and `←` `→` for stepping.
  See [keys.md](keys.md).

## Examples

### Species table cut after Gen, Spaced with totals (70 columns)
Rows from S04a; the columns from `30-day trend` on are dropped. The Hare row is
selected, so it carries the Marker and `theme::selected()`. The Info slot shows
the Sort text the spec asks for, and the totals value sits under the Count
column (S04a draws it one cell right; see Gaps today).
```
╔ Species ════════════════════════════════════════ sorted by count ↓ ╗
║   Species Kind  Count Adults   Juv Birth/d Death/d  Sick  Peak  Gen║
║                                                                    ║
║►H Hare    prey    201    201     0       0       1     0   703    4║
║ V Vole    prey    154    154     0       0       2     0   544    6║
║ D Deer    prey     13     11     2       0       0     0   230    2║
║ F Fox     pred     10     10     0       0       0     0    11    4║
║ W Wolf    pred      8      7     1       0       0     0    18    3║
║ L Lynx    pred      6      6     0       0       0     0     7    3║
║                                                                    ║
║   totals          392    prey 368  pred 24  ratio 15.3:1   births 0║
╚════════════════════════════════════════════════════════════════════╝
```

### Full width, header, selected row and totals (155 columns)
The whole S04a row: a species glyph cell in the roster colour, a Sparkline in
the 18-cell trend column (14 cells wide, starting at cell 71), a Trend Arrow,
eleven trait cells via `trait_color`, and a dim Diet cell.
```
╔ Species ═════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════ sorted by count ↓ ╗
║   Species Kind  Count Adults   Juv Birth/d Death/d  Sick  Peak  Gen  30-day trend        Spd Siz Sen Met Agg Cam Fer Lon Res Soc Mat    Diet            ║
║                                                                                                                                                         ║
║►H Hare    prey    201    201     0       0       1     0   703    4   ███▓▓▓▓▓▓▒▒▒░░ ↓    82  24  65  61  10  56  75  35  35  26  51   grass, bark      ║
║                                                                                                                                                         ║
║   totals          392    prey 368  pred 24  ratio 15.3:1   births 0  deaths 0   net +0 today     traits = species means x100;  /d = yesterday           ║
╚═════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════╝
```

### Marker and Bare bars, S06 regions (63 columns)
Plain variant, from the S06a render (prototype layout, see Gaps today). A text
column, two right-aligned number columns, then a Bare Bar cell with its value.
Ashen Ridge is selected.
```
║ region            cells   water     vegetation              ║
║ Northmarch          700     18%   [██████░░░░░░░░░░░░] 0.32 ║
║►Ashen Ridge         600     12%   [█████████░░░░░░░░░] 0.49 ║
```

## Open questions
- Where do the counts that S04 and Load World show in the Info slot go once
  Sort takes it: the Title (`Species (6)`) or the Foot?
- Should an ascending sort show `↑`? S04 always writes `↓`, even for name.
- Should a text cell cut at its column width end with `~` (S07) or be cut
  silently (S04, S06)? [Divider](divider.md) has the same question.
- Should coloured cells on the selected row keep their colour on
  `theme::SELECT_BG` or turn `theme::TEXT_BRIGHT` like the rest of the row?
- Are rows always one row high? The two-row outbreak entries in the S05d
  sidebar are a list, not a Table, under this spec.
