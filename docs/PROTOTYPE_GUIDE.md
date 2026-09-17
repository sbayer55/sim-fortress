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
  `border()`, `border_focus()`, `selected()`. Species glyphs and colours are the
  exception: they come from the `[[species]]` roster via `ui::style::SpeciesStyle`
  (`sim.roster().glyph(id)` / `.color(id)`).
- There is no longer a fixed frame. A screen's `render` receives whatever `area`
  it is given (the full terminal from `App::draw`); tests use a 155×45
  `TestBackend`.
- Keep `cargo clippy --all-targets` at **zero warnings** (pedantic + nursery are
  on, and several lints are denied — see `Cargo.toml`).

## Layout conventions

Spec sheets for every reusable piece live in
[components/README.md](components/README.md); every sheet's component has
shipped in `src/widgets`, and `tests/components.rs` renders each sheet's
examples cell for cell. Screens draw through the components:

- Every component implements `widgets::Component` (`height(width)`,
  `min_width()`, `render(buf, area)`); layout is `VStack` / `HStack` with
  `Constraint::{Auto, Fixed, Fill, Min}`, `Spacer` for gaps, and a `Columns`
  `Block` (or a `Table`) for rows that must line up. Section builders return
  `Rows` (`Vec<Box<dyn Component>>`) and the screen stacks them with
  `VStack::from_boxes(&rows)`.
- Chrome: `Panel::new("Title").kind(Kind::Outer).info(..).foot(..).render(buf, area) -> Rect`,
  `Divider::new("Section")`, `Modal::new(w, h).title(..).buttons(..).hint(..).render(buf, area) -> Rect`
  (the body area), `ScrollRegion::new(&stack).offset(n)` with `overflow()` for
  the Foot, `StatusBar::new(&[("k", "label"), …]).right("text").render(buf, row)`,
  `Ticker`, `KeyHint`.
- Rows: `Text` (one styled row), `LabeledBar` / `Bar`, `RangeBar`, `Sparkline`,
  `TrendArrow`, `Legend`, `Histogram`, `Chart`; inputs `Stepper`, `TextField`,
  `Checkbox`, `ButtonRow`, `Menu`, `FilterStrip`.
- The free functions `panel::draw*`, `panel::section*`, `bars::*`,
  `scroll::draw` and `util::line*` remain as thin wrappers over the components
  for the screens that have not moved yet (the single-layer S01 overlay
  sidebars, S08, S13, S04b and the S04 summary, S05's stacked, phase and group
  charts; the S01 Stack section, the compact overlay sidebar and S14 are
  components). New code uses the components.

## Map

`widgets::map::render(buf, area, source: &dyn MapSource, opts: &MapOptions)`.
`Sim` implements `MapSource`, so screens pass `sim` directly.

```text
MapOptions { stack: OverlayStack, night, winter, cursor: Option<(usize, usize)>,
             follow: Option<CreatureId>, origin: (usize, usize), creatures,
             selected_region: Option<usize>, species_color, creature_tint }
```

`OverlayStack` (`widgets::map::stack`, S14) is one `Base` (`None | Vegetation |
Pressure | Moisture | Species | Parasites`), the `sense`, `regions` and `health`
marks, `Disease` (`Off | On(Option<PathogenId>)`) and the remembered `species`,
`sense_subject` and `pathogen`. `stack.layers()` walks the active layers in
composition order; `map::creature_color` is the creature-colour precedence.
`map::terrain_cell(cell, winter) -> (char, fg, bg)` and `map::legend()` are public.
The stack lives on `AppState.overlay`; `s01_map` keeps `WorldMap` (`wide`,
`region_sel`), `map_options`, `map_hint`, the S02f/S02d default rules,
`pathogen_stops` and `live_stack` (the per-frame normalisation) in its root
module; each overlay's sidebar lives in its own submodule and
`stack_sidebar.rs` holds the Stack section, the compact sidebar and the legend
rows S14 shares.

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
  `0x8c2b_5ee8_5b5a_263d`. Refactors of `src/sim` must be **pure code motion**: never
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
