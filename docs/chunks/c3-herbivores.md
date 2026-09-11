# C3 — Herbivores: needs, movement, perception, death

Back to the [roadmap](README.md). Previous: [C2](c2-ecology.md). Next: [C4 Evolution](c4-evolution.md).

## Goal
Introduce the first living creatures. Voles, hares and deer are placed at world start,
perceive their surroundings, choose goals from their needs, move, graze, drink, rest, age
and die. There is **no reproduction** in this chunk, so every population declines to
zero; that is intended: the checkpoint validates that needs, movement and death behave
believably before evolution complicates the picture.

## Checkpoint (what the user sees)
Letters wander over the map, cluster on meadows, walk to water when thirsty, rest at
night, starve when a region is bare, and die of old age. The player can look at a cell,
follow a creature, open its inspector, zoom into a neighbourhood, and read deaths in the
event log with a detail pane and mini-map.

## Scope

### In
- `sim::creatures`: slot storage (`Vec<Option<Creature>>` with a free list, stable
  `CreatureId(u32)` that is never reused), `Genome`, `Sex`, needs, age, death record,
  carcass decay, trail, `Goal` enum.
- `sim::geom`: the single distance function `dist(a, b) = sqrt((dx/2)² + dy²)` (ellipse
  metric, 2:1 cell aspect) used by perception, targeting, the tooltip, S03 and S13, plus
  `cheb(a, b)` (Chebyshev) for adjacency. `widgets::map` calls `sim::geom` for rings.
- `sim::spatial`: grid buckets per cell; `in_rect`, `within(center, r)` returning ids
  sorted ascending.
- `sim::behavior`: perception, goal selection with hysteresis, fractional movement,
  grazing, drinking, resting, den creation.
- Death and carcasses; daily census into `Series`; `prey_pressure`.
- Events: `DeathStarved`, `DeathThirst` (new kind: glyph `x`, WARN colour, label
  `thirst`, chip `deaths`), `DeathAge`, `Note` (den discovered). Events gain
  `subject: Option<CreatureId>`.
- `widgets::map::render` takes a `MapSource` trait (replacing C1's transitional `MapData` struct) (world cells, dens/seeds/carcasses,
  iterator of living creatures, followed/selected id) implemented by both `Fixtures` and
  `Sim`; `follow` and `Overlay::Sense` carry `CreatureId`. Carcasses render only from
  `world.carcasses`.
- Live screens: **S01c look**, **S01e follow**, **S03a/S03c inspector** (see FR12 for
  what is live vs placeholder), **S13 zoom**, **S07b** detail pane, S01 Population section,
  S02b pressure overlay, **S09 Initial species rows** (edit `initial_counts`).
- Headless CSV gains per-species daily counts and deaths by cause.

### Out
Reproduction, maturity events, predators, migration, lineage, species browser.

## Dependencies
- [C2](c2-ecology.md) complete (vegetation to eat, water to drink, `Series` slots,
  `season_metabolism`).
- Screen requirements: [S01](../screens/s01-world-map.md), [S03](../screens/s03-creature-inspector.md),
  [S13](../screens/s13-local-zoom.md), [S07](../screens/s07-event-log.md), [S09](../screens/s09-world-generation.md).
- Data model from `fixtures::creatures` (moved to `sim::creatures` with the field changes
  in FR2), species metadata from `fixtures::species::SpeciesId` (moved to `sim::species`).

## Functional requirements

### FR1 Params (`[creatures]`)
```toml
initial_counts = { vole = 240, hare = 180, deer = 90, fox = 0, wolf = 0, lynx = 0 }
adult_age_days = { vole = 30, hare = 60, deer = 180, fox = 90, wolf = 120, lynx = 120 }
hunger_base = 0.004            # per hour
hunger_per_size = 0.008        # × size
hunger_metabolism_k = 1.0      # hunger × (0.5 + k × metabolism)
thirst_per_hour = 0.012
energy_awake_per_hour = 0.008
energy_rest_per_hour = 0.05
den_rest_bonus = 1.5
graze_per_hour = 0.12          # vegetation removed from the cell
graze_nutrition = 1.5          # hunger reduced = graze_nutrition × removed
graze_min_vegetation = 0.05
drink_per_hour = 0.35
hp_loss_per_hour = 0.02        # while hunger ≥ 1 or thirst ≥ 1
hp_regen_per_hour = 0.01       # while hunger < 0.5 and thirst < 0.5
max_age_base = 200             # days
max_age_per_longevity = 900
carcass_decay_days = 6
trail_len = 12
move_speed_base = 0.5          # cells per tick = base + speed × move_speed_per_trait
move_speed_per_trait = 2.0
move_cost_energy = 0.002       # per step
replan_ticks = 6
pressure_per_creature_tick = 0.02
pressure_decay_per_day = 0.85
den_create_chance_per_rest_hour = 0.01
max_dens_per_region = 12
[stats]  series_days = 720          # event capacity is params.events.capacity (C1), raised to 5000
```
`hunger_per_hour = (hunger_base + hunger_per_size × size) × (0.5 + hunger_metabolism_k ×
metabolism) × season_metabolism[season]` — base vole ≈ 0.0060/h, deer ≈ 0.0092/h.

### FR2 Creature record
```
id: CreatureId, species, name: NameId (index into the species' name list), sex,
x, y, born_day: i32 (negative for founders; S08 shows `founder`), generation: u32 (founders = 1), parents: Option<(CreatureId, CreatureId)>,
genome: Genome([f32; 8]), hp, hunger, thirst, energy: f32, adult: bool,
goal: Goal, target: Option<(usize, usize)>, replan_at: u64, trail: VecDeque<(usize, usize)>,
alive: bool, death: Option<Death { cause, day, killer: Option<CreatureId>, chase_ticks: u16 }>,
decay: f32, mutations: Vec<Mutation { trait_idx, delta, generation }>,
// C4/C5 fields declared now, unused here: pregnant_due, cooldown_until, mother, offspring, kills,
// attempts, chased, escaped, threats_by_species, kills_by_species, last_kill, chase_stats
```
`Goal` is an enum (`Graze, Drink, Rest, Wander, Mate, Flee, Hunt, Scavenge, Migrate, Patrol`)
with `Display` giving the fixture strings ("grazing", "seeking water", "resting",
"resting in den", "wandering"). `tag()` = `<glyph>#<id>`.

### FR3 Placement
Founders are placed on walkable non-water cells, accepted with probability
`0.3 + 0.7 × vegetation`; genome = species base ± gaussian 0.12 per trait, clamped
0.02..0.98; 70 % adults with `age_days = U(adult_age, max(adult_age, 0.75 × max_age))`, juveniles
`U(0, adult_age − 1)`; hp 1.0; hunger, thirst `U(0, 0.4)`; energy `U(0.6, 1.0)`;
`generation = 1`, `parents = None`. `max_age_days = max_age_base + longevity ×
max_age_per_longevity`. `adult = age_days ≥ adult_age_days[species]`, re-evaluated at the
day boundary (juveniles become adults in C3 too; the birth side arrives in C4).

### FR4 Perception
On replan a creature collects cells and creatures with `geom::dist ≤ sense_cells()` where
`sense_cells = 2 + floor(sense × 10)`, restricted to the spatial buckets inside the
ellipse's bounding box. Full perception runs only on replan (every `replan_ticks` or when
the goal completes); C5 adds a cheap per-tick predator query.

### FR5 Goal selection (with hysteresis)
Evaluated on replan, first match wins:
1. `Drink` if thirst > 0.6; satisfied when thirst ≤ 0.1. Target = nearest water cell seen
   (adjacent land cell), else the remembered `last_water: Option<(usize, usize)>` (set on
   every drink), else Wander.
2. `Graze` if hunger > 0.5; satisfied when hunger ≤ 0.2. Target = seen cell maximising
   `vegetation / (1 + dist / 4)` among cells with vegetation ≥ `graze_min_vegetation`; if
   the current cell scores within 10 % of the best, graze in place.
3. `Rest` if energy < 0.25, or `is_night()` (diurnal species; C5 adds nocturnal ones);
   energy-triggered rest ends at energy ≥ 0.9, night-triggered rest ends at sunrise. Target = nearest den in `world.dens` within
   2 × sense range, else rest in place.
4. `Wander`: keep heading with p = 0.7, target 4–8 steps ahead on a walkable cell.
Forced rest: at energy ≤ 0 the creature rests until energy ≥ 0.3 regardless of other needs
(hunger keeps rising; it may die there).

### FR6 Movement
Per tick `move_budget += move_speed_base + move_speed_per_trait × speed` (juveniles × 0.75);
while `move_budget ≥ 1`: take one 8-neighbour step to the walkable neighbour that most
reduces `geom::dist` to the target (ties: lowest row-major index); if none reduces it,
try the two neighbours adjacent to the best direction; if still blocked mark the target
unreachable and replan. Each step costs `move_cost_energy`. Deep water and rock are
impassable for every species; creatures never block each other. Positions are pushed to
`trail` on change, capped at `trail_len`. Debug builds assert a creature never occupies an
impassable cell.

### FR7 Needs and death
Each tick: hunger += hunger_per_hour; thirst += thirst_per_hour; energy −=
energy_awake_per_hour when awake, += energy_rest_per_hour (× den_rest_bonus in a den)
when resting; grazing removes `g = min(cell.vegetation, graze_per_hour)` and reduces hunger
by `graze_nutrition × g`; drinking reduces thirst by `drink_per_hour` when 8-adjacent to
water. hp −= `hp_loss_per_hour` while hunger ≥ 1 or thirst ≥ 1; hp += `hp_regen_per_hour`
while both < 0.5. **Death**: at the day boundary if `age_days ≥ max_age_days` → `DeathAge`;
any tick `hp ≤ 0` → cause `DeathThirst` if thirst ≥ 1, else `DeathStarved` if hunger ≥ 1,
else `Injury` (unused until C5). The creature becomes a carcass (`alive = false`,
`decay` rises linearly to 1 over `carcass_decay_days`, then the slot is freed); its cell
is added to `world.carcasses` and removed when freed. The death event names the creature,
cause and region and carries `subject`.

### FR8 Dens and pressure
A resting creature on a cell without a den, with vegetation < 0.2 and terrain
Dirt/GrassDense/Forest, creates a den with `den_create_chance_per_rest_hour` while the
region has fewer than `max_dens_per_region`; emits `Note` "<name> <tag> discovered a new den
site in <region>". Each tick every living prey adds `pressure_per_creature_tick` to its
cell's `prey_pressure` (clamped to 1.0); at the day boundary all pressures ×=
`pressure_decay_per_day`. `pred_pressure` stays 0 until C5.

### FR9 Determinism
Creatures update in slot order; spatial queries return ids ascending; ties break by lower
id; no `HashMap`/`HashSet` in `src/sim` (unit test greps for them). Checksum adds every
creature's `id, x, y, hp.to_bits(), hunger.to_bits(), goal as u8`.

### FR10 Look mode (S01c)
Cursor moves 1 cell with arrows, 10 with Shift+arrows; tooltip lists creatures with
`cheb ≤ 6` columns / 3 rows (as prototype); `Enter` opens S03 for the creature on the cell
(else nearest within `cheb ≤ 1`); `f` follows it; `z` opens S13. `Esc` from a screen opened
in look mode returns to look mode with the cursor intact.

### FR11 Follow (S01e)
Viewport keeps the followed creature in the middle third; sidebar vitals live; `n` cycles
living creatures by id (`Tab` stays the sidebar toggle from C1). If the followed creature
dies the sidebar shows the corpse summary; with `ui.pause_on_follow_death` the sim pauses;
otherwise following ends after 3 simulated hours and the screen returns to S01a.

### FR12 Inspector (S03a/S03c)
| Live in C3 | Placeholder `—` until later chunk |
|---|---|
| identity, species line, sex, age bar, Family (`unknown` for founders) | Timeline beyond `born`/`adult` |
| Location: position, region, goal, target, trail length; nearest water/den by BFS ≤ 30 cells | life stats other than `days alive` |
| Vitals bars; Condition `local forage` (mean vegetation within 3 cells); `predation risk —` | Survival, Kin nearby, Offspring forecast (C4/C5) |
| Genome table vs live species mean/min/max from the daily census; Derived from sim formulas (`move speed = steps/tick`, `daily food need = hunger_per_hour × 24`) | Hunt stats (C5) |
| Mutation history (`none recorded`) | Legacy (C4) |
| Surroundings mini-map; Recent events filtered by `subject` | Scavengers, Killer (C5) |
| Death section: cause, time since death, decay bar, `meat = size × 120 × (1 − decay)`, `gone in (1 − decay) × carcass_decay_days` | |
| Behaviour: one templated line `"<goal> because <need> = <value>"` | |

### FR13 Zoom (S13)
Window centred on the look cursor; all keys in the S13 Interaction table; `Tab` there
orders in-view creatures by `geom::dist` then id; distances shown use `geom::dist`.

### FR14 Event log (S07b) and S07 `Enter`
Detail pane for the selected event with the live mini-map; `i` opens S03 if the subject
slot still exists (corpses count). `Enter` on an event with a position now opens look mode
at that cell (upgrading C2's centre-only behaviour).

### FR15 Census and S01 sidebar
Daily per-species counts, adults, juveniles, deaths by cause, and genome mean/min/max (used
by the S03 genome table) into `Series`; C4 extends this record. Population
rows show counts, 30-day sparklines and arrows (rule: change over 30 days > +3 % ↑,
< −3 % ↓, else ↔ — used by every screen). `Series` also records `drought_flags[8]` daily
so charts do not depend on events surviving the ring buffer.

### FR16 Headless CSV
Columns appended: `vole,hare,deer,fox,wolf,lynx,d_starved,d_thirst,d_age`.

## Acceptance criteria
- Headless, seed 42, default params, 1 200 days: total population is non-increasing per
  day and reaches 0 no later than day 1 100; deaths attributed to thirst, hunger and age are
  each non-zero; among deer deaths in the first 90 days at least 60 % are not `DeathStarved`
  (vacuously true if there are none).
- Movement never enters an impassable cell (debug assert + test); spatial index equals
  brute force (property test on 20 random worlds).
- Determinism green; no `HashMap` in `src/sim`.
- Performance: 510 creatures on 150×40 at x25 keeps the UI ≥ 30 FPS; headless 1 200 days
  < 15 s.
- S01c/e, S03a/c, S13, S07b match their prototypes in panel structure.

## Checkpoint demo script
1. Generate the default world; x5. Prey cluster on meadows; at night most letters stop;
   dens `Ω` accumulate over a few days (capped per region).
2. `k`, move the cursor onto an `H`, read the tooltip, `Enter`: S03a live vitals. `Esc`
   returns to look mode; `f` follows until it drinks (thirst bar falls).
3. `z` → zoom; `Esc`.
4. x25 for a season; `e` → log; `f` until the deaths chip is active; select a death from
   today; the detail mini-map shows the cell; `i` opens S03c with the decay bar.
5. `2` pressure overlay: warm colours on meadows and near water.
6. Headless CSV: species counts fall to zero; the three death causes are all present.

## Tests
- `sim::creatures::tests::{placement_respects_terrain, initial_ages_below_max_age, needs_tick_rates,
  hp_death_attribution, age_death_at_day_boundary, carcass_decay_frees_slot, trail_cap}`
- `sim::behavior::tests::{drink_goal_when_thirsty, graze_picks_best_cell, graze_hysteresis,
  movement_never_impassable, fractional_movement_budget, rest_at_night, forced_rest_at_zero_energy,
  den_creation_capped}`
- `sim::spatial::tests::index_matches_bruteforce`
- `sim::stats::tests::{census_per_species, pressure_decay, drought_flags_daily}`
- `sim::tests::{no_hashmap_in_sim, checksum_includes_creatures}`
- `tests/herbivores.rs::{all_die_without_reproduction, death_causes_all_present, food_is_findable,
  performance_budget}`

## Decisions made here
- Single hp-based death with cause attribution; no separate starvation timers.
- Metabolism scales hunger so it is under selection in C4.
- Fractional movement budget (no integer steps) so speed differences matter.
- Greedy 8-neighbour stepping, no A*; revisit if creatures stall on lake shores.

## Risks
- Starvation cascades if C2's `growth_k`/`season_cap` are low; "food is findable" guards it.
- Perception cost: restrict to bucket bounding boxes; raise `replan_ticks` to 12 if needed.
