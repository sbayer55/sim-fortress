# C5 — Predators, predation and extinction

Back to the [roadmap](README.md). Previous: [C4](c4-evolution.md). Next: [C6 Persistence and balance](c6-persistence-and-balance.md).

## Goal
Add the second trophic level. Foxes, wolves and lynxes hunt herbivores using sense
against camouflage and cover, chase using speed, kill, eat and scavenge. Prey flee. Local
scarcity drives migration. Species can go extinct, which raises the alert modal. The
predator–prey phase plot and the sense overlay make the dynamics visible.

## Checkpoint (what the user sees)
Predator and prey lines chase each other with a lag; the phase plot draws an orbit; the
sense overlay shows what a wolf can see and which hares are hidden by cover; the predator
inspector shows hunt stats; when the lynx die out the alert modal appears and the
simulation pauses.

## Scope

### In
- `sim::predation`: `can_detect`, Hunt goal (Stalk → Chase → Kill → Eat), scavenging,
  hunt statistics, prey preference.
- Prey `Flee` goal with a cheap per-tick predator query; den protection.
- `sim::behavior` migration for prey and predator groups with cooldown.
- Extinction detection (once per species) and `StepReport` alerts; `pred_pressure`.
- `sim::stats::peak_lag()` shared by acceptance tests, S05a Coupling and C6 summaries.
- Events: `DeathPredation`, `Migration`, `Extinction`, `Note` (local extinction).
- Live screens: **S02d sense overlay**, **S03b predator inspector**, **S05b phase plot**,
  **S12 alert modal**, S03a Survival / Condition predation risk / Killer / Scavengers,
  S04b Interactions with kill shares, S06 migration pressure line, S01 danger line.

### Out
Save/load, title, presets, tuning tooling (C6).

## Dependencies
- [C4](c4-evolution.md) complete. C1 FR6 (modals stack and render below) and C3 FR2
  (predation fields already declared) are relied on.
- Screen requirements: [S02](../screens/s02-map-overlay.md), [S03](../screens/s03-creature-inspector.md),
  [S05](../screens/s05-population-charts.md), [S12](../screens/s12-alert-modal.md).

## Functional requirements

### FR1 Params (`[predation]`)
```toml
# defaults of params.creatures.initial_counts change to fox = 8, wolf = 6, lynx = 4 (balance table; spec start 30/24/12)
detect_threshold = 0.8         # hidden when camouflage × cover ≥ sense × detect_threshold
cover_by_terrain = { forest = 1.0, grass_dense = 1.0, marsh = 1.0, grass = 0.8, grass_sparse = 0.6, dirt = 0.6, sand = 0.4, shallow_water = 0.4 }
den_protects = true            # a resting prey on a den cell cannot be targeted
chase_trigger_cheb = 4
chase_max_ticks = 30           # clock starts at cheb ≤ chase_trigger_cheb, not at detection
chase_speed_bonus = 0.5        # extra move budget per tick while chasing
catch_distance_cheb = 1
kill_base = 0.35  kill_speed_w = 1.0  kill_aggression_w = 0.3  kill_size_w = 0.2  kill_min = 0.05  kill_max = 0.95
eat_hours_base = 2  eat_hours_per_size = 4          # ceil(base + per_size × prey.size)
hunger_per_kill_base = 4.0  hunger_per_kill_per_size = 0.4   # balance table; spec start 0.6 (a kill feeds a fox ~25 days)
kill_consumes_decay = 0.6      # eating advances the carcass decay by this much
hunt_cooldown_hours = 6
hunt_hunger_min = 0.45
scavenge_hunger_min = 0.7
carcass_nutrition = 0.5        # hunger −= nutrition × (1 − decay) once per scavenge visit; prey carcasses only
scavenge_hours = 1  scavenge_consumes_decay = 0.2
difficulty = "normal"          # easy | normal | hard — stored here, given meaning in C6 (never mutates kill_base)
prey_preference = { fox = { vole = 0.6, hare = 0.4 }, wolf = { deer = 0.5, hare = 0.4, vole = 0.1 }, lynx = { hare = 0.6, vole = 0.4 } }
nocturnal = ["fox", "lynx"]
flee_distance = 8  flee_ticks = 10  flee_energy_factor = 2.0  rest_detect_factor = 0.5
wary_distance = 3  wary_step = 3  wary_ticks = 2  wary_speed_factor = 0.5  wary_release_factor = 1.5
migrate_veg = 0.25  migrate_days = 6  migrate_pressure = 0.35  migrate_prey_min = 10  migrate_cooldown_days = 30
local_extinction_min = 5
```
`kill_chance = clamp(kill_base + kill_speed_w × (pred.speed − prey.speed) + kill_aggression_w
× pred.aggression − kill_size_w × prey.size, kill_min, kill_max)`.

### FR2 Detection (single source of truth `predation::can_detect`)
A predator detects a prey within its sense range (`geom::dist ≤ sense_cells`) unless
`prey.camouflage × cover_by_terrain[prey cell] ≥ pred.sense × detect_threshold`, or the
prey is resting on a den cell and `den_protects`. Detection is deterministic and
distance-independent inside the ring. This rule supersedes the S02d doc's "80 % of sense"
wording. Note: with base genomes voles (camouflage 0.60) and some hares are already hidden
from base wolves in Forest/GrassDense; hiding is common in cover from day one — do not tune
it away. A prey detects a predator within its own
sense range (halved while resting, `rest_detect_factor`) when `pred.camouflage < prey.sense`.

### FR3 Predator goal order
Drink (thirst > 0.6) → Hunt (hunger > `hunt_hunger_min` and a detectable prey in range;
choose the highest `preference / (1 + dist / 4)`; preference 0 means never targeted;
predators never hunt predators) → Scavenge (hunger > `scavenge_hunger_min` and a prey
carcass in range) → Mate (C4 rules; the predator values of the six-species maps in C4 FR1 apply, and the
vegetation gate does not) → Rest (energy
< 0.25, or `is_night()` for diurnal species, or `!is_night()` for `nocturnal`) → Patrol
(Wander biased toward the highest `prey_pressure` cell seen).

### FR4 Hunt phases
*Stalk*: move with the `+chase_speed_bonus` budget toward the prey; the prey detecting the
predator starts its Flee but does not start the chase clock. *Chase*: begins when `cheb ≤
chase_trigger_cheb`; from then the predator has `chase_max_ticks` to reach contact. *Contact*: on the first tick with
`cheb ≤ catch_distance_cheb` roll `kill_chance` once; success → prey dies
(`DeathPredation`, `death.killer`, `death.chase_ticks`), predator enters *Eat* on the
carcass cell for `eat_hours`, hunger −= `hunger_per_kill`, carcass `decay += kill_consumes_decay`;
failure → the predator idles one tick and the hunt fails; the prey enters Flee for
`flee_ticks` regardless of whether it had detected the predator. Ticks exhausted or prey out of range → fail. Every outcome records an attempt and
starts `hunt_cooldown_hours`; per-creature stats update: `kills_by_species`, `attempts`,
`last_kill (id, day, region)`, `chase_ticks_sum`, `chase_longest (ticks, year)`; prey
records `chased`, `escaped`, `threats_by_species`.

### FR5 Flee
Every tick, **predator-first**: iterate living predators (ids ascending) and, with
`spatial::within(pred, max_prey_sense_cells)`, mark each prey in range whose detection rule
(FR2) passes as threatened by that predator. (Per-prey bucket scans are forbidden: 1 000 prey
× 561 buckets per tick is infeasible.) On detection `Flee` pre-empts every goal: move at full budget away along the
predator→prey vector for `flee_ticks` or until `geom::dist ≥ flee_distance`; steps cost
`flee_energy_factor × move_cost_energy`. `predation_risk = min(1, 0.5 × cell.pred_pressure
+ 0.5 × predators_in_range / 3)` feeds S03 Condition.

### FR5b Wary (the low-exertion second tier)
A prey that detects a predator which is **not** a danger (FR5's danger rule fails: it is not
hunting this prey and is not hungry within `chase_trigger_cheb`) within `wary_distance` enters
`Wary` instead of `Flee`. Wary pre-empts every prey goal except a forced rest
(`RestReason::Forced`, energy ≤ 0) — including Drink, Graze, night/energy Rest, Mate, Wander
and Migrate — and never overrides Flee. It is deliberately cheaper than Flee: the waypoint is
`wary_step` cells away along the predator→prey vector, speed is `wary_speed_factor ×` normal,
and each step costs the *normal* `move_cost_energy` rather than `flee_energy_factor`. The
state is retained while `wary_ticks` runs and the predator stays within
`wary_distance × wary_release_factor` (hysteresis; the away-vector survives undetected ticks
exactly as a forced flee does). `wary_distance = 0.0` disables the tier. Wary never feeds
`threatened_by`, `predation_risk`, the S02d danger line or the C8 alarm pass, and never
touches `chased`/`escaped`/`threats_by_species` — those keep their FR5 meaning; per creature
it records `wary_count` (S03a Survival). **Event**: encounters are tallied per
`(region, prey species, predator species)` and flushed at the day boundary into **at most one
`EventKind::Wary` per region per day**, the most common pair naming the line and `detail`
carrying `region:prey:predator:total`, so the log gains the signal without one event per
animal per tick.

### FR6 Pressure
`pred_pressure` is maintained exactly like `prey_pressure` (C3 FR8) for living predators.

### FR7 Migration
Evaluated daily per (species, region) with `migrate_cooldown_days` per pair. Prey groups
migrate when the region's seasonal shortfall `veg_mean / season_cap[season] < migrate_veg`
holds for `migrate_days` consecutive days (so winter alone does not trigger migration), or the mean `pred_pressure` over the cells the group occupies >
`migrate_pressure`. Predator groups migrate when the region's prey count <
`migrate_prey_min` for `migrate_days`. A **group** is all living members of the species in the origin region. Destination = the
adjacent region (rectangles sharing an edge segment of positive length) maximising `mean_vegetation × (1 − mean_pred_pressure)` (prey) or `prey_count`
(predators); `target_cell` = the walkable destination cell with the highest vegetation
(prey) / highest `prey_pressure` (predators). Members get `Migrate(target_cell)` overriding
Wander/Graze/Patrol for up to 2 days. Group word: ≤ 3 members `family`, prey > 3 `herd`,
predators > 3 `pack`. One `Migration` event per group with `pos` = origin region centre.

### FR8 Extinction and alerts
At the day boundary, for each species with `initial_count > 0` and `peak > 0` that is not
already marked extinct: if `living == 0` mark it and emit `Extinction` (text with the
last individual's name, tag, cause, region; `pos` = its death cell) and push
`Alert::Extinction { event_index, species, last: CreatureId }` into the tick's
`StepReport`. Species with `initial_count == 0` never emit. Local extinction: a region
whose count for a species drops to 0 after being ≥ `local_extinction_min` a season ago
emits a `Note` once per (species, region) until repopulated. `Sim::step() -> StepReport
{ alerts: Vec<Alert> }`; the `App` loop pushes one S12 per alert (in species-table order
when several arrive the same day; popping one reveals the next), records
`speed_before_alert`, and sets `paused` when `auto_pause_on_extinction`; when the option
is off, the event is logged and shown in the ticker only. The last individual's death
record is retained until the alert is dismissed, or decays normally when no modal is raised;
`event_index` is the absolute event sequence number. S12 `years` = years since the species'
first birth (or world start). Buttons: Continue (pop, restore speed), View lineage (pop,
push S08 on the last individual), Pause (pop, stay paused).

### FR9 Sense overlay (S02d)
Selected creature = look cursor creature or followed creature, default the living
predator with the most kills (ties by id); `Tab` cycles living predators by id ascending.
Ring radius `sense_cells`; the table lists prey in range as detected/hidden/target using
`can_detect`; for a selected prey it lists detected predators using the prey rule. If the
selection dies the overlay reverts to the plain map.

### FR10 Predator inspector (S03b) and prey additions
Hunt stats from the per-creature counters: kills, attempts, success %, preference bars
from actual kill shares (params shares when < 5 kills), last kill, average/longest chase,
current target and distance. S03a Survival: `chased`, `escaped`, escape rate, threats
seen shares; Killer/Scavengers on S03c from `death.killer` and the two nearest predators
with Scavenge goal. S04b Interactions: kill shares, `hunted by` / `competes with`.

### FR11 Phase plot (S05b) and coupling
x = prey total, y = predator total from `Series`; last 40 days highlighted; equilibrium =
time means. `stats::peak_lag(prey, pred) -> Option<u32>`: skip the first 360 days, smooth both with a
30-day centred moving average, mean-subtract, Pearson correlation of `pred[t+L]` vs
`prey[t]` for `L ∈ 0..=120`, return `argmax L` (lowest index on ties), or `None` when either
series has fewer than 2 local maxima; a local maximum is a sample ≥ all samples within ±45
days (lowest index on ties) and ≥ 1.15 × the series mean. S05a Coupling computes over the
full `Series` and shows `–` on `None`.

### FR12 Follow-mode danger line
Nearest predator whose target is the followed prey, its `cheb` distance, and whether the
prey has detected it (prey rule).

### FR13 Territory: scent, avoidance and contests (2026-09-21)
Plan and decisions in [territory-plan.md](../territory-plan.md); the first slice of the
territory brainstorm in [feature-ideas.md](../feature-ideas.md). The `[territory]` table
(`sim::params::TerritoryParams`, fourteen tunables with field docs) drives three mechanics:

- **Scent grid.** `World.scent` holds one `Mark { strength, holder }` per species per cell
  (species block `s` at `s × cells`, sized by `World::init_scent` once the roster is known;
  prey blocks stay empty). Every living **adult** predator adds `mark_per_tick` to its
  species' mark under it on the same write as `pred_pressure` (`behavior::death::pressure`),
  capped at 1; a kill adds `kill_mark` at the kill cell. The holder changes to the depositor
  only while the mark is below `hold_min`, or when a contest is won. Every mark is multiplied
  by `decay_per_day` at the day boundary beside the pressure decay (0.90: a full mark falls
  under `hold_min = 0.15` on the nineteenth day of absence). No RNG.
- **Scent avoidance** (`behavior::territory::Scent`). A reader below the herding gate
  (`sociality < social.cohesion_min`: fox and lynx at their base genomes) treats same-species
  scent held by anyone else at or above `notice_min` as *foreign*; a social reader (wolf)
  reads nothing. `avoid = avoid_w × aggression × (1 − sociality) × (1 − hunger)`, so a
  starving animal trespasses. Two sites: the `perceive` patrol score becomes
  `prey_pressure × (1 − min(1, avoid × foreign))`, and the `pick_hunt_target` score is
  divided by `1 + avoid × foreign` under the prey. `wander`, Rest and every prey rule are
  untouched.
- **Challenge and contest.** A solitary adult that holds the cell it stands on and sees a
  same-species adult (not its mate) on ground it holds enters `Goal::Challenge` after Hunt
  and Scavenge and before Mate, walks at it for at most `challenge_ticks`, and gives up when
  the intruder dies, leaves sense range or steps off held ground. `territory::contest_contacts`
  runs after `hunt_contacts` and before `scavenge_contacts`: for each challenger adjacent
  (`cheb ≤ 1`) to its target, in ascending id, one `rng.chance` on the creature stream with
  the resident winning at `1 / (1 + exp(−contest_k × (aggression × size + resident_bonus −
  intruder's aggression × size)))`. Both pay `contest_energy`; the loser loses
  `contest_injury × winner size` of hp clamped at `hp_floor` (contests never kill), is evicted
  through the prey flee fields unchanged (`threatened_by` = the winner, `flee_until = tick +
  evict_ticks`, `preempt_predator` keeps it fleeing), and both wait
  `contest_cooldown_days`. The winner over-marks the loser's cell to strength 1. One
  `EventKind::Contest` per contest, subject the winner, `detail = winner:loser:region`;
  `DeathTallies::contests` accumulates per species.
- **Neutral control.** `mark_per_tick = 0` and `kill_mark = 0` (`TerritoryParams::neutral`)
  lays no scent, so no ground is foreign, nobody is resident, no challenge is raised and no
  contest draw happens: the run is bit for bit the pre-FR13 run, pinned by
  `behavior::tests_territory::neutral_territory_reproduces_the_old_checksum` against the
  previous default checksum `0xf883_9b51_57e3_c3f8`. The default checksum was re-baselined.
- **Seeing it.** S02j, the `Scent` base on S14 (shares the species sub-pick with S02f; `Tab`
  cycles); S03b `territory holds N cells · resident here | off its ground` and `contests won W
  lost L`, the Behaviour line while challenging or evicted; `Contest` in S07 (the avoidance
  chip) and S11; `--summary` columns `contests_<species>` and `nn_dist_<species>` (mean
  distance from each living adult to its nearest same-species adult, from
  `stats::nearest_neighbour_mean` in the midnight census); `bench_c5` keys `mark_per_tick`,
  `avoid_w`, `resident_bonus`, `contest_injury` and `territory=off`.
- **Save `VERSION = 17`**: the grid, the five per-creature fields (`challenge_target`,
  `challenge_until`, `contest_cooldown_until`, `contests_won`, `contests_lost`) and the table.

**Measured (one year, release, `bench_c5 <seed> 1 [territory=off]`, 2026-09-21).** Counts are
`[vole, hare, deer, fox, wolf, lynx]` at the year's end; `nn` is the adult fox nearest-neighbour
distance; contests are the fox tally.

| seed | territory | end counts | fox nn | fox contests | fox hunt success |
|---:|---|---|---:|---:|---:|
| 42 | on | 14, 187, 82, 28, 6, 3 | 4.2 | 206 | 70 % |
| 42 | off | 1, 116, 69, 39, 11, 4 | 2.9 | 0 | 59 % |
| 1 | on | 0, 65, 47, 15, 6, 0 | 8.2 | 205 | 57 % |
| 1 | off | 0, 42, 48, 4, 3, 2 | 10.2 | 0 | 64 % |
| 2 | on | 80, 101, 59, 0, 0, 0 | — | 26 | 75 % |
| 2 | off | 2, 25, 68, 0, 1, 0 | — | 0 | 70 % |

Voles end year one far higher with territory on two of the three seeds (14 vs 1, 80 vs 2)
and foxes lower on seed 42; the nearest-neighbour number is confounded by the fox count
(seed 1 keeps 15 foxes against 4).

**Sweeps (`scripts/sweep.sh 1 6 Y`, release, defaults vs the neutral overlay, 2026-09-21).**
One year, seeds 1–6; `nn` is the adult fox nearest-neighbour distance in cells:

| seed | territory | vole | hare | deer | fox | wolf | lynx | fox contests | fox nn |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | on | 0 | 65 | 47 | 15 | 6 | 0 | 205 | 8.2 |
| 1 | off | 0 | 42 | 48 | 4 | 3 | 2 | 0 | 10.2 |
| 2 | on | 80 | 101 | 59 | 0 | 0 | 0 | 26 | — |
| 2 | off | 2 | 25 | 68 | 0 | 1 | 0 | 0 | — |
| 3 | on | 36 | 189 | 68 | 8 | 2 | 1 | 59 | 15.8 |
| 3 | off | 17 | 203 | 33 | 19 | 0 | 0 | 0 | 8.2 |
| 4 | on | 2 | 185 | 60 | 28 | 9 | 3 | 173 | 4.8 |
| 4 | off | 3 | 186 | 32 | 34 | 8 | 3 | 0 | 4.3 |
| 5 | on | 2 | 128 | 15 | 19 | 7 | 3 | 134 | 7.8 |
| 5 | off | 6 | 107 | 15 | 10 | 7 | 3 | 0 | 11.0 |
| 6 | on | 0 | 12 | 32 | 41 | 10 | 5 | 332 | 4.2 |
| 6 | off | 0 | 85 | 57 | 41 | 5 | 4 | 0 | 4.7 |

- **Spacing: not shown.** The plan's claim was a higher fox nearest-neighbour distance on at
  least five of six seeds. Raw `nn` is higher with territory on seeds 3 and 4 (and 42), lower
  on 1, 5 and 6, and on seed 6, the one seed with equal fox counts (41 each), it is *lower*
  (4.2 vs 4.7). Contests are frequent (26–332 fox contests a year), so foxes do meet on held
  ground, but the mean nearest-neighbour distance follows the fox count and the prey
  distribution more than the mechanic. A count-normalised spacing index (Clark–Evans) is the
  follow-up before any spacing claim is made.
- **Voles: better where it matters, not everywhere.** Year-one voles 80 vs 2 (seed 2), 36 vs
  17 (seed 3), 14 vs 1 (seed 42); a handful either way on seeds 4 and 5; extinct on both on
  seeds 1 and 6. Total year-one extinctions are equal (6 vs 6 over the six seeds).
- **Two years:** foxes are extinct by year two on five of six seeds under both conditions
  (seed 3 keeps 3 either way); voles survive only on seed 2 with territory (16 vs 0).
  Extinctions 23 vs 23 over the six seeds. **Five years:** every predator extinct on every
  seed under both conditions; extinctions 27 with territory vs 28 without (seed 2 keeps
  hares). The C5 collapse is untouched by this slice and the population bands above keep
  their status; the ignored tests' reasons still hold.
- **Cost:** `--profile` at seed 1 over 20 000 ticks: 0.5007 s per 1 000 ticks with territory
  against 0.4986 under the neutral overlay (+0.4 %), below the 2 % the plan allowed, so
  `docs/PERFORMANCE.md` is unchanged.
- **Tests:** `tests/predators.rs::{contests_are_non_lethal_and_logged,
  territory_spaces_foxes_on_seed_42}`; the second pins the one-year seed-42 measurement
  (4.2 vs 2.9) and says in its comment that the sweep does not generalise it.

- **Habitat-side experiment (2026-09-22).** C2 FR12 (succession and trampling,
  [succession-plan.md](../succession-plan.md)) attacks the vole-refuge problem from the
  other side: lightly grazed meadow closes into forest, which is cover 1.0 for prey. Its
  first six-seed read is recorded in the C2 FR12 result; the population bands above keep
  their status until that result says otherwise.

## Acceptance criteria
- Seed 42, default params, 10 years headless: all six species alive at year 5 and at
  least five at year 10; on the smoothed series both prey and predator totals have ≥ 3
  local maxima; `peak_lag` ∈ 5..=60.
- Seeds 1..=10 with `initial_counts.lynx = 4`: an `Extinction` event within 3 years in at
  least 7 seeds; no `Extinction` ever for a species with `initial_count == 0`; a species
  emits at most once.
- `can_detect`: hare camouflage 0.9 vs wolf sense 0.7 is hidden in Forest (0.90 ≥ 0.56)
  and visible on Sand (0.36 < 0.56); a resting prey on a den is never targeted.
- Hunt success per predator species over a 1-year run is between 15 % and 60 %.
- Wary (FR5b): a satiated predator within `wary_distance` turns a detecting prey Wary, not
  Flee; a hungry predator inside `chase_trigger_cheb`, or one hunting that prey, still flees;
  a forced rest is never interrupted; a wary step costs less energy than a flee step and
  covers fewer cells; seed 42 over one year emits at least one `Wary` event and at most one
  per region per day.
- Dry world: a `Migration` event occurs and the destination region's count for that
  species rises within 5 days; no (species, region) pair migrates twice within the cooldown.
- Determinism green; UI ≥ 30 FPS at x25 at the balance-table population; 10 years
  headless < 5 min.
- S02d, S03b, S05b, S12 match their prototypes in panel structure.
- **Balance table**: the implementer may tune only `kill_base`, `chase_max_ticks`,
  `chase_speed_bonus`, `hunt_cooldown_hours`, `hunger_per_kill_*` and predator
  `initial_counts` to meet the bands above, and must record the final values in FR1.

  **Implementation status (second pass, Sept 2026).** All C5 mechanics, screens and
  spec-listed unit tests are in place; `forced_extinction_7_of_10`, `migration_scenario`
  (destination rises within 5 days, cooldown honoured) and `performance_budget` pass. The
  three population bands (`six_species_five_years`, `oscillation_lag`, `hunt_success_band`)
  remain `#[ignore]`d with the measured numbers in their messages. Four mechanism defects
  found while tuning were fixed first, since no lever mattered before them:
  - prey fled from *every* detected predator within sense range on every tick, exhausted
    themselves (flee steps cost double) and died of thirst in forced rest — a threat is now
    a detected predator that is hunting this prey or is hungry and within
    `chase_trigger_cheb`, and a failed kill roll's forced flee is retained for `flee_ticks`;
  - satiated predators camped on the highest-`prey_pressure` cell (the water hole) — Patrol
    is a biased wander again;
  - a path searched for one target was followed toward the next (flee vector → water);
  - hunt stats died with the carcass (`DeathTallies::hunt_*` now accumulate).

  **Wary tier (`FR5b`, second pass).** The second avoidance level is implemented with its
  measured volume: seed 42 over one year records **1 824 `Wary` events**, ~5 per day across
  the eight regions against the 8/day hard bound, since most region-days have at least one
  wary encounter. The first-pass defaults (`wary_distance = 6`, `wary_ticks = 3`) were too
  costly: over seeds 1-8 at 3 years they drove seeds 1 and 2 from deer-plus-wolves to **zero
  living creatures**, the mechanism being that wary now pre-empts Drink and Graze, so prey
  near resident predators cannot feed. The recorded defaults are `wary_distance = 3`,
  `wary_ticks = 2`: in the same sweep every radius ≥ 4 pushed seed 1 to zero, while 3 and 2
  kept it alive. Ranking 3 against 2 and the timers is not meaningful, because the collapse
  is chaotic — seed 4 is fully extinct at every setting including `wary_distance = 0`, and
  seed 6 reaches zero at most wary settings but not at 0 — so the radius is the largest that
  did not show the systematic seed-1 collapse, and `wary_distance = 0.0` stays the kill
  switch. The population bands below are unchanged in status; wary is not one of the
  allowed balance levers and does not need to be, since the structural refuge/mate defects
  are still the binding constraint.

  With those fixed, the population bands still fail for a structural reason the levers do
  not reach: **foxes have no refuge from voles and voles are the marginal C4 species.**
  Fox sense 0.80 defeats vole camouflage 0.60 in every cover (`0.60 × 1.0 < 0.64`),
  `kill_chance` fox→vole is 0.76 (0.56 at `kill_base` 0.15), and a satiated fox is
  always mate-eligible, so 8 foxes become 25–60 in a year and take the ~250 voles to zero;
  the foxes then thin out on hares while wolves and lynxes, spread thin over the map, die of
  old age without meeting a mate. Sweeps of the allowed levers (`hunger_per_kill_base`
  0.6–6.0, `hunt_cooldown_hours` 6–72, `kill_base` 0.35–0.15, `chase_*`, founders
  30/24/12 down to 4/3/2) and of the fallback lever (predator `mate_cooldown_days` 120–360,
  predator `litter_max` 3–1, vole `litter_max` 0–1) all end with voles extinct in year
  1–3 and predators gone by year 5; the best outcomes keep three species (hares, deer and a
  few foxes) at year 5. Recorded FR1 values are the best spec-only point found.
  Reaching the bands needs a design change rather than a value: a vole refuge (cover or a
  `detect_threshold` per prey size), a predator mate-seeking range or a den-based pack so
  wolves and lynxes can find mates, and a reproduction gate on predators tied to recent
  kills rather than hunger alone. `examples/bench_c5.rs` runs any lever set headless and
  prints the per-year table, kill counts and `peak_lag`.

## Checkpoint demo script
1. Default world, x25, 3 years. Predator counts fall after prey dips.
2. `g`, `2` → phase orbit; `1` → both lines with the lag shown in Coupling.
3. `k` on a `W`, `4` → sense overlay with detected and hidden prey; `Enter` → S03b hunt
   stats; `f` follow through a hunt; the ticker shows the kill; `Esc`.
4. `w` → S09, set Lynxes to 4 (species rows are live since C3), Generate, x25 until S12
   appears; View lineage → S08 → `Esc` → map, still paused → `Space`.
5. Dry world: a Migration event appears; `Enter` in the log opens look mode on the origin.

## Tests
- `sim::predation::tests::{can_detect_cover_table, den_protects, kill_chance_bounds, hunt_phases_and_single_roll,
  chase_clock_starts_at_trigger, eat_reduces_hunger_and_decay, scavenge_consumes_decay, scavenge_prey_carcass_only,
  flee_query_is_predator_first, nocturnal_rest_by_day, cheb_vs_ellipse_usage}`
- `sim::behavior::tests::{flee_reacts_within_one_tick, flee_costs_energy, migration_destination, migration_cooldown,
  predator_migration_on_low_prey, pressure_clamped}`
- `sim::stats::tests::{peak_lag_on_synthetic_series, local_maxima_rule}`
- `sim::tests::{extinction_once_and_not_for_absent_species, alert_queue_two_species_same_day}`
- `tests/predators.rs::{six_species_five_years, oscillation_lag, forced_extinction_7_of_10, hunt_success_band,
  migration_scenario, performance_budget}`

## Decisions made here
- Predators are solitary agents; packs are emergent.
- One kill roll per contact with a head start on failure, so `kill_chance` is the lever.
- Chebyshev distance for adjacency/catch/trigger; ellipse distance for perception.
- Fox and lynx nocturnal; wolf and all prey diurnal.

## Risks
- Overkill collapse: levers are `hunt_cooldown_hours`, `eat_hours_*`, `hunger_per_kill_*`,
  `kill_base`; the oscillation criterion bounds it.
- Per-tick prey predator-queries: keep them bucket-bounded; measure before optimising.
