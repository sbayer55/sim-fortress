# Sim Fortress prototype guide (for contributors and subagents)

Static UI prototypes for a Dwarf-Fortress-style predator/prey/evolution sim.
Rust, ratatui 0.30 (crossterm re-exported as `ratatui::crossterm`). No simulation
logic: every screen draws fixed fixture data.

## Hard rules
- Fixed frame **155x45**. Row 0 is the header (drawn by `widgets::header`). Every
  prototype's `render(f, area)` receives `area` = 155x44 (x=0, y=1).
- **CP437 glyphs only.** Use constants from `src/glyphs.rs` (add new ones there if
  needed; `cargo test` verifies they are CP437 and one cell wide). Never use braille,
  eighth-blocks `▁▂▃`, `❄ † ‖ ☾ ✓ ✗ •`-style extras... (`•` IS in CP437). If unsure, call
  `glyphs::is_cp437(c)` in a test or check the `CP437` table string in glyphs.rs.
  ratatui `Chart`/`Canvas` must use `Marker::HalfBlock`, `Marker::Block`, or `Marker::Dot` —
  never `Marker::Braille`.
- Colors: always `Color::Rgb` via `src/theme.rs` (palette + `heat/veg/water` ramps,
  `lerp`, `dim`, `night`). Styles: `theme::text()`, `dim_text()`, `title()`, `key()`,
  `label()`, `border()`, `selected()`.
- Only edit the files you own. Do not modify `theme.rs`, `glyphs.rs`, `widgets/*`,
  `fixtures/*`, `app.rs`, `main.rs`, or another screen's file. If you need a helper,
  put it in your own file. If you truly need a fixture field that does not exist,
  derive it locally from what exists.
- Keep `cargo build` warning-free for *your* files (unused imports etc.).

## Layout conventions (see `src/prototypes/s01_map.rs` as the reference)
- Bottom row (`area.y + area.height - 1`) is the status bar:
  `widgets::status::render(f, rect, &[("k","label"),...], "right text")`.
- Panels: `widgets::panel::draw(f, rect, "Title", panel::Kind::Outer|Inner|Focus) -> inner Rect`
  (double-line border for Outer/Focus, single for Inner; it clears the area first).
  `panel::draw_with_hint(..., "right hint", kind)` adds a dim right-aligned title.
  `panel::section(f, inner, row, "Section")` draws a `── Section ────` divider row.
- Text: `widgets::util::line(f, inner, row, Line::from(vec![Span::styled(..)]))`;
  `util::fill`, `util::centered(area, w, h)`, `util::dim_area(buf, area, 0.6)` (modal backdrop).
- Bars: `widgets::bars::labeled(buf, area, row, "label", value01, color, label_w, bar_w)`,
  `bars::bar`, `bars::range(min, mean, max)`, `bars::histogram(buf, area, &[u16], color, col_w)`,
  `bars::sparkline(buf, x, y, w, &[u16], color)`, `bars::vital_color(v, inverted)`.
- Map: `widgets::map::render(buf, rect, fixtures, &MapOptions{..})` with
  `MapOptions { overlay: Overlay::None|Vegetation|Pressure|Moisture|Sense(creature_idx),
  night, winter, cursor: Option<(x,y)>, follow: Option<idx>, origin: (ox,oy), creatures, fade_creatures }`.
  `prototypes::s01_map::map_panel(f, rect, "Title", &opts) -> inner` draws the bordered
  map panel with a scroll hint; `s01_map::render_base(f, area)` draws the whole S01a
  screen (use it as the backdrop for modals, then `util::dim_area` and draw on top).
  `s01_map::ORIGIN` is the default viewport origin (20,0); world is 150x40.
  `map::terrain_cell(cell, winter) -> (char, fg, bg)` and `map::legend()` are public.

## Fixtures (`crate::fixtures::get() -> &'static Fixtures`)
- `world: World` — `cell(x,y) -> &Cell { terrain: Terrain, elevation, moisture, vegetation,
  prey_pressure, pred_pressure }`, `dens`, `carcasses`, `seeds: Vec<(x,y)>`,
  `regions: Vec<(name, x0,y0,x1,y1)>`, `region_name(x,y)`, `width()`, `height()`.
- `creatures: Vec<Creature>` — `id, name, species: SpeciesId, x, y, adult, sex, alive,
  age_days, max_age_days, hp, hunger, thirst, energy (all 0..1), genome: Genome([f32;8]),
  generation, goal, target, kills, offspring, mutations: Vec<String>, parents: (String,String),
  trail, cause_of_death: Option<String>, decay`. `glyph()`, `tag()` ("h#217"), `kind()`.
  `fx.hero_prey` (Bramble the hare), `fx.hero_pred` (Ashfang the wolf), `fx.corpse`
  (Thistle the deer) are indices into `creatures`.
- `Genome` accessors: `speed() size() sense() metabolism() aggression() camouflage()
  fertility() longevity()`, `sense_cells()`. `fixtures::TRAIT_NAMES: [&str; 8]`.
- `species: Vec<Species>` (order = `SpeciesId::ALL`) — `id, count, adults, juveniles,
  births_today, deaths_today, peak, generation, trend: Vec<u16> (30 days), mean/min/max: Genome,
  hist: [[u16;12];8], drift: Vec<Genome> (12 generations)`, `trend_arrow()`.
  `SpeciesId::{name, plural, glyph, color, kind (Kind::Prey|Predator), diet, base_genome}`.
- `series: Series` — `population: Vec<Vec<f32>>` (6 species x 240 days), `vegetation`,
  `water`, `carcasses: Vec<f32>` (0..1 or counts), `pop(id)`, `prey_total()`, `pred_total()`, `day0`.
- `events: Vec<Event>` — `year, day, hour, kind: EventKind, species: Option<SpeciesId>,
  text, pos: Option<(x,y)>, detail`. `EventKind::{glyph, color, label, is_death}`.
- `lineage: Lineage { nodes: Vec<Node{name, tag, generation, born_year, died_year,
  mutations, children: Vec<usize>, notable}>, root, focus }`.
- `clock: Clock { year, day, season: Season, hour, tick, speed, paused }`, `clock.label()`,
  `Season::{name, short, glyph, color}`.

## Registering a screen
Each `src/prototypes/sNN_*.rs` exposes `pub fn all() -> Vec<Box<dyn Prototype>>`.
Implement `Prototype { id(), name(), variant(), render(&self, f, area) }`. Ids/names/variants
must match the placeholder list already in the file (keep the same strings).

## Verifying
```
cargo build && cargo test
```
Then use the `tui-mcp` tools: `launch` command `./target/debug/sim-fortress S03b`
(cwd = repo root, cols 155, rows 45), `screenshot` to look at it, `send_keys "]"` to step
to the next prototype, `kill` when done. Check: nothing overflows or wraps, every panel
is filled, the status bar is on the last row, glyphs render (no boxes).
