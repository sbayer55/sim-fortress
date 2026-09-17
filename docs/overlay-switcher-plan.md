# Overlay switcher — implementation plan

*Revalidated against `main` at 5494dc0 (the component system merge, PR #18) on
2026-09-16. Everything below reflects that tree; the paragraph "What the component
system changed" lists the deltas from the first draft.*

*Implemented 2026-09-16 (branch `claude/overlay-switcher-plan-f05483`). Deviations from
the text below: steps 3 and 4 landed as one commit; the Disease slot is a `Disease` enum
rather than `Option<Option<PathogenId>>` (clippy `option_option`); `Esc` and `Backspace`
call `OverlayStack::clear`, which keeps the sub-picks as the S14 State section requires;
the cut mark is `·` and the row cue `»`, since CP437 has neither `…` nor `›`; the S02
renders were never test-generated, so `regenerate_screen_renders_overlays` now writes
S01a, S02a–i and S14a–c.*

**Goal.** Build [S14 Overlay Switcher](screens/s14-overlay-switcher.md): a modal over
the map that composes one base heatmap with any number of marks, picks the species,
pathogen or predator inside the dialog, previews live on the map beneath, and retires
the `1`–`9` overlay keys and the `o` cycle. The S14 document is the specification; this
file is the order of work, the design decisions behind it, and what "done" means for each
step.

**Non-goals.** No animation (mockup variants 5 and 6 are a separate decision that needs a
per-layer alpha in the renderer). No persistence of the stack or its sub-picks. No change
to what any single overlay draws: with exactly one layer on, every S02 render must come
out cell-for-cell as it does today, minus the retired selector rows in the sidebar.

**Starting point** (`main` at 5494dc0). `widgets::map::Overlay` is a single-valued
enum carried in `MapOptions.overlay` and owned by `WorldMap.overlay`
(`src/ui/screens/s01_map.rs`, 397 lines), with `sense_id` and `species_sel` beside it
as sub-pick memory. `s01_map/input.rs` (301) handles `o`, `1`–`9`, `Esc`,
`Tab`/`BackTab` in three mode handlers. Each overlay's sidebar lives in its own
`s01_map/*.rs` and ends with `overlays_selector` (`s01_map/overlays.rs`); those
sidebars still draw through the thin `panel::section` / `util::line` wrappers and are
named in `docs/components/README.md` as the next screens to move onto the components.
S12b hands a pathogen to the map through `AppState::pending_overlay`.
`src/widgets/map.rs` is 734 lines and `src/ui/screens/mod.rs` is 793 — both within a
step of the 800-line ceiling.

**What the component system changed.** `src/widgets` now holds a first-class
component per sheet in `docs/components/` (`Component` trait: `height`, `min_width`,
`render(buf, area)`), and `tests/components.rs` renders every sheet example cell for
cell from `tests/components/registry.rs`. New UI code draws through the components
(`Modal`, `Panel`, `Divider`, `Text`, `VStack`/`HStack`, `Checkbox`, `Table`,
`Legend`, `KeyHint`, `StatusBar`), not the wrappers. `Modal::new(w, h)` centres in
whatever `area` it is given and never dims; `render_stack` still applies the 0.55
backdrop dim. A component whose sheet gains an example must gain a registry entry, or
the components test fails. The determinism checksum was re-baselined to
`0xd0e3_ee1a_c665_f531` and the save format is `VERSION = 11` (worldgen history).

**Standing constraints.** Everything in `AGENTS.md`: 800-line files, clippy pedantic
with the 80-line / complexity-15 / 6-argument / 3-bool limits, CP437 glyphs from
`glyphs.rs`, colours from `theme.rs`, sim/UI separation, and the new **components
tier**: any edit under `src/widgets/` or `docs/components/` runs
`cargo test --test components`. No file under `src/sim` is touched, so
`checksum_is_fnv_stable` stays `0xd0e3_ee1a_c665_f531` and save `VERSION = 11` is
unchanged.

---

## Design decisions

**D1 — The stack lives on `AppState`, not on `WorldMap`.** The switcher is a pushed
screen and a pushed screen only sees `&mut AppState`. Putting `app.overlay: OverlayStack`
there lets the modal edit it directly, lets the map read it every frame (live preview),
and lets S12b write it directly instead of going through `pending_overlay`. `WorldMap`
keeps only what is truly the map screen's (`wide`, `region_sel`, `world_name`).

**D2 — One type, `OverlayStack`, replaces `Overlay` everywhere.** No period with both
representations in the tree beyond a single step: step 1 adds the type with
`From<Overlay>` so the renderer can switch first, step 3 deletes `Overlay`. Keeping two
would double every match and every test.

**D3 — Sub-pick memory is part of the stack.** `species`, `pathogen` and `sense_subject`
are fields of `OverlayStack` that always hold a value, so a row can show its remembered
choice while off (S14 item 3) and the S02f/S02h "remembered across Esc" rules fall out
for free.

**D4 — The renderer composes layers in place; there is no second renderer.**
`map::render` keeps its pass structure (terrain, resources, ring, trail, creatures,
cursor, region tint, labels). Each pass asks the stack a predicate (`base()`,
`health()`, `regions()`, `sense()`, `disease()`) instead of matching one enum. The
creature-colour precedence in S14 item 17 becomes one function, `creature_color`, with
its own unit test.

**D5 — The switcher is a `Screen` with an undimmed backdrop.** `Modal` never dims
(its sheet now says so); the 0.55 dim is applied by `render_stack` before any
non-opaque screen draws. S14 needs the map at full strength, so `Screen` gains a
defaulted method `fn dims_backdrop(&self) -> bool { true }` that S14 overrides and
`render_stack` consults. This also closes the "a modal cannot choose its own amount"
gap in [components/modal.md](components/modal.md).

**D6 — Centre on the map panel.** `Modal::frame(area)` and `Modal::body(area)` centre
in whatever `area` they are given, so the switcher passes the map panel's rect (from
`viewport::SIDEBAR_W` and `GUTTER_W`, matching `WorldMap::render`) rather than the whole
screen, and the sidebar's Stack section stays readable beside the modal. No change to
`util::centered`.

**D7 — `pending_overlay` is retired.** With D1 the S12b button sets
`app.overlay.disease = Some(Some(p))` and centres the viewport; `effective_overlay` and
`take_pending` in `s01_map.rs` go away with it.

**D8 — Built from components, with two Checkbox variants.** The modal is a `Modal`
whose body is an `HStack` of the left `VStack`, a one-cell vertical rule and the right
`VStack`. Layer rows are `Checkbox`es, which gain two sheet variants the S14 spec
needs: **Radio** (`(•)` / `( )`) and **Focused** (the whole row in
`theme::selected()`), plus a `value` slot for the remembered sub-pick and a `›` cue.
The sub-pick list is a `Table` (its selected-row bar and Marker column give the cursor
for free; `TableCell::species` gives the coloured glyph; `TableRow::absent` gives the
dim `extinct` row). Heatmap ramps are a `Text::spans` row of 24 shaded spans, as the
S02 sidebars draw them; band and swatch legends are `Legend::new(entries)`. The
vertical rule is S11's private `Rule` component promoted to `ui::screens::common`.
Every new Checkbox example goes into `docs/components/checkbox.md` and
`tests/components/registry.rs` together.

## Data model

```rust
// src/widgets/map/stack.rs
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Base { None, Vegetation, Pressure, Moisture, Species, Parasites }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer { Base(Base), Sense, Regions, Health, Disease }   // composition order

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OverlayStack {
    pub base: Base,
    /// Remembered even while `base != Species`.
    pub species: SpeciesId,
    pub sense: bool,
    /// Remembered even while `sense` is off; a dead subject is replaced by the S02d
    /// default rule the next time the row is turned on (S14 edge cases).
    pub sense_subject: Option<CreatureId>,
    pub regions: bool,
    pub health: bool,
    /// `Some(None)` = Disease on, every pathogen; `Some(Some(p))` = one slot; `None` = off.
    pub disease: Option<Option<PathogenId>>,
    /// Remembered pathogen while Disease is off.
    pub pathogen: Option<PathogenId>,
}

impl OverlayStack {
    pub const PLAIN: Self;                         // Default
    pub fn is_empty(&self) -> bool;
    pub fn layers(&self) -> impl Iterator<Item = Layer>;   // composition order, S14 item 16
    pub fn fades_creatures(&self) -> bool;          // S14 item 18
    pub fn title(&self, roster: &Roster) -> String; // S14 item 19, without the `…` cut
}
```

Three `bool` fields is exactly the `clippy::struct_excessive_bools` limit; Disease's
on/off is carried by its `Option` so a fifth flag never appears. `Layer` is what the sidebar, the title and
the modal rows iterate; nothing else should match on the fields directly.

## Steps

Each step ends green: `just check`, `just test-unit ui`, `just test-unit widgets`. Steps
0 to 2 change no behaviour and can land as their own commits.

### Step 0 — make room (pure code motion)
| Move | From → to | Why |
|------|-----------|-----|
| the `#[cfg(test)] mod tests` block (lines 124–793) | `src/ui/screens/mod.rs` → `src/ui/screens/tests.rs` (`#[cfg(test)] mod tests;`) | mod.rs is 793 lines; step 6 adds a `pub mod` line, the `dims_backdrop` default and a branch in `render_stack` |
| `overlay_cell`, `density_field`, `density_cell`, `condition_color`, `parasite_cell`, `disease_tint`, `parasite_tint` and their tests | `src/widgets/map.rs` → `src/widgets/map/overlay.rs`, re-exported with `pub use` | map.rs is 734 lines; step 2 adds composition |

Bodies move verbatim; `pub use` keeps every path. Re-apply test lint `allow`s on the new
test module (AGENTS.md gotcha). *Done when* `git diff --stat` shows only moves,
clippy is clean, and `cargo test --lib -- --ignored regenerate_screen_renders` produces
no diff under `docs/screens/renders/`.

### Step 1 — `OverlayStack`
- New `src/widgets/map/stack.rs` with the model above, `Default`, and
  `impl From<Overlay> for OverlayStack` (a temporary bridge deleted in step 3).
- Unit tests: `layers_are_in_composition_order`, `title_joins_layers_with_plus`
  (`moisture + regions + health + disease: greyfever`; species uses the roster plural),
  `fades_only_under_a_base_without_health_or_disease`, `plain_is_empty`.

*Done when* the tests pass and nothing else references the type yet.

### Step 2 — the renderer composes a stack
Files: `src/widgets/map.rs`, `src/widgets/map/overlay.rs`.
- `MapOptions.overlay: Overlay` → `MapOptions.stack: OverlayStack`. Call sites:
  `s01_map.rs::map_options`, `s07_log.rs:327–328`, `s03_inspector/life.rs:68–69` (the
  last two pass `OverlayStack::default()`), and the four `MapOptions { overlay: … }`
  tests in map.rs. `s01_map.rs` converts with `.into()` for now. `map.rs` has no sheet,
  so the components test is unaffected by this step.
- `draw_terrain`: base cell from `stack.base` (the existing `overlay_cell` /
  `density_cell` / `parasite_cell`), then `if stack.health` dim 60 %, then the S02h ground
  tint when `stack.disease.is_some()`.
- `region_tint` / `region_labels` run when `stack.regions`; `draw_sense_ring` when
  `stack.sense`; the ring interior tint applies after the region tint.
- `draw_creatures`: `creature_color(&stack, creature, tint) -> (Color, bool)` implementing
  S14 item 17 (Health → Disease → Parasites → Species → fade → species colour). The
  `creature_tint` map keeps its shape; the caller fills it for Disease and Parasites and
  the function looks it up in that order.
- `fade_creatures` on `MapOptions` becomes `stack.fades_creatures()`; drop the field.
- Tests: the four existing map tests re-expressed on the stack; new
  `health_beats_disease_on_a_creature`, `regions_and_sense_tints_both_apply`,
  `species_base_under_regions_keeps_density_shade`.

*Done when* map tests are green and the S02 renders regenerate with **no diff** (one
layer at a time is unchanged by construction).

### Step 3 — the stack moves to `AppState`
Files: `src/ui/app.rs`, `src/ui/screens/s01_map.rs`, `s01_map/input.rs`,
`s01_map/tests.rs`, `src/ui/screens/s12_alert.rs`, `src/widgets/map.rs`.
- `AppState.overlay: OverlayStack` replaces `pending_overlay`; `WorldMap` drops
  `overlay`, `sense_id`, `species_sel`. `default_species`, `default_sense`,
  `open_species`, `cycle_species`, `cycle_pathogen`, `cycle_sense`, `next_predator` stay
  on `WorldMap` but read and write `app.overlay`.
- `effective_overlay` and `take_pending` are deleted; the "sense subject died" fallback in
  `render` becomes a one-line normalisation at the top of `handle_key` and `render`
  (`app.overlay.sense = None` when the subject is not alive; `disease` slot → `None`
  when the slot is gone), matching the S14 edge cases.
- S12b `show_outbreak` sets `app.overlay.disease = Some(Some(p))` and `pathogen`.
- Turning Sense on (`cycle_sense` today, the S14 row in step 6) sets `sense = true` and
  replaces a dead or absent `sense_subject` with `default_sense(app)`.
- Delete `Overlay` and the `From` bridge; `select_overlay` and `cycle_overlay` are
  rewritten against the stack for now (they are deleted in step 4) so this step stays
  behaviour-neutral.
- Tests: `s12_alert` tests at lines 411–427 (the setter is at line 203) and
  `s01_map/tests.rs` 145–150 assert on `app.overlay` instead of `pending_overlay`;
  `screens/tests.rs` cases that read `WorldMap.overlay` read `app.overlay`.

*Done when* every UI test is green with the same key sequences as before.

### Step 4 — map keys and hints
Files: `s01_map/input.rs`, `s01_map.rs::status_keys`, `src/ui/screens/s11_help.rs`.
- Remove the `'1'..='9'` arms from `handle_plain_key`, `handle_look_key` and
  `handle_follow_key`; remove `select_overlay` and `cycle_overlay`.
- `o` is matched once at the top of `WorldMap::handle_key`, before the follow / look /
  plain dispatch, and returns `Action::Push(Box::new(OverlaySwitcher::new()))` (a stub
  screen until step 6 — an empty non-opaque screen that pops on `Esc` is enough to keep
  this step compiling and testable). The three per-mode `o` arms are removed.
- `Esc` in plain mode sets `app.overlay = OverlayStack::default()`.
- `status_keys`: `[o] overlay` replaces `[1-9] overlay` / `[o] cycle` in every branch;
  the `Tab` hints stay conditional on the layer being on. The bar is already drawn
  through `StatusBar::new(keys)`, so only the slices change.
- S11 help rows (`s11_help.rs` `keys_column`, lines 208 and 215): `("o", "cycle
  overlays")` → `("o", "overlay switcher")`, `("Esc", "clear overlay")` → `"clear
  overlays"`; there are no `1`–`9` rows to remove.
- Tests: rewrite the key-driven overlay tests in `screens/tests.rs` (lines 385–410,
  544–560, 619–638) and `s01_map/tests.rs` (156–164) to set `app.overlay` directly, and
  add `digits_do_nothing_on_the_map` and `o_pushes_the_switcher`.

### Step 5 — sidebar and title
Files: `s01_map/overlays.rs` (rename the file `s01_map/stack_sidebar.rs`), every
`s01_map/{sense,regions,species,health,disease_overlay,parasites}.rs`, `s01_map.rs`.
- Delete `overlays_selector`; each single-layer sidebar calls `stack_section` in the rows
  it freed (S14 item 20). `stack_section` returns `Rows` (`Divider`, `Text` per layer,
  a dim `Text` hint) and the wrapper-based sidebars render it with
  `VStack::from_boxes(&rows).render(buf, Rect { y: inner.y + row, height: inner.height − row, ..inner })`,
  the same mixing the S01 status sidebar used mid-migration. Row budgets in the S02 doc
  change by the selector's height minus the Stack section's; record the new counts in
  S02 when regenerating renders.
- New `compact_sidebar` for two or more layers, built entirely from components: per
  layer a `Divider` (name and sub-pick), a `Text` description, and its legend (a
  `Text::spans` ramp row plus a tick-label `Text`, or a `Legend::new(entries)` of
  bands or region swatches); then `stack_section`; `VStack` drops what does not fit
  from the bottom. This is the S01-overlay-sidebar migration the components README
  lists as next, done for the sidebars this feature touches; the per-layer legend
  rows are `pub(super)` free functions in `stack_sidebar.rs` returning `Rows`, so the
  S14 modal (step 6) reuses them.
- `draw_map_sidebar` dispatch: follow / look first as today, then
  `match stack.layers().count() { 0 => status sidebar, 1 => that layer's sidebar,
  _ => compact }`.
- `map_origin_title`: `overlay: ‹stack.title()›`, cut with `…` so the scroll hint fits
  (S14 item 19). The S01 status sidebar's Overlay section (`base.rs`, already a
  `Rows` builder) changes to the two S14 lines.
- Tests: `single_layer_sidebar_ends_with_stack_section`,
  `compact_sidebar_lists_layers_in_order`, `long_title_is_cut_before_the_hint`.

### Step 6 — the S14 screen
Files: `src/ui/screens/s14_switcher.rs` (root: struct, `Screen` impl, key handling,
≈250 lines), `s14_switcher/rows.rs` (the two tabs' row tables, sub-pick `Table`
sources, right-column builders ≈250 lines), `s14_switcher/tests.rs`;
`src/widgets/checkbox.rs` (+ `docs/components/checkbox.md`,
`tests/components/registry.rs`); `src/ui/screens/common.rs` (the `Rule` moved out of
`s11_help.rs`); `src/ui/screens/mod.rs` (`pub mod s14_switcher;`, the `dims_backdrop`
default, and `render_stack` skipping `util::dim_area` when the top screen returns
`false`).
- **Checkbox variants first** (own commit, gated by `cargo test --test components`):
  `.radio(true)` draws `(•)` / `( )`; `.focused(true)` fills the row in
  `theme::selected()`; `.value("hare")` writes a second text slot after the label pad;
  `.cue()` writes `›` at the right edge; `.disabled(true)` draws the row in
  `theme::dim_text()`. Add a **Radio** and a **Focused** variant row to the sheet's
  Variants table, one example fence each under Examples, and one registry entry per
  fence. This answers the sheet's open question about a focused look.
- State: `tab: Tab { Base, Marks }`, `cur: usize`, `focus: Focus { Rows, List }`,
  `list_cur: usize`. Rows come from `rows::rows(tab, app)`; each row knows its `Layer`,
  name, description, whether it is disabled (Sense with no living predator), and its
  sub-pick kind.
- Keys exactly as the S14 Interaction tables; everything else `Action::Unhandled`.
  `Space` in the list acts as `Enter` (keys.md rule 2). No `KeyCode::Char('0'..='9')`
  arm anywhere in the file — add a test that asserts digits are `Unhandled`.
- Render: `Modal::new(84, 21).title("Overlay").info("o or Esc closes")` with
  `frame(map_panel)` / `body(map_panel)` (D6). Body = `VStack` of: the tab strip
  (`Text::spans`), the tab rule (`Text::spans`: `─` cells with the active span in
  `theme::SELECT_BG` and the rule word right-aligned), an `HStack` of the left column
  (`VStack` of `Checkbox` rows, `Fixed(34)`), the `Rule` (`Fixed(1)`), and the right
  column (`Fill(1)`); then a `Divider::new("Showing")`, the stack-title `Text` and the
  hint `Text`. The `╤` and `┴` joins are two `set_stringn` cells after the stack
  renders. Right column: a `Table` from a `RowSource` over the sub-pick entries with
  `.selected(list_cur)` while the list has focus, or the description, legend rows
  (step 5 helpers) and the two dim notes. Then the status row through
  `StatusBar::new(&[…])` with `[Bksp]`, undimmed as the other modals do.
- Sub-pick lists read the sim each frame: species in roster order with living counts;
  predators by id ascending capped at 10 with `… and N more` as a `TableRow::tail`;
  pathogens `All` then non-extinct slots with strains indented — reuse the ordering code
  from `cycle_pathogen` by lifting it to a `pathogen_stops(sim)` helper on the map
  screen root.
- Box-drawing joins: add `glyphs::T_DOWN` `╤` and `glyphs::T_UP` `┴` beside `CROSS`, so
  `all_glyphs_are_cp437` covers them.
- Tests (TestBackend 155 × 45, seed-7 sim as in `s01_map/tests.rs`):
  `opens_on_base_tab_first_row`, `tab_flips_tabs`, `space_selects_base`,
  `space_toggles_mark`, `right_enters_list_enter_picks_and_enables`,
  `left_returns_without_change`, `backspace_clears_stack_and_stays_open`,
  `enter_closes_and_keeps_stack`, `digits_are_unhandled`, `sense_row_disabled_without_predators`,
  `dead_sense_subject_turns_mark_off`, `render_matches_geometry` (box corners at
  (14,10)–(97,30), divider at column 49), `backdrop_is_not_dimmed`.

### Step 7 — documentation and renders
- `docs/screens/README.md`: S14 row, `S01 -- "o" --> S14`, `S14 -- "Esc / Enter" --> S01`
  in the navigation graph, and "Modals over the map" family; the global key table drops
  `1`–`9`.
- `docs/screens/s01-world-map.md` line 115 (`o`, `1`–`7`), `s02-map-overlay.md`
  (Variants "when shown" column, every Interaction table, item 16, the row budgets),
  `s11-legend-help.md` lines 94 and 125, `s12-alert-modal.md` (Show outbreak wording).
- `docs/components/keys.md`: `[Bksp]` spelling; the `1`–`9` row loses the overlay use;
  a Deviations row if `Backspace` stays (see S14 open questions).
  `docs/components/modal.md`: S14 in the sizes table (84 × 21, `Overlay`, Info
  `o or Esc closes`), the undimmed-backdrop note under API (D5 hook on `Screen`).
  `docs/components/checkbox.md`: written in step 6 with its registry entries; only the
  cross-reference to S14 is left for here. `docs/components/table.md`: S14's sub-pick
  list joins the "Used by" column.
- `docs/PROTOTYPE_GUIDE.md` lines 66 (the overlay sidebars are no longer "not moved
  yet"), 72–76 (`MapOptions` now carries `stack`) and 84 (`s01_map` state).
- Renders: `cargo test --lib -- --ignored regenerate_screen_renders` for `S01a`–`S02i`
  (selector rows gone); extend `src/ui/tests.rs::regenerate_screen_renders` to write
  `S14a.txt`, `S14b.txt`, `S14c.txt` from the modal over the S02c state.

### Step 8 — verification
```sh
just check                                   # 0 warnings, no file over 800 lines
just test-unit ui                            # every screen test
just test-unit widgets                       # map composition and stack tests
cargo test --test components                 # every sheet example, incl. the two Checkbox variants
just test-unit sim::tests::checksum          # untouched: 0xd0e3_ee1a_c665_f531
cargo run                                    # the demo below
```
Demo script: New World → `o` → `Space` on Moisture → `Tab` → `Space` on Regions →
`↓` `Space` on Health → `↓` `→` `↓` `Enter` (Greyfever) → `Enter`. Expect the title
`… · overlay: moisture + regions + health + disease: greyfever`, the compact sidebar
with a four-row Stack section, `Esc` clearing everything, and `1`–`9` doing nothing.

## Test plan summary
| Level | Where | Covers |
|-------|-------|--------|
| unit | `widgets/map/stack.rs` | order, title, fade rule, emptiness |
| unit | `widgets/map.rs` | per-cell composition, creature-colour precedence, single-layer parity |
| unit | `ui/screens/s14_switcher/tests.rs` | every key, geometry, disabled row, dead subject, no dim, no digits |
| components | `tests/components.rs` via `docs/components/checkbox.md` + `registry.rs` | the Radio and Focused Checkbox variants, cell for cell |
| unit | `ui/screens/s01_map/tests.rs`, `ui/screens/tests.rs` | map keys, sidebar dispatch, title cut, S12b hand-off |
| snapshot | `docs/screens/renders/S02*.txt`, `S14a–c.txt` | visual regression; S02 differs only by the selector rows |
| chunk | none | no `src/sim` change; acceptance suites are unaffected |

## Risks and how the plan handles them
- **800-line ceiling.** `screens/mod.rs` (793) and `map.rs` (734) are split *before*
  they grow (step 0). `s01_map.rs` (397) loses more than it gains in step 3.
- **Component discipline.** Two Checkbox variants and two registry entries are the price
  of drawing the rows through the component instead of hand-laid spans; the alternative
  (spans in the screen) would be the only new screen not built from components and
  would be flagged at review. The variants land in their own commit with the
  components test green before the screen uses them.
- **Clippy limits.** `creature_color` and the terrain pass are where complexity 15 bites;
  each layer predicate is its own small function. `struct_excessive_bools` is avoided by
  carrying Disease as an `Option`. The 6-argument limit already forces
  `#[allow(clippy::too_many_arguments)]` on `draw_map_sidebar`; do not add a seventh —
  pass `&OverlayStack` instead of `overlay, overlay_active`.
- **Test churn.** Fourteen existing tests press overlay keys. Step 3 keeps them green by
  keeping the old keys; step 4 rewrites them in one go against `app.overlay`, so no
  intermediate commit has a red suite.
- **Render parity.** Step 2's "no diff" gate is the proof that composition did not change
  any single overlay; step 5 is the only step allowed to change S02 renders, and only in
  the selector rows.
- **`redundant_pub_crate` and sibling privacy** (AGENTS.md gotchas): shared legend helpers
  are `pub(super)` free functions in the `s01_map` root's child, reached by the S14 screen
  through a `pub` re-export on `s01_map.rs`, not by `pub(crate)` on a private submodule.
- **Look and follow modes.** `o` is handled before the mode dispatch in
  `WorldMap::handle_key`, so the modal opens from every mode without three copies of the
  arm.

## Follow-ups not in this plan
- Animated variants (mockup 5 and 6): a per-layer alpha on `OverlayStack` and a
  frame-timer redraw; the modal offset and clip are cheap, the crossfade is not.
- Persisting sub-picks to `ui.toml`.
- The S02 open question on whether the fouled-ground tint belongs to Parasites rather
  than Disease; the composition model makes moving it a one-line change once decided.
