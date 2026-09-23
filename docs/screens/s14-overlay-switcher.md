# S14 — Overlay Switcher

Back to the [screen overview](README.md).

Status: **built** (Sept 2026, `src/ui/screens/s14_switcher.rs`; renders
[S14a](renders/S14a.txt), [S14b](renders/S14b.txt), [S14c](renders/S14c.txt)). Mockup:
variant 4 of the throwaway `overlay-switcher-mockup.html`. The implementation plan is
[../overlay-switcher-plan.md](../overlay-switcher-plan.md). The layer rows are
[Checkbox](../components/checkbox.md) rows (Radio, Focused, Disabled, Value and Cue
variants) and the sub-pick list is a [Table](../components/table.md); items 5 and 10
follow those sheets where the mockup differed. Two glyph deviations from this text, forced
by the CP437 invariant: the row cue is drawn `»` (`glyphs::CUE`), not `›`, and a cut map
title ends in `·` (`common::clip`), not `…`.

## Purpose
One place to compose what the map shows. Today [S02](s02-map-overlay.md) is nine
mutually exclusive overlays reached by `1`–`9` and an `o` cycle, and three of them carry a
hidden second choice (which species, which pathogen, which predator) that is only reachable
by `Tab` after the overlay is up. The switcher replaces all of that with a modal over the
map that separates the overlays into two kinds:

- a **base heatmap** — at most one, because it recolours every cell: vegetation, pressure,
  moisture, species density, parasites, scent, or none;
- **marks** — any number, because each draws *on top of* whatever is beneath it: the sense
  ring, region tints and labels, the health recolouring of creatures, the disease
  recolouring of creatures.

The player can therefore ask compound questions the single overlays cannot ("are the
sick deer in the dry regions?" = species: deer + regions + disease) and picks the species,
pathogen or predator inside the same dialog instead of cycling blind. The map underneath
updates live as layers are toggled, so the dialog is also a preview.

The number keys are retired: the switcher has no `0`–`9` bindings, on the map or inside
the modal.

## Variants
| Id   | Variant                | When it is shown                                                       |
|------|------------------------|------------------------------------------------------------------------|
| S14a | Base heatmap tab       | `o` from the map (plain, look or follow mode); the modal opens on this tab with the cursor on the first row |
| S14b | Marks tab              | `Tab` inside the modal                                                 |
| S14c | Sub-pick list focused  | `→` on a row that has a sub-pick (Species, Sense, Disease); the right column takes the cursor |

## Layout
A [Modal](../components/modal.md) of **84 × 21** centred on the **map panel**, not on the
whole screen: on the 112 × 42 map panel it occupies columns 14–97 and rows 10–30, so the
sidebar stays fully visible beside it. In the wide (S01b) view the panel is 152 columns
and the modal centres on that. Unlike the other modals the backdrop is **not dimmed**: the
map beneath is the live preview and must read at full strength.

The inner area is 82 × 19 and is split into a fixed left column and a right column:

| Region        | Inner columns | Inner rows | Notes                                                             |
|---------------|---------------|------------|-------------------------------------------------------------------|
| Tab strip     | 0–33          | 0          | ` Base heatmap ` at column 1, ` Marks ` at column 16, `Tab` key at column 30 |
| Tab rule      | 0–33          | 1          | `─` rule; the active tab's span is blanked in the selection background; the tab's rule word (`pick one` / `any number`) right-aligned in the accent colour |
| Layer rows    | 0–33          | 2–14       | one row per layer of the active tab, from row 2                    |
| Divider       | 34            | 0–14       | `│`, joined to the top border with `╤` and ended with `┴` on row 15 |
| Right column  | 36–80         | 0–14       | sub-pick list, or the hovered layer's description and legend       |
| Showing       | 0–81          | 16–18      | `─ Showing ─` section rule, the stack title, then a dim hint line   |

```
╔ Overlay ═════════════════════════╤══════════════════════════════ o or Esc closes ╗
║  Base heatmap   Marks        Tab │ species · which species                       ║
║─              ────── pick one ───│   v vole                 212 alive            ║
║  ( ) None                        │   h hare                  97 alive            ║
║  ( ) Vegetation                  │   d deer                  41 alive            ║
║  ( ) Pressure                    │   f fox                   23 alive            ║
║  (•) Moisture                    │   w wolf                   9 alive            ║
║  ( ) Species     hare         ›  │   l lynx                   6 alive            ║
║  ( ) Parasites                   │                                               ║
║  ( ) Scent       hare         ›  │ → steps into this list, ← back                ║
║                                  │ Enter picks it and switches the layer on      ║
║                                  │ Space on the left toggles without picking     ║
║                                  │                                               ║
║                                  │                                               ║
║                                  │                                               ║
║                                  │                                               ║
║                                  ┴                                               ║
║─ Showing ────────────────────────────────────────────────────────────────────────║
║  moisture                                                                        ║
║  Enter closes and keeps the stack   Backspace clears everything                  ║
╚══════════════════════════════════════════════════════════════════════════════════╝
```
*S14c over S14a: the Base heatmap tab with moisture on, the cursor on Species and the
species list focused.*

A layer row is laid out as: blank, blank, the three-cell **mark** at columns 2–4, blank,
the **name** at column 6 padded to 11, the **sub-pick value** at column 18 cut to 11, and a
`›` cue at column 31 on rows that have a sub-pick.

## Content requirements

### Left column — layer rows
1. **Base heatmap tab** lists, in this order: `None`, `Vegetation`, `Pressure`,
   `Moisture`, `Species`, `Parasites`, `Scent`, `Succession`. Exactly one is on. The mark is a radio, `(•)` on and
   `( )` off.
2. **Marks tab** lists, in this order: `Sense`, `Regions`, `Health`, `Disease`. Any number
   may be on. The mark is a [Checkbox](../components/checkbox.md) mark, `[x]` on and `[ ]`
   off.
3. **Sub-pick value.** `Species`, `Scent`, `Sense` and `Disease` rows show their current choice in
   the value slot even when the layer is off, so the row says what would come on: the
   species' lowercase name; the predator's name; `All pathogens` or the pathogen's name.
   The value is the remembered choice (see State), never blank.
4. **Sense with no living predator**: the row is drawn dim with the value `no predators`
   and cannot be turned on; `Space` and `→` do nothing on it.
5. **Row styling.** The cursor row is a Focused [Checkbox](../components/checkbox.md):
   the whole row in `theme::selected()`, only while the left column has focus. A row's
   name is bright and bold when it is the cursor row or the layer is on. The mark follows
   the Checkbox sheet: `theme::GOOD` bold when on, `theme::dim_text()` when off (the
   mockup drew it in the accent colour; the sheet wins). The sub-pick value is text colour
   when the layer is on, dim otherwise. The `›` cue is accent on the cursor row, dim
   elsewhere.

### Right column — hovered row has a sub-pick (S14c and the un-focused preview of it)
6. **Header** in the accent colour: `species · which species`, `sense · which predator`,
   `disease · which pathogen`.
7. **Species list**: every roster species in roster order, `‹glyph› ‹name›` in the species
   colour, right part `N alive`, or `extinct` dim when the count is zero. Extinct species
   stay selectable (S02f).
8. **Predator list**: every living predator, id ascending, `‹GLYPH› ‹name› (‹species›)` in
   the species colour, right part `sense N` where N is the ring radius in cells
   (`2 + floor(sense × 10)`, S02d). At most 10 rows, then `… and N more` dim. The current
   subject is always among the rows shown.
9. **Pathogen list**: `All pathogens` first, then every non-extinct slot in slot order, a
   strain indented `└ ` under its parent; right part `N sick` (creatures currently
   infectious or incubating with that pathogen, all of them for `All pathogens`).
10. **Selection marks.** The list is a [Table](../components/table.md) without a header.
    Its Marker column carries `•` in the accent colour on the entry currently chosen for
    the layer, with a bright bold label, **only while the layer is on**; when the layer is
    off the remembered entry has no marker, so an off layer never looks active. While the
    right column has focus the Table's selected row is the list cursor: the row is filled
    with the selection background and the Marker column shows `►` there instead (the
    mockup showed no `►`; the sheet wins).
11. **Footer** (three dim lines under the list): `→ steps into this list, ← back`,
    `Enter picks it and switches the layer on`, `Space on the left toggles without
    picking`.

### Right column — hovered row has no sub-pick
12. Line 1 (accent): the layer name and its kind, `moisture  · base heatmap` or
    `regions  · mark`. Line 2: the one-line description from the table below. Then, after a
    blank row, the **legend**: heatmaps show the 24-cell ramp built from the map's shade
    glyphs on the layer's ramp (`theme::veg`, `heat`, `water`, `parasite`) with the
    `0% 25% 50% 75% 100%` ticks beneath; Health shows `■ fit  ■ strained  ■ critical` in the
    good / warning / bad colours; Regions lists every region as a `■` swatch in the region's
    colour and its name; None reads `terrain and creatures only` dim.
13. Rows 12–13 of the column (dim): `no sub-pick for this layer` and either `Space makes it
    the base` or `Space toggles it`.

| Layer      | Kind | Description line                            | Sub-pick |
|------------|------|---------------------------------------------|----------|
| None       | base | plain terrain                               | —        |
| Vegetation | base | standing biomass per cell                   | —        |
| Pressure   | base | prey and predator traffic                   | —        |
| Moisture   | base | soil moisture, open water saturated         | —        |
| Species    | base | population density of one species           | species  |
| Parasites  | base | parasite load heatmap                       | —        |
| Scent      | base | one predator species' scent and holders     | species  |
| Succession | base | ground climbing toward forest or wearing to dirt | —   |
| Sense      | mark | one predator's sense-range rings            | predator |
| Regions    | mark | named regions, tinted with labels           | —        |
| Health     | mark | creatures by their weakest vital            | —        |
| Disease    | mark | infection state per pathogen                | pathogen |

### Showing section
14. Row 17 shows the **stack title** bright and bold, `nothing, plain terrain` dim when
    the stack is empty. The stack title is the same string the map panel title uses (item
    19), so the two always agree.
15. Row 18 (dim): `Enter closes and keeps the stack   Backspace clears everything`.

### Effects on the map (changes to [S02](s02-map-overlay.md))
16. **Composition order** per frame: terrain (night and winter palettes suppressed whenever
    any layer is on, as today for one overlay) → base heatmap cell (S02a/b/c/f/i rules,
    including the water and rock exclusions) → Health terrain dim 60 % (S02g item 19) →
    Regions tint (S02e item 10) → sense-ring interior tint and `°` edge (S02d items 10–11)
    → resources → region labels → creatures → look cursor.
17. **Creature colour precedence** when several layers want it, highest first: Health
    band (S02g) → Disease tint (S02h) → Parasites band (S02i) → Species base (own species
    full colour, others faded 55 %) → the plain 55 % fade whenever a base heatmap is on →
    species colour. The followed creature keeps its accent highlight and the sense subject
    its bright bold glyph under every combination.
18. **Fading.** Creatures and resources fade (55 % / 50 %) when a base heatmap is on and
    neither Health nor Disease is on; Regions and Sense alone never fade anything, as
    today.
19. **Map title**: `‹world› · overlay: ‹stack›` where the stack is the active layers in
    composition order joined by ` + `, named as S02 names them: `vegetation`, `pressure`,
    `moisture`, the species' lowercase plural, `parasites`, `‹species› scent`, `sense range`, `regions`,
    `health`, `disease` or `disease: ‹pathogen›`. When the title plus the scroll hint would
    not fit the top border, the title is cut with `…` so the hint survives.
20. **Sidebar.** The **Overlays selector** section (S02 item 16) is removed from every
    overlay sidebar. In its place, at the bottom, a **Stack** section: `─ Stack ─`, one row
    per active layer `N. ‹Name›  (base)` / `(mark)` in composition order, then the dim hint
    `o edits the stack   Esc clears all`. With exactly one layer on, the sidebar is that
    layer's existing S02 sidebar with this substitution. With two or more on, the sidebar
    is the **compact** form: for each layer in order, its name as a section rule (with the
    sub-pick after ` · `), its description line and its legend as in item 12, then the
    Stack section; sections that do not fit the 40 inner rows are dropped from the bottom,
    the Stack section last.

```
╔ Overlay ════════════════════════════════╗
║─ Species · deer ────────────────────────║
║  population density of one species      ║
║         ░░░░░▒▒▒▒▓▓▓▓▓█████             ║
║   0%      25%      50%      75%      100%
║                                         ║
║─ Regions ───────────────────────────────║
║  named regions, tinted with labels      ║
║  ■ Fern Hollow     20 creatures         ║
║  ■ Rowan Ridge     27 creatures         ║
║  ⋯                                      ║
║─ Health ────────────────────────────────║
║  creatures by their weakest vital       ║
║  ■ fit    ■ strained  ■ critical        ║
║                                         ║
║─ Disease · All pathogens ───────────────║
║  infection state per pathogen           ║
║  ☻ 31 sick   ☺ 118 immune               ║
║                                         ║
║─ Stack ─────────────────────────────────║
║  1. Species  (base)                     ║
║  2. Regions  (mark)                     ║
║  3. Health   (mark)                     ║
║  4. Disease  (mark)                     ║
║  o edits the stack   Esc clears all     ║
```
*Compact sidebar with four layers on (region rows elided here).*

21. **Status bar (map, switcher closed)**: `[o] overlay` replaces `[1-9] overlay` and
    `[o] cycle` in every S01/S02 hint set. `[Tab] next species` / `next pathogen` / `next
    predator` and `[Shift+Tab] previous` stay while that layer is on. `[Esc] clear` stays.
22. **S12b "Show outbreak"** turns the Disease mark on with that pathogen, leaves the base
    and other marks as they are, and centres the viewport as today.

## State
- The stack is one value owned by the app, not by the map screen, so the modal (a pushed
  screen) edits it directly and the map beneath reads it every frame:
  `base` (one of six), the four mark flags, and three **remembered sub-picks**: the
  species (initially the S02f default rule), the pathogen (`All` initially) and the sense
  subject (initially the S02d default rule, re-evaluated when the row is turned on and the
  remembered subject is dead).
- The remembered sub-picks survive turning the layer off and on, `Esc` clearing the
  stack, and leaving for a data screen. They are not saved to disk.
- The modal's own state (tab, cursor row, focused column, list cursor) is discarded on
  close; the modal always opens on the Base heatmap tab, cursor on the first row, left
  column focused.

## Glyphs and colors
| Glyph / colour                        | Meaning                                                  |
|---------------------------------------|----------------------------------------------------------|
| `(•)` `( )`                           | base heatmap on / off (Radio Checkbox); `•` is `glyphs::BULLET`; on is `theme::GOOD` bold |
| `[x]` `[ ]`                           | mark on / off (Checkbox); on is `theme::GOOD` bold       |
| `›`                                   | this row has a sub-pick list to the right                |
| `•` accent                            | the chosen entry in a sub-pick list, only while its layer is on |
| `►`                                   | the list cursor row while the sub-pick list has focus (Table marker) |
| `└ `                                  | a strain under its parent pathogen (S02h)                |
| `┴` `╤` `│`                           | the column divider and its joins                         |
| ` ░ ▒ ▓ █` on a ramp                  | legend sample of a heatmap                               |
| `■` in good / warning / bad           | health bands                                             |
| `■` in `theme::region(i)`             | region swatch                                            |
| `☻` sick, `☺` immune                  | disease counts in the compact sidebar                    |
| selection background                  | the cursor row of the focused column; the active tab     |
| `theme::border_focus()`               | the modal box (Panel `Kind::Focus`)                      |
| `theme::label()` (accent)             | section rules, column headers, the tab rule word         |
| `theme::key()`                        | the `Tab` label in the tab strip                         |

## Interaction
Standard keys are in [keys.md](../components/keys.md). The modal consumes every key it
lists and returns the rest unhandled, so `?` and the data-screen letters keep their
global meaning; the simulation keeps running (or stays paused) underneath.

### Map (switcher closed)
| Key   | Action                                                        | Goes to |
|-------|---------------------------------------------------------------|---------|
| `o`   | open the switcher, from plain, look and follow modes alike     | S14a    |
| `Esc` | clear the whole stack (when not in look or follow mode, which `Esc` leaves first as today) | [S01](s01-world-map.md) |
| `Tab` / `Shift+Tab` | next / previous species, pathogen or predator while that layer is on, as S02 defines; otherwise the S01 meaning | stays |
| `1`–`9` | **no longer bound** on the map                              | —       |

### Inside the modal, left column focused (S14a, S14b)
| Key         | Action                                                                     |
|-------------|----------------------------------------------------------------------------|
| `Tab`, `Shift+Tab` | flip to the other tab; cursor to its first row                      |
| `↑` `↓`     | move the cursor, wrapping                                                  |
| `Space`     | base tab: make this row the base; marks tab: toggle this row               |
| `→`         | on a row with a sub-pick: focus the list, cursor on the remembered entry   |
| `Enter`     | close, keeping the stack                                                   |
| `Backspace` | clear the whole stack (base to None, every mark off); stays open           |
| `Esc`, `o`  | close, keeping the stack                                                   |

### Inside the modal, sub-pick list focused (S14c)
| Key         | Action                                                                     |
|-------------|----------------------------------------------------------------------------|
| `↑` `↓`     | move the list cursor, wrapping                                             |
| `←`         | back to the left column, nothing changed                                   |
| `Enter`, `Space` | pick the entry, turn the layer on (a base row becomes the base), and return focus to the left column |
| `Tab`       | as in the left column: flip tabs (focus returns to the left column)        |
| `Backspace`, `Esc`, `o` | as in the left column                                          |

Every key above appears in the status bar while the modal is open:
`[Tab] base / marks  [↑↓] move  [Space] toggle  [→] sub-pick  [←] back  [Enter] done
[Bksp] clear all  [o Esc] close`. `[Bksp]` is a new spelling for
[keys.md](../components/keys.md).

## States and edge cases
- **Empty stack**: Showing reads `nothing, plain terrain`; the map title has no suffix;
  the sidebar is the S01 status sidebar, whose Overlay section reads `press o to open the
  switcher` / `Tab flips between base and marks`.
- **Sense subject dies** while the mark is on: the mark turns itself off at the next frame
  (S02d's fallback), the remembered subject is cleared, and the row is re-evaluated the
  next time it is turned on. If no predator lives the row is disabled (item 4).
- **Shown pathogen slot vanishes**: the pathogen falls back to `All pathogens` (S02h).
- **Extinct species** as the base: the map shows no shading and the list row reads
  `extinct`; the layer stays on.
- **Health and Disease both on**: Health wins on every creature (item 17); the disease
  ground tint (S02h item 20) still shows.
- **Regions and Sense both on**: both tints apply, regions first; region labels draw over
  the ring interior but under creatures.
- **Wide view**: the modal centres on the 152-column panel; the Stack section is hidden
  with the rest of the sidebar, so the map title is the only summary.
- **Look and follow modes**: `o` opens the switcher without leaving the mode; on close the
  mode is as it was. The look cursor and follow highlight draw over every layer.
- **Small terminals**: the modal is clamped to the map panel minus a one-cell margin like
  every Modal; below 86 × 23 the right column is dropped first, then rows are cut from the
  bottom.
- **Paused simulation**: counts in the lists (`alive`, `sick`) are read each frame and
  simply stop changing.

## Open questions
- Should the disease ground tint (fouled cells, S02h item 20) belong to the Disease mark or
  to the Parasites base? Today it is drawn only under the disease overlay; the compact
  model suggests it is a Parasites property.
- `Backspace` is not in the keys standard. `Del` is the other candidate (Load World uses
  it to delete a save); a letter shown in the hint would also satisfy keys.md rule 4.
- Should the remembered sub-picks persist to `ui.toml`, so a session that always watches
  wolves reopens on wolves?
- Should the modal open on the Marks tab when a mark row was the last thing changed, or
  always on Base heatmap as specified?
- The 55 % creature fade under a base heatmap was tuned for one overlay; with Regions on
  top the faded creatures may need a contrast check on real terminals (S02 open question).
- Whether the sense ring interior tint should also apply under a heatmap base, where the
  18 % accent blend fights the ramp colour, is undecided; the mockup applies it.
