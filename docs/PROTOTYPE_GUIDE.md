# Sim Fortress UI guide (for contributors and subagents)

Rust, ratatui 0.30 (crossterm re-exported as `ratatui::crossterm`).

> **The filename is historical.** This doc used to describe a static-prototype
> layout (`src/prototypes/`, a `fixtures` module, a `Prototype` trait). None of
> that exists any more: the UI renders a live simulation. The content below
> describes the current tree; the glyph, palette and widget conventions carried
> over unchanged.

## Architecture

- `src/sim/` — the pure, deterministic simulation core. **No `ratatui` and no
  `HashMap`/`HashSet` may appear anywhere under it** (enforced by
  `no_ratatui_in_sim` / `no_hashmap_in_sim` in `src/sim/mod.rs`).
- `src/ui/` — the ratatui layer. `ui::app::App` owns a `Stack` of
  `Box<dyn Screen>` and an `AppState` (`sim: Option<Sim>`, `params`, viewport
  origin, …). `App::draw` fills the background with `theme::BG`, then calls
  `screens::render_stack`.
- `src/widgets/` — reusable drawing helpers (`panel`, `status`, `bars`, `util`,
  `map`). `src/theme.rs` — palette and styles. `src/glyphs.rs` — the CP437 table.

## Hard rules

- **CP437 glyphs only.** Use constants from `src/glyphs.rs` (add new ones there if
  needed; `cargo test` verifies they are CP437 and one cell wide). Never use
  braille, eighth-blocks `▁▂▃`, or `❄ † ‖ ☾ ✓ ✗ •`-style extras (`•` *is* in
  CP437). If unsure, call `glyphs::is_cp437(c)` in a test or check the `CP437`
  table string. ratatui `Chart`/`Canvas` must use `Marker::HalfBlock`,
  `Marker::Block` or `Marker::Dot` — never `Marker::Braille`.
- **Colors always come from `src/theme.rs`.** Palette plus the ramps `heat`,
  `veg`, `water`, `parasite`, `species_ramp`, and `lerp` / `dim` / `night`.
  Styles: `theme::text()`, `dim_text()`, `title()`, `key()`, `label()`,
  `border()`, `border_focus()`, `selected()`.
- There is no longer a fixed frame. A screen's `render` receives whatever `area`
  it is given (the full terminal from `App::draw`); tests use a 155×45
  `TestBackend`.
- Keep `cargo clippy --all-targets` at **zero warnings** (pedantic + nursery are
  on, and several lints are denied — see `Cargo.toml`).

## Layout conventions

- Status bar (usually the last row): `widgets::status::render(f, rect, &[("k","label"),…], "right text")`.
- Panels: `panel::draw(f, area, "Title", panel::Kind::Outer|Inner|Focus) -> Rect`
  returns the inner area (double border for `Outer`/`Focus`, single for `Inner`;
  it clears the area first). `panel::draw_with_hint(…, "right hint", kind)` adds a
  dim right-aligned title; `panel::section(f, inner, row, "Section")` draws a
  `── Section ────` divider row.
- Text: `util::line(f, area, row, Line::from(vec![Span::styled(..)]))`; also
  `util::fill`, `util::centered(area, w, h)`, `util::dim_area(buf, area, 0.6)`
  (modal backdrop), `util::pct(v)`.
- Bars: `bars::labeled(buf, area, row, label, value01, color, label_w, bar_w)`,
  `bars::bar`, `bars::range(min, mean, max)`, `bars::histogram(buf, area, &[u16], color, col_w)`,
  `bars::sparkline(buf, x, y, w, &[u16], color)`, `bars::vital_color(v, inverted)`.

## Map

`widgets::map::render(buf, area, source: &dyn MapSource, opts: &MapOptions)`.
`Sim` implements `MapSource`, so screens pass `sim` directly.

```text
MapOptions { overlay: Overlay, night, winter, cursor: Option<(usize, usize)>,
             follow: Option<CreatureId>, origin: (usize, usize), creatures,
             fade_creatures, selected_region: Option<usize>, … }
```

`Overlay` is `None | Vegetation | Pressure | Moisture | Sense(CreatureId) |
Region | Species(SpeciesId) | Health | Disease(Option<PathogenId>) | Parasites`.
`map::terrain_cell(cell, winter) -> (char, fg, bg)` and `map::legend()` are public.
`s01_map` keeps the shared map state (`WorldMap`, `map_options`, `map_hint`) in its
root module; each overlay's sidebar lives in its own submodule.

## Screens

Screens live in `src/ui/screens/sNN_*.rs` and implement `Screen`:

```rust
pub trait Screen: std::fmt::Debug {
    /// `false` for modals (they dim what is drawn beneath them).
    fn opaque(&self) -> bool;
    /// The top screen sees every key first; return `Action::Unhandled` for keys
    /// it does not consume.
    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action;
    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect);
}
```

`Action` is `None | Unhandled | Push | Pop | Replace | Quit | GoTitle | EnterWorld`.
Register a screen in `src/ui/screens/mod.rs`; reach it from the global key table
there.

**File layout.** Each screen file is a façade: module doc, imports, the screen
struct and its `impl Screen`, `mod` declarations, and `pub use` re-exports for
anything that must keep its `sNN_*.rs::name` path. Topic submodules hold the rest
(Rust 2021 allows `s01_map.rs` alongside `s01_map/`, so no `mod.rs` is needed).
When a screen already owns sibling modules, a shared helper belongs in the
**root**, not in a section module: a child can reach a parent's private items, but
siblings cannot reach each other's.

## Structural invariants (enforced by tests)

- `tests/file_size.rs` — **no file under `src/` may exceed 800 lines.**
- `src/sim/mod.rs` — no `ratatui`, `HashMap` or `HashSet` under `src/sim`.
- `src/glyphs.rs` — every glyph is CP437 and one cell wide.
- `src/sim/mod.rs` — `checksum_is_fnv_stable` pins the simulation checksum to
  `0x348e3c6eeec2e6d6`. Refactors of `src/sim` must be **pure code motion**: never
  reorder or merge RNG draws, or this fails.

`docs/file-split-plan.md` records how the 800-line ceiling was reached.

## Verifying

```sh
cargo clippy --all-targets     # must be warning-free
cargo test                     # lib + integration acceptance tests
```

The multi-year acceptance tests (`tests/predators.rs`, `tests/evolution.rs`, …) are
slow in debug; run them as `cargo test --release --test predators`. A 20-seed sweep
lives behind `cargo test --release --test sweep -- --ignored`.

Headless runs:

```sh
cargo run -- --headless --seed 42 --ticks 5000
cargo run -- --seeds 1-20 --years 10 --summary   # writes summary.csv
```
