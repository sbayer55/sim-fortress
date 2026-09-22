# S17 — Hunt Watch

Back to the [screen overview](README.md).

Status: **built** (Sept 2026, `src/ui/screens/s17_hunts.rs`; renders [S17a](renders/S17a.txt),
[S17b](renders/S17b.txt)), from variant 7d ("hunter | ribbon | prey") of the throwaway
`hunt-watch-mockup.html` mockup (Sept 2026, twelve variants over four rounds).
The implementation plan is [../hunt-watch-plan.md](../hunt-watch-plan.md). The ribbon is a
new [Ribbon](../components/ribbon.md) component; the energy bars are Bare
[Labeled Bars](../components/labeled-bar.md); the rest is [Panel](../components/panel.md),
[Divider](../components/divider.md), [Text](../components/text.md) and the
[Status Bar](../components/status-bar.md). Two deviations from the mockup are forced by the
sim: a fifth hunt outcome, *dropped* (the hunter's replan switched goals mid-hunt, which the
sim does not count as an attempt), and the "kin" line, which shows the prey's `kin_nearby`
count rather than named fawns. Two more from the build: the S01 Notable block had no spare
row, so `[h] hunt watch` replaces the blank row under it rather than joining the second
row; and the odds parts line omits parts that are zero (no pack, no sickness).

## Purpose
The map shows a hunt as two letters converging. S17 shows the hunts as contests: for every
predator that is hunting, or has just failed and is about to try again, one row tells who is
chasing whom, where, how the chase is going tick by tick, whether the hunter needs the kill,
and how much of a challenge the prey is. The player uses it to watch several hunts at once,
to see a failed hunter through its cooldown to its next attempt or its starvation, to pin the
animals worth watching (pins are shared with the map, the inspector and S16's Watch strip),
and to jump to the map or the inspector for one of them.

## Variants
| Id   | Variant                          | When it is shown |
|------|----------------------------------|------------------|
| S17a | lanes with the details panel     | On entry with `h`. Up to five lanes, the details of the selected row below. |
| S17b | lanes only                       | `d` from S17a. Up to eight lanes, no details panel; `d` again returns. |

## Layout
A full-screen data screen. On a 155×45 terminal the body is rows 0–43 and the status bar
row 44; the test harness and the render draw it in a 44-row area, which costs the details
panel its last row (S17a) or the eighth lane its key line (S17b). The layout derives from the
area height: lanes fill from row 1, the Details divider sits at `1 + 5·lanes`, the key line
is the last body row.

| Panel | Position | Size | Border |
|-------|----------|------|--------|
| Chase Lanes | cols 0–154, rows 0–43 | 155 × 44 | Focus. Info `<n> hunting · <m> tracked · Y<y> D<d> <hh>:00 <☼ day│○ night>[ · ♦ <k> pinned]` |
| Status bar | row 44 | 155 × 1 | — |

Inside the panel (inner columns from the left border, so inner column 3 is screen column 3):

| Rows | Content |
|------|---------|
| 1–25 (S17a) / 1–40 (S17b) | Lanes, five rows each, in display order (pinned first); a slot with no hunter reads `row <n> · free` on its first row on a dim ground |
| 26 (S17a) | Divider `Details · <hunter> ► <prey>` or `Details · <hunter> · <state>` |
| 27–41 (S17a) | Details panel, three columns at columns 3, 54 and 104 (widths 49, 48, 49) |
| 42 | Key line: `key: ribbon: the rope marker per chase tick, ESCAPE top, KILL bottom, ┤ the clock out; grey = stalk · ♦ pinned: row held` |

**Lane anatomy** (lane rows 0–4, `y0` the lane's first row):

- Row 0 is filled with `theme::lerp(PANEL_BG, HEADER_BG, .6)` across columns 1–153
  (`SELECT_BG` on the selected lane); `»` (`glyphs::CUE`) at column 1 on the selected lane;
  the hunter `<G> <Name> <tag>` at column 3 in its species colour, bold, followed by ` ♦`
  when pinned; the prey `<g> <Name> <tag>` at column 117 in its species colour, bold, or
  `no prey` (dim) / `dead` (`BAD`) between hunts.
- Rules `│` (`V_LINE`, `BORDER`) at columns 39 and 116 on rows 0–4.
- Hunter column, columns 3–37, rows 1–4, labels at 3 in `dim_text()`, values at 10:
  `energy` + a Bare Labeled Bar 27 wide in the species colour (no value);
  `need` + the need word; `state` + the state word, then ` · pack of <n>` in `MAGENTA` when
  packmates hunt the same prey; `where` + the region name.
- Ribbon, columns 41–114, rows 0–4: the [Ribbon](../components/ribbon.md) at two cells per
  tick with six stalk ticks (columns 41–52 on the stalk ground), the clock-start rule `│`
  in `WARN` at column 53, thirty chase ticks (columns 54–113 on the chase ground, the top
  row tinted toward `GOOD`, the bottom toward `BAD`), the clock-out `┤` (`CLOCK_OUT`,
  `BAD`) at column 114, and `10` / `20` in `dim_text()` on the bottom row at columns 74
  and 94. Between hunts the last hunt's ribbon is drawn dimmed (every colour lerped .45
  toward `PANEL_BG`) with the state sentence centred on row 2.
- Prey column, columns 117–152, rows 1–4, labels at 117, values at 124. Hunting:
  `energy` bar (27, species colour), `level` + the challenge word, `goal` + `fleeing` /
  `wary` (`WARN`) or `grazing, unaware` (`GOOD`), `gap` + `<n>  <G><·×(n−1)><g>` (the gap
  literal, `BAD` bold at 2 or less) and, in Chase, `<left> left` at column 138 (`BAD` bold
  at 6 or less). Between hunts: `no prey` / `dead`, `last: <prey>`, `taken|escaped|
  outlasted|lost|dropped <hh>:00` (`BAD` for taken, `GOOD` otherwise), then the last beat
  wrapped to two rows in `dim_text()`.

Cell-exact render from the mockup (rows 0–25 of S17a at a busy tick; two live hunts, one
cooldown, one stalk, one free slot):

```
╔ Chase Lanes · Scoreboard · hunter | ribbon | prey ═══════════════════════════════════════════════════════ 3 hunting · 4 tracked · Y4 D213 21:00 ○ night ╗
║» W Greymaw w#042                     │             │                                                            ┤ │D Thistle d#133                      ║
║  energy [██████████░░░░░░░░░░░░░░░]  │             │                                                            ┤ │energy [██████░░░░░░░░░░░░░░░░░░░]   ║
║  need   HUNGRY                       │             │                                                            ┤ │level  EASY PREY                     ║
║  state  CHASE · pack of 3            │             │            ■─■─■─■─■─■─■─►─                                ┤ │goal   fleeing                       ║
║  where  Rowan Ridge                  │             │                    10                  20                  ┤ │gap    2  W·D        17 left         ║
║  F Ember f#017                       │   ■─        │                                                            ┤ │V Clover v#412                       ║
║  energy [██████████████████░░░░░░░]  │     ■─■─■─■─│                                                            ┤ │energy [████████████████████░░░░░]   ║
║  need   HUNGRY                       │             │■─                                                          ┤ │level  EASY PREY                     ║
║  state  CHASE                        │             │  ■─►─                                                      ┤ │goal   fleeing                       ║
║  where  Fern Hollow                  │             │                    10                  20                  ┤ │gap    3  F··V       28 left         ║
║  W Rime w#051                        │             │                                                      ■─·─  ┤ │no prey                              ║
║  energy [██░░░░░░░░░░░░░░░░░░░░░░░]  │             │                                              ■─■─■─■─      ┤ │last: H Sorrel h#288                 ║
║  need   DESPERATE                    │             │        hunt cooldown: 5 h before Rime can hunt again       ┤ │lost 20:00                           ║
║  state  COOLDOWN 5h                  │             │                                                            ┤ │Rime idles a tick, then patrols on   ║
║  where  Sedge Fen                    │             │                    10                  20                  ┤ │                                     ║
║  L Sable l#003                       │             │                                                            ┤ │H Bramble h#217                      ║
║  energy [██████████████████░░░░░░░]  │             │                                                            ┤ │energy [█████████████████████░░░░]   ║
║  need   HUNGRY                       │   ■─■─■─■─►─│                                                            ┤ │level  CHALLENGE                     ║
║  state  STALK                        │             │                                                            ┤ │goal   grazing, unaware              ║
║  where  Hazel Wood                   │             │                    10                  20                  ┤ │gap    5  L····H                     ║
║  row 5 · free                                                                                                                                           ║
║                                                                                                                                                         ║
║                                                                                                                                                         ║
║                                                                                                                                                         ║
║                                                                                                                                                         ║
║─ Details · W Greymaw w#042 ► D Thistle d#133 ───────────────────────────────────────────────────────────────────────────────────────────────────────────║
```

The details panel and status bar of the same frame (rows 27–44; the mockup's free-text
`Deer in Rowan Ridge: 41 · one other wolf hunt within 20 cells` line has no data source and
is dropped):

```
║  Hunter                                             Prey                                              Odds at contact 73%                               ║
║  W Greymaw w#042  wolf adult ♂                      D Thistle d#133  deer adult ♀                     ████████████████████████████████████│▒▒▒▒▒▒▒░░░░░ ║
║  hunger [█████████████░░░░░]  74%  HUNGRY           speed edge: slower by .12 (.66 vs .78)            base +.35 · speed +.12 · aggression +.26 · prey   ║
║  speed .78  aggression .88  sense 8 cells           ground: Rowan Ridge, grass · cover 0.8 x camo     size -.16 · pack +.16 = 73%                       ║
║  kills 11 of 27 attempts (41%) · avg chase 11,      .28 = .22 vs sense 8: seen                        escape by gap 14% · clock 43% · legs 40%          ║
║  longest 24 (Y3)                                    record: escaped 3 of 4 chases · size .82          ─ Play by play ────────────────────────────────── ║
║  last 12 · ■ ■ · ■ · · ■ ■ · · ■  6 kills           stamina: 7 flight ticks vs 17 on the clock:        -7 Greymaw is already on Thistle: 3 cells, 6     ║
║  last kill: Rowan d#119, 3 days ago, Rowan Ridge    tiring                                                ticks on the clock                            ║
║  pack of 3: Snarl w#047, Howl w#049 (+.08 each to   challenge: EASY PREY at 73% odds                   -5 Greymaw closes to 2: a lunge away             ║
║  the roll)                                          kin: 2 nearby                                      -4 Thistle opens the gap to 3                    ║
║  reserves: 28 chase ticks in the legs vs 17 on the  goal: fleeing, 8 cells at double energy cost       -3 Greymaw closes to 2: a lunge away             ║
║  clock: can finish                                                                                     -1 Thistle keeps to the grass                    ║
║  other prey in range: 3 · hungry enough to hunt                                                                                                         ║
║                                                                                                                                                         ║
║                                                                                                                                                         ║
║  key: ribbon: the rope marker per chase tick, ESCAPE top, KILL bottom, ┤ the clock out; grey = stalk · ♦ pinned: row held                              ║
╚═════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════╝
 [↑↓] row  [p] pin  [d] details  [Enter] follow  [i/o] inspect  [Space] pause  [.] step  [-/=] speed  [Esc] back         21:00 ○ · tick 1,064,779 · ►► x2
```

## Content requirements

### Rows
1. **One row per predator.** A predator joins the board when its first hunt trace opens
   (`sim.hunts`, see [../hunt-watch-plan.md](../hunt-watch-plan.md) D1) and takes the lowest
   free slot; it keeps that slot while it hunts, eats, cools down, rests, patrols and hunts
   again, until it has eaten and rested four ticks after `eat_until` (it leaves the board),
   or it has been dead four ticks. A pinned predator keeps its slot while sated or dead.
   Slots never shift; free slots read `row <n> · free`.
2. **Display order** is pinned slots first (in slot order), then the others, then free slots.
   Eight slots exist; S17a shows the first five in display order and, when more are
   occupied, `+<n> more: hide details` in `WARN` at column 120 of the key line.
3. **State word** (hunter column): `STALK` / `CHASE` (`INFO` / `WARN`) while hunting;
   `KILL` (`BAD`), `MISSED`, `TIMED OUT`, `LOST` (`GOOD`), `DROPPED` (`dim_text()`) on the
   tick a hunt resolves; then `EATING` (`BAD`) until `eat_until`, `FED <n>h` (`GOOD`) for
   four ticks, `SATED` (`GOOD`, pinned only) after; after a failure `COOLDOWN <n>h`
   (`dim_text()`) while `tick < hunt_cooldown_until`, `RESTING` (`INFO`) while the goal is
   Rest, else `PATROL` (`TEXT`); `STARVED` (`BAD`) for a dead hunter, with the cause word
   from `Cause::label()` when it is not starvation.
4. **Need word**: `DESPERATE` (`BAD`) at hunger ≥ .80, `HUNGRY` (`WARN`) ≥ .60, `IN NEED`
   (`TEXT`) ≥ `predation.hunt_hunger_min`, else `NOT HUNGRY` (`dim_text()`).
5. **Challenge word**: from the kill odds at contact (item 9): `EASY PREY` (`GOOD`) ≥ .70,
   `FAIR CHASE` (`TEXT`) ≥ .50, `CHALLENGE` (`WARN`) ≥ .35, else `LONG SHOT` (`BAD`). Read
   from the hunter's side: green is good for the hunter.
6. **Energy bars**: the hunter's and the prey's `energy`, 0–1, in their species colours.
7. **Gap**: `geom::cheb` between hunter and prey this tick; the literal is the hunter's
   adult glyph, `gap − 1` dots (`DOT`), the prey's glyph, capped at 9 dots. `left` is
   `chase_max_ticks − (tick − chase start)` in Chase.

### Ribbon
8. **Samples.** Each open hunt is sampled once per tick after movement (gap, phase, both
   energies, hunter hunger, the prey's goal and terrain). The ribbon shows the last six
   stalk ticks right-aligned to the clock rule and the chase ticks from the trace's own
   first Chase tick (the sim re-plans a hunt every six ticks and resets
   `chase_start_tick`; the trace keeps the first one, so the clock axis is honest).
9. **The tug value** `p` per sample, a UI estimate documented here, 0 = the prey is getting
   away, 1 = the kill is on: `gap01 = (gap − 1) / (sense_cells − 1)`, `clock01 = chase tick /
   chase_max_ticks`, `legs01 = 1 − (energy − .05) / (energy at chase start − .05)`, all
   clamped to 0..1; `K = 1` at contact else `1 − gap01`; `E = max(gap01, clock01, legs01)`;
   `p = clamp(.5 + .5 (K − E))`. The final sample of a resolved hunt is pinned to 1 (kill)
   or 0 (miss, timeout, lost, dropped). Row = `round(p × 4)`, 0 at the top (ESCAPE), 4 at
   the bottom (KILL).
10. **Marks.** Each tick: `■` (`SQUARE`) at its first cell in `theme::lerp(GOOD, BAD, p)`,
    `─` (`H_LINE`, the same colour lerped .5 toward the ground) in its second; `│` connectors
    between consecutive rows when `p` jumps; the newest tick `►` (`PLAY`) in `TEXT_BRIGHT`
    bold while the hunt is open; the final tick of a resolved hunt shows the outcome glyph
    on a ground lerped .6 toward its colour: `x` (`DEATH`) on `BAD` for a kill, `→` (`RIGHT`)
    on `GOOD` for a miss or timeout, `·` (`DOT`) for lost and dropped.
11. **Between hunts** the last hunt's ribbon stays, dimmed, until the next hunt overwrites
    it; the state sentence is centred on the middle row on the chase ground: `hunt
    cooldown: <n> h before <Name> can hunt again`, `resting: energy <e>% and rising, hunger
    <h>% and rising`, `patrolling <region> for prey`, `feeding on <prey> for <n> more hours`,
    `fed on <prey>; leaves the board in <n>`, `sated, hunger <h>% and rising`, `<Name>
    starved <hh>:00`.

### Details panel (S17a, the selected row)
12. **Header row**: `Hunter`, `Prey` (`Last prey` between hunts), `Odds at contact <n>%`
    (`Last hunt` between hunts), in `ACCENT` bold.
13. **Hunter column** (width 49, lines wrap greedily): `<G> <Name> <tag>  <species> adult
    <♂|♀>`; `hunger` Labeled Bar 20 wide with its percent and the need word as suffix;
    `speed .nn  aggression .nn  sense <n> cells`; `kills <k> of <a> attempts (<p>%) · avg
    chase <n>, longest <m> (Y<y>)` from `kills`, `attempts`, `chase_stats`,
    `chase_longest_year`; `last 12 · <■|· per attempt, most recent first>  <n> kills` from the
    trace's attempt history; `last kill: <prey label>, <n> days ago, <region>` from
    `last_kill`; `pack of <n>: <packmates> (+.08 each to the roll)` or `hunting alone: no
    pack bonus`; `reserves: <r> chase ticks in the legs[ vs <left> on the clock: can finish|
    may tire first]` where `r = floor((energy − .05) / exertion per chase tick)`; `other prey
    in range: <n> · <hungry enough to hunt|below the hunting threshold (45%)|starvation is
    days away>` where `n` counts living prey within `sense_cells` that `predation::can_detect`
    passes.
14. **Prey column** (width 48): name line; `speed edge: <faster|slower> by .nn (.qq vs .pp)`;
    `ground: <region>, <terrain> · cover <c> x camo .nn = .nn vs sense <s>: <seen|hidden>`;
    `record: escaped <e> of <c> chases · size .nn` (or `never chased before`); `stamina: <f>
    flight ticks vs <left> on the clock: <can outlast|tiring>` where `f = floor((energy −
    .05) / (exertion × flee_energy_factor))`; `challenge: <word> at <n>% odds`; `kin: <n>
    nearby` from `kin_nearby`, or `sick with <pathogen>: +.nn to the roll` when infectious;
    `goal: fleeing, <flee_distance> cells at double energy cost | wary, edging away | grazing,
    unaware`. Between hunts the column describes the last prey and `outcome: <taken at
    contact|escaped at contact|outlasted the clock|slipped out of sense range|hunt dropped>
    <hh>:00`.
15. **Odds column** (width 49): a 49-cell stacked bar of the contact roll's parts (`█` in
    `ROCK_FG` base, `INFO` speed, `WARN` aggression, `MAGENTA` pack, `SICK` sick; the size
    penalty as `▒` in `BAD` from the right end of the positives; `│` in `TEXT_BRIGHT` at the
    clamped total; `░` dim to the end); the parts line `base +.35 · speed +.12 · aggression
    +.26 · prey size -.16 · pack +.16 = 73%` from `predation::kill_odds`; `escape by gap
    <g>% · clock <c>% · legs <l>%` (item 9's three routes) or `the hunter won at contact` /
    `the prey won` / `no chase running`; a `Play by play` Divider; then the beats.
16. **Play by play**: derived from the samples, newest last, each `<offset> <text>` with the
    offset in ticks from now (`-7`, `now`): the first sample (`<H> picks out <P> at <n> cells
    in <region>[; <P> has not noticed]` or `<H> is already on <P>: <n> cells, <m> ticks on
    the clock`), the prey's goal turning to Flee or Wary (`<P> sees <H> and bolts` / `<P>
    catches <H>'s scent and grows wary`), Stalk → Chase (`chase on: <n> cells, thirty ticks
    on the clock`), the gap closing (`<H> closes to <n>`, `: a lunge away` at 2 or less) or
    opening (`<P> opens the gap to <n>`), chase ticks 20 / 25 / 28 (`clock: <left> ticks
    left for <H>`), the outcome (`CONTACT: <H> takes <P>`, `CONTACT: <H> lunges and misses;
    <P> breaks free`, `the clock runs out: <P> outlasts <H>`, `<P> slips beyond <H>'s senses:
    lost in the <terrain>`, `<H> gives up the chase`), and after it `<H> feeds[; <packmates>
    share the kill], <n> hours` or `<P> flees <flee_distance> cells; <H> gives up for six
    hours`. Colours by kind: `TEXT` calm, `WARN` tense, `BAD` bold hot, `TEXT_BRIGHT` bold
    kill, `GOOD` bold escape, `dim_text()` aftermath. Between hunts one status beat is
    appended (`cooldown: <n> h before the next hunt`, …).

## State
- The screen remembers the selected hunter **by creature id**, the details toggle, and the
  eight lane slots. Nothing survives `Esc`; reopening allocates slots afresh in order of
  trace start, so pinned rows come first and everything else follows.
- Pins are `AppState.pins` entries of kind `Pin::Member(id)`, shared with S16's Watch strip
  (four at most, session only, not saved). Pinning a fifth animal does nothing and the
  status bar's right side says `4 pinned already`.
- Hunt traces are sim state (`Sim.hunts`), saved with the world, so a loaded world shows the
  same ribbons and the same rows.

## Glyphs and colors
- `»` (`CUE`) selected row; `♦` (`DIAMOND`) pinned; `■` (`SQUARE`) tick marks; `►` (`PLAY`)
  the newest tick; `─` `│` (`H_LINE`, `V_LINE`) connectors and rules; `┤` (`CLOCK_OUT`, new)
  the clock expiring; `x` (`DEATH`), `→` (`RIGHT`), `·` (`DOT`) outcomes; `▒ ░ █` (`SHADE_2`,
  `SHADE_1`, `BAR_FILL`) the odds bar; `♂ ♀` sex.
- Grounds: `theme::lerp(PANEL_BG, DIM, .35)` stalk zone; `lerp(PANEL_BG, INFO, .25)` chase
  zone, its top row `lerp(.., GOOD, .18)` and bottom row `lerp(.., BAD, .22)`; lane title
  rows `lerp(PANEL_BG, HEADER_BG, .6)`, `SELECT_BG` when selected; a free slot
  `lerp(PANEL_BG, HEADER_BG, .3)`.
- Species colours from the roster for names, glyphs and energy bars; `WARN` for the
  clock-start rule; `BAD` for `┤`; verdict colours as items 3–5.

## Components
| Part | Component | Differences from the mockup |
|---|---|---|
| Panel | [Panel](../components/panel.md) (Focus) | — |
| Ribbon | [Ribbon](../components/ribbon.md) | New component; the sheet is the spec |
| Energy bars | [Labeled Bar](../components/labeled-bar.md), Bare | Label drawn by the screen at the lane's label column |
| Hunger bar | [Labeled Bar](../components/labeled-bar.md) with suffix | — |
| Details / Play by play rules | [Divider](../components/divider.md) | — |
| Lines | [Text](../components/text.md) | Wrapping is a shared `common::wrap`, not a Text option (gap) |
| Status row | [Status Bar](../components/status-bar.md) | — |
| Lane title fills, rules, odds bar, gap literal | drawn into the buffer | No component |

## Interaction
| Key | Action | Goes to |
|-----|--------|---------|
| `↑` `↓` | previous / next occupied row in display order (wraps; free rows are skipped) | — |
| `p` | pin or unpin the selected hunter (`Pin::Member`, four at most) | — |
| `d` | show / hide the details panel (S17a ↔ S17b); overrides the global `d` while S17 is open | — |
| `Enter` | follow the selected hunter on the map | [S01e](s01-world-map.md) |
| `i` | inspect the selected hunter | [S03](s03-creature-inspector.md) |
| `o` | inspect the selected row's prey (the last prey between hunts, while it lives) | [S03](s03-creature-inspector.md) |
| `Esc` | back to the screen S17 was opened from | [S01](s01-world-map.md) |

Every other key falls through to the global table (`Space`, `+`/`-`, `.`, `?`, other
screens). The status bar reads `[↑↓] row  [p] pin  [d] details  [Enter] follow  [i/o]
inspect  [Space] pause  [.] step  [-/=] speed  [Esc] back` with `<hh>:00 <☼|○> · tick
<n> · <►► x<s>|││ paused>` on the right.

**Effects on other screens:**
- `h` is added to the global table next to `d`.
- The S01 sidebar's Notable block gains a third row, ` [h] hunt watch`, with ` · ♦ <n> pinned`
  appended while animals are pinned; the map draws pinned creatures bold in `theme::ACCENT`
  (the followed creature's highlight still wins when both apply).
- S03's identity header shows ` ♦` after the tag when the creature is pinned.
- The S11 Screens group gains `h  hunt watch`.
- S16's Watch strip already lists `Pin::Member` pins, so a hunter pinned here appears there.

## States and edge cases
- **No hunts yet** (a new world, or every predator fed): eight free rows, the details panel
  reads `no predator tracked: select a row when one appears`.
- **More than eight tracked predators**: the ninth and later wait off-board and take the
  next free slot; the key line counts them.
- **Same-tick open and close** (prey adjacent on the first stalk tick): the ribbon holds one
  sample with the outcome glyph.
- **The prey dies of something else mid-chase** (starvation, disease): the sim fails the hunt
  as *lost*; the prey column reads `dead`.
- **Loaded world**: rows and ribbons are restored from the saved traces; pins are not.
- **44-row harness**: see Layout; nothing overflows, the details panel loses its blank
  bottom row.

## Open questions
- Should a fed, unpinned hunter stay on the board (so a valley with five predators never
  shows a free row), or leave as specified so the board favours hunters in trouble?
- Should the tug formula weight the clock more heavily late in the chase (a 28-tick chase
  at gap 2 still reads as the hunter winning)?
- Should `Enter` also pin, so following and pinning are one gesture?
