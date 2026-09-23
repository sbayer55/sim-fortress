# S16 — Top Dynasties

Back to the [screen overview](README.md).

Status: **built** (Sept 2026, `src/ui/screens/s16_dynasties.rs`; render
[S16a](renders/S16a.txt)), from variant 6, round 4 of the throwaway
`top-dynasties-r4.html` mockup. The implementation plan is
[../top-dynasties-plan.md](../top-dynasties-plan.md). The dynasty and member lists are
[Tables](../components/table.md), the six race charts are the
[Race Chart](../components/race-chart.md) component, and the bars are Bare
[Labeled Bars](../components/labeled-bar.md) with a Marker. Deviations from the mockup,
forced by the data or the sheets: the sidebar's *Kills by year* became **Kills by prey**
(the sim keeps kills per prey species, not per year, for one animal); the Survival lines
are counts (`* survived a hard winter ×1`), since the tallies carry no year; the Table
header is the sheet's dim text, not a `HEADER_BG` band; a one-cell spacer column separates
the numbers from the text columns that follow them.

## Purpose
S16 makes the interesting predators easy to follow and become invested in. It ranks
*dynasties* — a founder and every animal whose mother line leads back to it — one region
at a time, by six stats over the life of the line: kills, territory, young, survival, age
and mutations. The selected line gets a banner with an ASCII portrait and its standing in
the region and the valley, six cumulative "race" charts against the region's other
dynasties, its living members, and a sidebar for its top member. A Watch strip keeps up to
four pinned dynasties or animals reachable from any region, and the selection follows the
line's identity when the ranking shifts, so a favourite is never lost.

S04 ranks species and shows five notable individuals; S08 shows one family tree; S15 shows
what a trait does. S16 shows which bloodlines are winning, and who carries them.

## Variants
| Id   | Variant                   | When it is shown |
|------|---------------------------|------------------|
| S16a | race chart, one region    | On entry with `d` from the map or any screen that lets the global table through. Opens on region 1, the first line by kills, focus on the dynasty list. |

## Layout
A full-screen data screen. On a 155×45 terminal the body is rows 0–43 and the status bar is
row 44; the test harness and the render draw it in a 44-row area, which costs one member
row (see item 14).

| Panel | Position | Size | Border |
|-------|----------|------|--------|
| Watch | cols 0–154, rows 0–2 | 155 × 3 | Outer. Right-hand info `<n> pinned · [p] pin the selection · [1-4] jump` |
| Top dynasties by region | cols 0–111, rows 3–43 | 112 × 41 | Focus. Info `Year <y>, day <d> · kills lead the ranking` |
| Top member | cols 112–154, rows 3–43 | 43 × 41 | Outer |
| Status bar | row 44 | 155 × 1 | — |

Main panel, inner rows (inner row 0 is screen row 4, inner columns from the left border):

| Inner row | Content |
|-----------|---------|
| 0 | Region line: `◄ <Region> ►  region <i>/8 · <n> dynasties · <counts> living` (or `the whole valley · <n> dynasties in 8 regions` on the All stop), then the species [Filter Strip](../components/filter-strip.md) from column 76: `all`, one chip per predator species, hint `[s]` |
| 1 | Dynasty table header |
| 2–5 | Four dynasty rows, the region's best four by kills (fewer when the region has fewer) |
| 6 | Divider `<Line> · founded Y<y> by <founder> · <g> generations · <e> members ever, <l> living` |
| 7–13 | Portrait (columns 1–15) beside the standing line (row 7) and six stat bars (rows 8–13) |
| 14 | `carried by <top member> · <k> of <K> kills · <t> of <T> cells · age <age> · gen <g>` |
| 15 | `♦ pinned · [p] unpins` or `[p] pins this line to the Watch strip`, then `│ valley mean of the species' lines` at column 62 |
| 16 | Divider `Race since Y1` with the legend `■ <Line>  ∙ <rival>  ∙ <rival>  ∙ <rival>` in species colours |
| 17–23, 24–30 | Six race charts, 35 × 7 each, three per row at columns 1, 37, 73: kills, territory, young over survival, age, mutations |
| 31 | `totals: kills, young, survival, mutations · year-end levels: territory = cells the living hold, age = oldest` |
| 32 | Divider `Living members · <l>, the top <n> by kills` (Focus colour when the member list has focus) |
| 33 | Member table header |
| 34–38 | Member rows: `inner height − 34`, clamped to 1..=5 (five on a 45-row terminal, four in the 44-row harness) |

```
╔ Watch ═══════════════════════════════════════════════════════════════════════════════════════════════════ 0 pinned · [p] pin the selection · [1-4] jump ╗
║ nothing pinned yet · [p] pins the selected dynasty or animal, it then stays selected when the ranking shifts                                            ║
╚═════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════╝
╔ Top dynasties by region ═════════════════════════════════════════════ Year 2, day 1 · kills lead the ranking ╗╔ Top member ═════════════════════════════╗
║ ◄ Western Forest ►  region 1/8 · 4 dynasties · 1 foxes, 2 wolves, 1 lynxes    all F fox W wolf L lynx    [s] ║║ W Fenrir w#953                   ♂ wolf ║
║     #dynasty        sp  since  gens  living   kills   terr  young  surv   age  muts carried by     region    ║║ carries the Cinder line                 ║
║ ►  1 Cinder line    W      Y1     2       2      63     24      4     2    0y     1 Fenrir w#953   W. Forest ║║ ♥ living · age 0y 294d · gen 2          ║
║    2 Rook line      W      Y1     2       3      59     49      2     3    1y     1 Rook w#520     W. Forest ║║                                         ║
║    3 Ember line     L      Y1     2       2      33     33      2    22    1y     0 Ember l#528    W. Forest ║║     ▄▄     ▄▄     kills       21 #5/11  ║
║    4 Dusk line      F      Y1     1       1      19     18      0    12    1y     0 Dusk f#513     W. Forest ║║     █▀█▄▄▄█▀█     territory   10 #11/11 ║
║─ Cinder line · founded Y1 by Cinder w#523 · 2 generations · 3 members ever, 2 living ────────────────────────║║    ▐█ ■ ▄ ■ █▌    young        2 #2/11  ║
║    ▄▄     ▄▄     standing   #1 of 4 dynasties in the Western Forest · #2 of 5 wolf lines in the valley       ║║     █▄▄▄▀▄▄▄█     survival     1 #1/11  ║
║    █▀█▄▄▄█▀█     kills        63 [████████████│█░░░░░░░░░░] region #1/4 · valley #2/5 · mean 55 · best 107   ║║      ▀██▄▄██▀     age         0y #6/11  ║
║   ▐█ ■ ▄ ■ █▌    territory    24 [████████░░░│░░░░░░░░░░░░] region #3/4 · valley #3/5 · mean 36 · best 76    ║║        ▀██▀       mutations    0 #7/11  ║
║    █▄▄▄▀▄▄▄█     young         4 [██████████████│█░░░░░░░░] region #1/4 · valley #2/5 · mean 4 · best 6      ║║         ▀▀        [p] pin               ║
║     ▀██▄▄██▀     survival      2 [████████████░│░░░░░░░░░░] region #4/4 · valley #3/5 · mean 2 · best 4      ║║                                         ║
║       ▀██▀       age          0y [██████████░░░░│░░░░░░░░░] region #4/4 · valley #5/5 · mean 1y · best 2y    ║║─ Against living wolves ─────────────────║
║        ▀▀        mutations     1 [█████░│░░░░░░░░░░░░░░░░░] region #1/4 · valley #2/5 · mean 1 · best 5      ║║ kills    [████████████│██░░░]   top 46% ║
║                  carried by Fenrir w#953 · 21 of 63 kills · 10 of 24 cells · age 0y 294d · gen 2             ║║ territory[████████░░░░│░░░░░]  top 100% ║
║                  [p] pins this line to the Watch strip      │ valley mean of the species' lines              ║║ young    [█████│███░░░░░░░░░]   top 19% ║
║─ Race since Y1 ── ■ Cinder line  ∙ Rook line  ∙ Ember line  ∙ Dusk line ─────────────────────────────────────║║ survival [█████████████████│]   top 10% ║
║ kills ■63 ∙59 ∙33 ∙19               territory ■24 ∙49 ∙33 ∙18           young ■4 ∙2 ∙2 ∙0                    ║║ age      [███████│░░░░░░░░░░]   top 55% ║
║ │■─■                                │∙─∙                                │■─■                                 ║║ mutations[░░░░░│░░░░░░░░░░░░]   top 64% ║
║ │                                   │∙─∙                                │                                    ║║ │ species mean · bar = share of the best║
║ │∙─∙                                │■─■                                │∙─∙                                 ║║                                         ║
║ │∙─∙                                │∙─∙                                │                                    ║║─ Kills by prey ─────────────────────────║
║ │                                   │                                   │∙─∙                                 ║║         ███                             ║
║ └1───────────────────────────────── └1───────────────────────────────── └1─────────────────────────────────  ║║ ··      ███                             ║
║ survival ■2 ∙3 ∙22 ∙12              age ■0y ∙1y ∙1y ∙1y                 mutations ■1 ∙1 ∙0 ∙0                ║║ V   H   D                               ║
║ │∙─∙                                │∙─∙                                │■─■                                 ║║ 0   2   19                              ║
║ │                                   │∙─∙                                │                                    ║║                                         ║
║ │∙─∙                                │■─■                                │                                    ║║─ Territory ─────────────────────────────║
║ │∙─∙                                │                                   │                                    ║║ 10 cells · 41% of line · won 0 lost 0   ║
║ │■─■                                │                                   │∙─∙                                 ║║                                         ║
║ └1───────────────────────────────── └1───────────────────────────────── └1─────────────────────────────────  ║║─ Survival ──────────────────────────────║
║ totals: kills, young, survival, mutations · year-end levels: territory = cells the living hold, age = oldest ║║ * survived a hard winter ×1             ║
║─ Living members · 2, the top 2 by kills ─────────────────────────────────────────────────────────────────────║║                                         ║
║   name     tag    sex   gen  kills  terr  young  surv   age  muts kills vs the living                        ║║─ Genetics ──────────────────────────────║
║ · Fenrir   w#953  ♂      g2     21    10      2     1    0y     0 [██████████████│█░░░░]                     ║║ no mutation at birth                    ║
║   Vex      w#952  ♀      g2     19    14      0     1    0y     1 [██████████████│░░░░░]                     ║║                                         ║
║                                                                                                              ║║                                         ║
║                                                                                                              ║║ [f] follow on map  [l] lineage          ║
╚══════════════════════════════════════════════════════════════════════════════════════════════════════════════╝╚═════════════════════════════════════════╝
 [←→] region  [↑↓] pick  [Tab] members  [s] species  [p] pin  [n] name  [f] follow  [l] lineage  [Esc] back              W. Forest · Cinder line · dynasties

```
*S16a: seed 7 after one year, region 1, its first line by kills, the dynasty list focused, nothing pinned. The 44-row harness shows the member rows the height leaves; two lines live in this one.*

**Watch item anatomy.** Slots of `⌊153 / n⌋` cells from inner column 0, a `│` in the last
cell of every slot but the last. Inside a slot: the digit at 0 in the key colour, `♦` at 2
in the accent colour, then from 4 the label cut at word boundaries (` · ` first) to the
slot width minus 7: `<Line> · <kills> kills · <region short> · #<r> in region` for a
dynasty, `<Name> <tag> · <kills> kills · <region short> · top <p>% of <plural>` for an
animal. The slot whose target is the current selection is filled with the selection
background.

**Dynasty row anatomy.** Inner column 0 holds `♦` on pinned lines. The Table starts at
column 1: its Marker cell (`►` when the dynasty list has focus, `·` when it does not, blank
on other rows), then `#` right-aligned in 3, the line name padded to 15, the species glyph
in its colour, `since` (`Y<y>`) in 6, `gens` 6, `living` 8, `kills` 8, `terr` 7, `young` 7,
`surv` 6, `age` 6 (`<y>y`), `muts` 6, `carried by` (`<Name> <tag>` in the species colour)
15, the region's short name cut to 10.

**Member row anatomy.** As the dynasty row: `♦` at column 0 for a pinned animal, Marker,
then `name` 9, `tag` 7, `sex` 4, `gen` 5 (`g<n>`), `kills` 7, `terr` 6, `young` 7, `surv`
6, `age` 6, `muts` 6, then a 22-cell bracketed bar of the animal's kills as a share of the
best living animal of its species with a `│` marker at the species mean, and the tail
`top <p>% · #<r> of <n>`.

## Content requirements

### Watch strip
1. **Pins.** Up to four items, in the order they were pinned, from `AppState.pins`. Each
   is a dynasty (by root id) or an animal (by creature id). A pin whose target no longer
   exists (the line was dropped, the animal died and was pruned) is drawn dim as
   `<n> ♦ <label> · gone` and its digit does nothing.
2. **Empty strip.** `nothing pinned yet · [p] pins the selected dynasty or animal, it then
   stays selected when the ranking shifts`.

### Top dynasties by region
3. **Region cycle.** Eight regions in `World.regions` order, then an *All regions* stop
   whose list is the valley's best four. The region line names the region, its index, how
   many dynasties have a living member there, and the living predators by species
   (`6 foxes, 17 wolves, 4 lynx`), cut at column 74.
4. **Dynasty.** A founder plus every animal whose mother chain leads to it
   (`LineageNode.root`). A line is listed in the region where the plurality of its living
   members stand (lowest region index on ties). Lines with no living member are kept for
   ten years and never listed; the All stop lists every line with a living member.
5. **Ranking.** Kills, then young, then founded day, then root id. The species filter
   (`s`) restricts the list to one predator species. Only predator lines are shown; a
   roster with no predators shows every line.
6. **Six stats.** For a line, `kills`, `young`, `surv`, `muts` are running totals over
   every member ever, dead members included (folded at death, so lineage pruning cannot
   shrink them); `terr` is the sum of scent-grid cells its living members hold at or above
   `territory.hold_min`; `age` is its oldest living member. For an animal the same six are
   its own: kills, cells held, offspring, survival events, age, mutations at birth.
   Survival events = infections survived + predator escapes + contests won + droughts
   survived + hard winters survived (every winter counts; the sim has no winter severity).
7. **Banner.** The divider names the line, its founder (name and tag as recorded at birth),
   founded year, generations (max − root generation + 1), members ever and living. The
   portrait is chosen by the species' adult glyph letter (`F`, `W`, `L`) with a generic
   predator or prey portrait for any other roster; portraits are 15 × 7 block-glyph art in
   the species colour with `■` eyes in the key colour.
8. **Standing.** `#<r> of <n> dynasties in the <Region>` by kills, and `#<r> of <n>
   <species> lines in the valley`. Each stat bar: label padded to 10, value right-aligned
   in 4 (`age` as `<y>y`), a 24-cell bar of the value over the valley's best line for that
   stat with a `│` marker at the valley mean, then `region #<r>/<n> · valley #<r>/<n> ·
   mean <m> · best <b> (<who>)`, the parenthesis dropped when it does not fit.
9. **Carried by.** The line's living member with the most kills (then id): its share of
   the line's kills and cells, its age and generation.
10. **Race charts.** Six charts, one per stat in order. The x axis is years since Y1, two
    cells per year, the last `⌊(w − 1) / 2⌋` years shown; the selected line is the bold
    `■` series drawn last, the region's (or valley's) next three lines by kills are dim `∙`
    series. Totals plot the line's per-year rows (closed years) followed by its current
    total; levels plot the year-end value. The title row gives each series' last value.
11. **Note.** Row 31 spells out which stats are totals and which are levels.
12. **Members.** The line's living members by kills; the divider says how many live and how
    many rows show. The bar compares the animal's kills with every living animal of its
    species: fill = kills / best, marker = mean / best, tail `top <p>% · #<r> of <n>` where
    `p = ⌈100·r / n⌉`.
13. **Empty region.** A region with no dynasty shows `no dynasties match the filter` in the
    first table row; the banner, charts and members are not drawn and the sidebar reads
    `nothing selected`.
14. **Height.** The main panel has 34 fixed inner rows; member rows are what remains,
    clamped to 1..=5. The S16a render, drawn in a 44-row area, shows four.

### Top member (inner width 41, text at 1)
15. **Subject.** The selected member when the member list has focus, else the carrier.
16. **Head.** Species glyph and `<Name> <tag>`, `<sex> <species>` right-aligned; `carries
    the <Line>` (accent) or `of the <Line>, #<r> by kills`; `♥ living · age <y>y <d>d · gen
    <g>`.
17. **Ranks.** The portrait at columns 3–17, rows 5–11; beside it six lines `<stat>
    <value> #<r>/<n>` among living animals of the species, then `♦ pinned` or `[p] pin`.
18. **Against living <plural>.** Six 18-cell bracketed bars, fill = value / best living,
    marker at the species mean, `top <p>%` right-aligned (good colour from the top 10 %);
    then `│ species mean · bar = share of the best`.
19. **Kills by prey.** A two-row [Histogram](../components/histogram.md) of the animal's
    kills per prey species (`col_w(4)`), the prey glyphs and counts beneath; a species it
    never killed shows `··`.
20. **Territory.** `<t> cells · <p>% of the line · won <w> lost <l>`.
21. **Survival.** Up to three lines from the tallies, in this order and only when
    non-zero: `☺ gained immunity ×n`, `» escaped a predator ×n`, `¡ survived a drought ×n`,
    `* survived a hard winter ×n`, `► won a contest ×n`; `nothing survived yet, nothing
    lost` when all are zero.
22. **Genetics.** `§ <Trait> <±.nn> at birth (g<n>)` per mutation, or `no mutation at
    birth`; then `inherited: <Trait> <±.nn>, …` from the line's earlier mutations.
23. **Footer** on the last inner row: `[f] follow on map  [l] lineage`.

## State
- The screen remembers `region`, the species filter, which list has focus, the selected
  dynasty **by root id** and the selected member by creature id. When the ranking shifts,
  the selection stays on the same line; when the line leaves the region's list, the region's
  first line is selected. Nothing survives `Esc`; reopening starts on region 1.
- Pins live on `AppState` for the session and survive `Esc`; they are not saved.
- Rankings and standings are rebuilt once per sim day (the same cache rule as S15).

## Glyphs and colors
- `♦` (`glyphs::DIAMOND`) pin mark; `►` (`PLAY`) focused cursor, `·` (`DOT`) unfocused
  cursor; `◄ ►` (`REWIND`, `PLAY`) around the region name.
- Race charts: `■` (`SQUARE`) lead points, `∙` (`TRAIL`) rival points, `─ │` and the four
  box corners `┌ ┐ └ ┘` (`glyphs::BOX_TL/TR/BL/BR`) for connectors and the foot.
- Portraits: `▀ ▄ █ ▐ ▌ ▲` and `■` eyes.
- Survival: `☺` (`IMMUNE`), `»` (`CUE`), `¡` (`DROUGHT`), `*` (`SNOW`); `§` (`MUTATION`)
  genetics; `♥` (`BIRTH`) living; `♂ ♀` sex.
- Colours: species colours from the roster for names, glyphs, bars and lead series; rivals
  in `theme::dim(species, 0.55)`; `KEY` for digits and eyes; `ACCENT` for `♦` and the
  carries line; `SELECT_BG` on the selected row and slot; `BORDER_FOCUS` on the members
  divider when focused; `GOOD` for living and top-10 % percentiles; `INFO` for `§`;
  `SICK`, `WARN`, `INFO` for `☺`, `¡`, `*`.

## Components
| Part | Component | Differences from the mockup |
|---|---|---|
| Panels | [Panel](../components/panel.md) (Outer, Focus, Outer) | — |
| Dividers, section rules | [Divider](../components/divider.md) | The race legend and the focus colour are drawn over the rule after it; Divider has no colour option (gap) |
| Species chips | [Filter Strip](../components/filter-strip.md) | Chips show the roster glyph; hint `[s]` |
| Dynasty and member lists | [Table](../components/table.md) | The header is the Table's dim header, not the mockup's `HEADER_BG` band; the Table owns the Marker column, so `♦` is drawn one cell to its left by the screen; a one-cell spacer column sits between a numeric column and the text column after it |
| Stat bars, member bars, sidebar bars | [Labeled Bar](../components/labeled-bar.md), Bare with Marker | Label, value and suffix are laid out by the screen around a `Bar` |
| Race charts | [Race Chart](../components/race-chart.md) | New component; the sheet is the spec |
| Kills by prey | [Histogram](../components/histogram.md) | `rows(2)`, `col_w(4)`; labels drawn by the screen |
| Lines | [Text](../components/text.md) | — |
| Status row | [Status Bar](../components/status-bar.md) | — |
| Watch slots, portraits | drawn into the buffer | No component |

## Interaction
| Key | Action | Goes to |
|-----|--------|---------|
| `←` `→` | previous / next region; the ninth stop is All regions (wraps) | — |
| `↑` `↓` | move the cursor in the focused list (wraps) | — |
| `Tab` | move focus between the dynasty list and the member list | — |
| `s` | cycle the species filter: all → each predator species → all | — |
| `p` | pin or unpin the focused dynasty or member (four at most) | — |
| `1`–`4` | jump to that pin: its region, the line, and the member when the pin is an animal | — |
| `f` | follow the sidebar's subject on the map | [S01e](s01-world-map.md) |
| `l` | lineage of the sidebar's subject | [S08](s08-lineage.md) |
| `n` | name the focused dynasty (list focus) or member (member focus): the rename modal of [S03](s03-creature-inspector.md#naming). A dynasty name is up to 20 characters and replaces `<Founder> line` everywhere the line is named; an empty name restores it | rename modal |
| `Esc` | back to the screen S16 was opened from | [S01](s01-world-map.md) |

`s`, `p` and `n` are consumed here (the global `s` Species Browser and `p` Controls are
reachable after `Esc`), as S04 consumes `s` for sort. Every other key falls through to the
global table. The status bar reads `[←→] region  [↑↓] pick  [Tab] members  [s] species
[p] pin  [n] name  [f] follow  [l] lineage  [Esc] back`, with `<Region short> · <Line> ·
dynasties|members` on the right, the line cut to 16 cells. `1-4` is hinted in the Watch
title only: the row has no room for it beside `n` and `Esc`.

**Effects on other screens:**
- `d` is added to the global table next to `t`.
- The S01 sidebar's Notable row becomes ` [s] species · [t] traits · [d] dynasties`.
- The S11 Screens group gains `d  top dynasties`.
- `f` sets `AppState.follow` exactly as S03's `f` does and pops S16; the map follows once it
  is the top screen again.

## States and edge cases
- **New world.** Every founder is its own line with one member and no closed year; charts
  show a single point; `carried by` is the founder.
- **A line founded mid-history** (a newborn whose mother's node was already pruned) starts
  its own dynasty with `founded Y<n>`; it is listed like any other.
- **Extinct line.** Not listed; a pin to it reads `· gone`. Its record is dropped ten years
  after its last member died.
- **Small rosters and filters.** Fewer than four lines fill fewer rows; a filter that
  matches nothing shows item 13's message.
- **Long names.** Line names are `<Founder> line` cut to 15; region names to 10 in the
  table; watch labels cut at ` · ` boundaries.
- **Paused simulation.** The screen is read-only; the day cache is refreshed only when the
  day changes.

## Open questions
- After `f`, S01e's `Tab` and `n` change the followed creature, so the followed member is
  easy to lose; pushing S16 over the follow instead of popping would keep it, at the cost
  of the map not being visible.
- Whether prey lines deserve their own view: the root is stored for every species, the
  store keeps predators only.
- A colour option on Divider would remove the re-draw of the members divider.
