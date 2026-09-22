# Territory — implementation plan

*Written against `main` at 9e43d1e (the S15 Traits & Fates merge, PR #27) on 2026-09-21.
Save format `VERSION = 16`, checksum `0xf883_9b51_57e3_c3f8`, thirteen genome slots.*

*Implemented 2026-09-21 on this branch. Deviations from the text below: the scent overlay
S02j shares the S02f species sub-pick and its `Tab` cycles every species (a prey species paints
an empty block and the sidebar says `prey lay no scent`) rather than predators only; the
contest tally lives in `DeathTallies::contests` and the spacing statistic in
`GroupCensus::nn_mean` (`stats::nearest_neighbour_mean`, adults only) beside the group census,
so both ride the existing midnight refresh; `contest_contacts` takes the tallies as a plain
extra argument (`too_many_arguments` is allowed crate-wide, so no `ContestCtx` bundle was
needed) and `perceive` takes a `&Scent` view the same way; a full mark falls under `hold_min`
on the nineteenth day, not the eighteenth; `Contest` events share the S07 avoidance chip with
`Wary` rather than adding a tenth chip. Checksum re-baselined to `0xf9ea_eb02_3e27_c085`,
save `VERSION = 17`; the neutral-overlay tripwire pins the old value. Results are recorded in
C5 FR13 and in the sweep section at the end of this document.*

**Goal.** Give predators territories that emerge from three mechanics, chosen from the
territory brainstorm in [feature-ideas.md](feature-ideas.md): a **per-species scent grid**
that adult predators mark and that decays; **scent avoidance**, where a solitary predator
prefers ground it holds and steers around ground held by a rival of its own species; and an
**intrusion response**, where a resident challenges a same-species intruder standing on its
ground and a short contest evicts the loser. The expected visible effect is foxes and lynxes
spacing out into tiled ranges with gaps between them, a rising nearest-neighbour distance for
the solitary predators against a control, and contest events in the log. The C5 doc's own
status note names predator territoriality as the missing modelling change behind the
fox–vole collapse; this plan is the experiment on that claim.

**Non-goals.** No new genome slot: the S03/S04 budgets are exhausted (diet-breadth notes),
so territoriality is derived from existing genes. No territory object, owner record, home
anchor or pack record; ownership is per cell and per individual. No cross-species scent
reading (a fox does not yet avoid wolf scent). No change to `wander`, prey behaviour,
migration or mating. Contests are **non-lethal** in this slice. No save migration: v16 files
are refused like every earlier bump. The parked ideas are listed under *Territory
follow-ups* in `feature-ideas.md`.

**Starting point.** Three existing pieces carry most of the weight.
- `Cell.pred_pressure` / `prey_pressure` are already scent-shaped fields: written once per
  creature-tick by `behavior::death::pressure` (`pressure_per_creature_tick`, capped at 1)
  and multiplied by `pressure_decay_per_day` in `day_boundary`. The scent grid follows the
  same two touch points.
- `perception::perceive` visits every land cell in the sense ellipse once per replan and
  already reads `cell.prey_pressure` to pick `best_patrol`; the same loop can read one more
  value per cell for predators. `hunt::pick_hunt_target` scores neighbours from the
  `Peer` list the same call returns.
- The prey flee machinery is reusable as is: `Creature.threatened_by` + `flee_until` drive
  `threat::flee_target`, `goal_satisfied(Flee)` and the doubled move cost in
  `movement.rs`. Nothing reads those two fields on a predator today, so eviction can set them.
- `SocialParams::cohesion_min` already splits the roster into solitary (hare, fox, lynx)
  and social (deer, wolf) at their base sociality; `CreatureStore` never reuses an id
  (`next_id` is monotonic), so a holder id on a cell can never point at a later creature.

**Standing constraints.** Everything in `AGENTS.md`. The ones that bite here: no `HashMap`
under `src/sim` (the grid is a flat `Vec`); `perceive` is the hot loop, so one index per
cell and nothing allocated; every tunable has a `FIELD_DOCS` entry; the step order is fixed
and RNG draws are never reordered, so the one new draw (the contest roll) lives in a new
contact pass at a fixed position and fires only when a contest happens; the 800-line
ceiling (`goals.rs` is at 439, `hunt.rs` at 413, `behavior.rs` at 351, so the new code goes
in a new module); and `clippy`'s six-argument limit, which `perceive` already dodges with a
bundle in the diet plan's spirit.

---

## Design decisions

**D1 — A mark per predator species per cell, with a holder.** A species-only grid cannot
tell a fox its own scent from a rival fox's, and intraspecific exclusion is the point. Each
mark is a strength and the id of the creature holding it:

```rust
pub struct Mark { pub strength: f32, pub holder: CreatureId }   // holder 0 = none
World.scent: Vec<Mark>   // roster.len() × cells, row-major within a species block
```

The block for species `s` starts at `s × cells`; prey blocks stay at zero and cost 8 bytes a
cell. The alternative, a compact predator-only index, was rejected because every per-species
table in the sim is a `Vec` in roster order (`AGENTS.md`), and a second index scheme for one
table is a trap. At 150 × 40 × 6 the grid is 288 KB in memory and in the save.

**D2 — Deposit, decay and the holder rule.** No RNG in any of it.
- **Deposit**, in the predator arm of `pressure()`: every living **adult** predator adds
  `mark_per_tick` to its species' mark under it, capped at 1. Juveniles follow their mother
  and do not mark. A kill adds `kill_mark` at the kill cell (in `hunt::resolve_kill`, beside
  the killer's tally).
- **Holder rule**: the holder changes to the depositor only when the strength *before* the
  deposit is below `hold_min`. A resident who keeps refreshing stays holder; a floater can
  claim only faded ground. The single exception is a won contest (D6), which over-marks
  the loser's cell to the winner.
- **Decay**, beside the pressure decay in `day_boundary`: `strength *= decay_per_day`
  (0.90, so a full mark falls below `hold_min = 0.15` on the nineteenth day of absence). The holder
  id is left in place; a dead holder's ground is foreign to everyone until it fades, which is
  a short "ghost" period and is accepted.

**D3 — Own, foreign and how strongly it matters.** The reader decides, at replan, over its
own species' block only:
- `solitary = sociality < social.cohesion_min` (the herding gate C8 already uses).
- `foreign(cell) = strength` when `holder ≠ self`, `solitary`, and `strength ≥ notice_min`;
  otherwise 0. A social predator treats all same-species scent as neutral, so wolves share
  a range without a pack record existing.
- `exclusion = aggression × (1 − sociality)`, so the strength of territorial behaviour is an
  emergent phenotype of two genes the browser already draws, and S15 can correlate it.
- `avoid = avoid_w × exclusion × (1 − hunger)`: a starving animal trespasses, a fed one
  stays home. That asymmetry is what produces contests instead of a static tiling.

**D4 — Where avoidance acts.** Two sites, no new goal, no new state.
- `perceive`, predator branch of the cell loop: the patrol candidate score becomes
  `prey_pressure × (1 − min(1, avoid × foreign))`, one extra index per visited cell and
  only for predators (the `foreign` read is skipped for prey by kind).
- `pick_hunt_target`: the score is divided by `1 + avoid × foreign(prey cell)`, so a hungry
  hunter still crosses but prefers prey on its own ground.
`wander` and `Rest` are untouched. Under the neutral overlay (D9) both formulas are
identities.

**D5 — Residency and the challenge.** A creature is **resident** on a cell when it is that
cell's holder and the strength is at least `hold_min`. In `replan_predator`, after Scavenge
and before Mate (hunger still wins), a solitary adult resident scans the `Peer` list
`perceive` already returned for the nearest same-species adult that is not its `mate_id`
and stands on a cell the resident holds. Ties resolve by ascending id, which is the list's
order. It enters **`Goal::Challenge`** with `challenge_target = Some(id)`,
`challenge_until = tick + challenge_ticks`, target = the intruder's cell, and re-targets on
each replan like Stalk does. `goal_satisfied(Challenge)` holds when the intruder is gone,
dead, no longer on held ground, or the timer ran out; `contest_cooldown_until` gates the
next challenge either way. `Challenge` is appended to `Goal` so every existing
discriminant, and the checksum's `goal as u8`, is unchanged.

**D6 — The contest, one pass, one draw.** `territory::contest_contacts` runs after
`hunt_contacts` and before `scavenge_contacts`, over challengers in ascending id whose
target is alive and within Chebyshev 1. The resident's score is `aggression × size +
resident_bonus`, the intruder's `aggression × size`, and the resident wins with probability
`1 / (1 + exp(−contest_k × (resident − intruder)))`, one `rng.chance` on the creature
stream. Then:
- both lose `contest_energy` (energy 0 already means a forced rest at the next replan);
- the loser loses `contest_injury × winner size` of hp, clamped at `hp_floor` (D7 makes it
  non-lethal), takes `contests_lost += 1`, gets `threatened_by = Some((winner x, y,
  species))` and `flee_until = tick + evict_ticks`, drops any hunt, and takes
  `contest_cooldown_until = tick + contest_cooldown_days × ticks_per_day`;
- the winner takes `contests_won += 1`, the same cooldown, and the loser's cell is
  over-marked to strength 1 with the winner as holder;
- one `EventKind::Contest` event, subject the winner, text `Ash f#012 drove Birch f#020
  off <place>`, `detail = winner:loser:region` in the `origin>dest` tradition.
Mutual challenges at a shared border are common: the lower id's contest resolves first and
the other's `Challenge` clears when its target is found evicted.

**D7 — Eviction reuses Flee, unchanged.** A new `preempt_predator`, the sibling of
`preempt_prey`, runs first in `update_one` for predators: while `threatened_by` is set and
`tick < flee_until` the goal is `Flee` with `flee_target`'s away cell, replanning every tick;
when the timer lapses the fields clear and the creature replans as Patrol. Flee steps already
cost `flee_energy_factor ×`, so eviction is expensive by construction. Prey-only counters
(`chased`, `escaped`, `threats_by_species`) are not touched. `Cause::Injury` exists but its
death path is `unreachable!()` in `behavior.rs`, which is why contests stay non-lethal here;
opening it needs an event kind and an S15 fate first (follow-up).

**D8 — Parameters, and the neutral control.** A `[territory]` table, `TerritoryParams`,
after `diet` in `Params` (serde order is load-bearing; this is part of the VERSION bump):

| key | default | role |
|---|---:|---|
| `mark_per_tick` | 0.05 | adult predator deposit per tick (20 ticks from 0 to full) |
| `kill_mark` | 0.50 | extra deposit at a kill cell |
| `decay_per_day` | 0.90 | daily multiplier on every mark |
| `hold_min` | 0.15 | below this a depositor takes the holder slot; at or above it the holder is resident |
| `notice_min` | 0.10 | foreign scent below this is ignored |
| `avoid_w` | 1.0 | weight of `exclusion × (1 − hunger)` in D4 |
| `resident_bonus` | 0.25 | prior-residence advantage in D6 |
| `contest_k` | 4.0 | steepness of the win logistic |
| `contest_energy` | 0.15 | energy both sides pay |
| `contest_injury` | 0.10 | hp the loser pays per unit of winner size |
| `hp_floor` | 0.05 | hp is clamped here after a contest (non-lethal) |
| `evict_ticks` | 24 | the loser's flee timer |
| `challenge_ticks` | 12 | give up a challenge after this |
| `contest_cooldown_days` | 3 | both sides wait this long before another challenge |

Setting `mark_per_tick = 0` and `kill_mark = 0` is the **neutral control**: no scent, so
`foreign` is 0 everywhere, both D4 formulas are identities, nobody is ever resident, no
challenge is raised and no contest draw happens. Under that overlay the run is bit for bit
today's run, and step 2 pins that with a test against the *old* checksum. No `enabled` flag
in the hot loop, in the spirit of `social.group_size_max = 0` and the diet neutral overlay.

**D9 — Save `VERSION = 17`, checksum re-baselined.** `World.scent`, the four new `Creature`
fields (`challenge_target`, `challenge_until`, `contest_cooldown_until`, `contests_won`,
`contests_lost`) and `Params.territory` are all serde-visible. Under defaults the pinned
checksum moves because patrol targets change and contests draw from the creature stream;
the test comment says why, as the C8 and diet bumps did.

**D10 — Seeing it, within budget.** S03 gets two rows for predators in the Life panel next
to the hunt tallies: `holds N cells · resident here` (a count over the species block, computed
by the screen, not the sim) and `contests won W lost L`; while evicted the Condition section
shows `evicted by <label>` where prey show their threat line. S07 lists Contest events with
the existing kinds; `--summary` gains `contests_<species>` and `nn_dist_<species>` (mean
nearest same-species-adult distance at the last midnight census, the number the acceptance
test asserts on). An **S02j scent overlay** is a `Base::Scent` row on the S14 Base tab with
`Tab` cycling predator species as S02f does, tint = strength on the `heat` ramp, cells held by
the selected creature drawn bright. It is the last step and the one to cut if the S14 modal
(21 rows) has no spare row; the fallback is documenting the S02b pressure base as the way to
see it.

**D11 — Documentation lands in C5.** Territory is predator behaviour, so it is a new FR
after FR12 in `c5-predators.md`, with a balance row; C8 gets a one-line pointer where its
Out list mentions territory. `feature-ideas.md` already marks the slice in progress.

---

## Data model

```rust
// src/sim/params/territory.rs — `[territory]`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TerritoryParams { /* the fourteen fields of D8, documented */ }
impl TerritoryParams {
    /// `avoid_w × aggression × (1 − sociality) × (1 − hunger)`, clamped at 0.
    pub fn avoid(&self, genome: &Genome, hunger: f32) -> f32;
    /// D6: the resident's win probability.
    pub fn resident_wins(&self, resident: &Genome, intruder: &Genome) -> f32;
    /// True when the overlay disables the mechanic (both mark rates 0).
    pub fn is_neutral(&self) -> bool;
}

// src/sim/world.rs
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mark { pub strength: f32, pub holder: CreatureId }   // `CreatureId` has no Default:
impl Mark { pub const NONE: Self = Self { strength: 0.0, holder: CreatureId(0) }; }
pub struct World { /* … */ pub scent: Vec<Mark> /* roster.len() × cells; after `names` */ }
impl World {
    pub fn mark(&self, species: SpeciesId, x: usize, y: usize) -> Mark;
    pub fn mark_mut(&mut self, species: SpeciesId, x: usize, y: usize) -> &mut Mark;
    /// Called by `Sim::new` and on load when the roster length is known.
    pub fn init_scent(&mut self, n_species: usize);
}

// src/sim/creatures.rs
pub enum Goal { /* … existing …, */ Challenge }
pub struct Creature {
    /* … */
    // ---- territory ----
    pub challenge_target: Option<CreatureId>,
    pub challenge_until: u64,
    pub contest_cooldown_until: u64,
    pub contests_won: u16,
    pub contests_lost: u16,
}

// src/sim/behavior/territory.rs — the new module
pub(super) fn deposit(c: &Creature, world: &mut World, tp: &TerritoryParams);   // from pressure()
pub(super) fn foreign(c: &Creature, world: &World, x: usize, y: usize, tp: &TerritoryParams, sp: &SocialParams) -> f32;
pub(super) fn pick_intruder(c: &Creature, peers: &[CreatureId], view: &TickView, world: &World, tp: &TerritoryParams, sp: &SocialParams) -> Option<(CreatureId, (usize, usize))>;
pub(super) fn preempt_predator(c: &mut Creature, world: &World, time: &Time, pp: &PredationParams);
pub(super) fn contest_contacts(store: &mut CreatureStore, world: &mut World, events: &mut EventRing, time: &Time, roster: &Roster, tp: &TerritoryParams, rng: &mut Rng);
pub fn decay(world: &mut World, tp: &TerritoryParams);                          // from day_boundary
```

`EventKind::Contest` with label `contest`. `perceive` gains the territory rules through the
same bundle that carries `cp`, `sp` and the diet params (the diet plan kept a plain seventh
argument because `too_many_arguments` is allowed crate-wide; either way, no new `allow`).

---

## Steps

Each step ends green on its tier before the next starts. Steps 1 and 2 are the sim change;
step 2 is the only one that moves the default checksum.

### Step 0 — `[territory]` params and the save bump
- `src/sim/params/territory.rs` with `TerritoryParams`, `Default`, the three helpers and
  unit tests: `avoid_is_zero_for_the_social_and_the_starving`,
  `resident_wins_more_often_with_the_bonus`, `neutral_overlay_is_detected`.
- Register in `params.rs` (`pub mod territory; pub use territory::TerritoryParams;`), the
  field after `diet`, fourteen `FIELD_DOCS` lines (`params::tests::field_docs_complete`
  checks both directions), `validate` rejects negative rates and `hp_floor` outside 0..1.
- `save::VERSION = 17`. `older_version_rejected` and `version_mismatch_rejected` build
  their files synthetically and need no fixture change.
- `scripts/affected-tests.sh`: `src/sim/behavior/territory.rs|tests_territory.rs` →
  `units[sim::behavior]`, `chunks[predators]` (before the `behavior*` → `herbivores` line).
- Tier: `just test-unit sim::params`, `just test-unit sim::save`.

### Step 1 — the grid (D1, D2) and the new state
- `world.rs`: `Mark`, `World.scent`, the accessors, `init_scent`; `Sim::new` and the load
  path call it; `World` has no `Default`, so every constructor and every hand-built
  fixture in `behavior/tests.rs` and `ecology.rs` starts with `scent` empty, and `mark()` on
  an empty grid returns `Mark::NONE` so old fixtures keep working without edits.
- `creatures.rs`: `Goal::Challenge` (appended), the five fields, zeroed in the constructor.
- `behavior/territory.rs`: `deposit`, `decay`, and their calls from `pressure()` and
  `day_boundary` (after the pressure decay, before `disease::decay_cells`), plus the kill
  deposit in `resolve_kill`.
- `src/sim/behavior/tests_territory.rs`: `adult_marks_and_juvenile_does_not`,
  `holder_changes_only_below_hold_min`, `decay_frees_ground_after_nineteen_days`,
  `kill_adds_kill_mark`.
- Deposit and decay do not move the checksum on their own: nothing reads the grid yet, and
  the checksum hashes named fields, not the grid. `just test-unit sim::tests::checksum`
  must still be green at the end of this step; that is the proof the wiring is pure.
- Tier: `just test-unit sim::behavior`, `sim::world`, `sim::tests::checksum`.

### Step 2 — avoidance, challenge, contest, eviction (D3–D7)
- `territory.rs`: `foreign`, `pick_intruder`, `preempt_predator`, `contest_contacts`.
- `perception.rs`: the patrol score term (predators only); `hunt.rs`: the hunt score divisor.
- `goals.rs`: `preempt_predator` at the top of `update_one` for predators; the Challenge
  branch in `replan_predator`; `goal_satisfied(Challenge)`; the Challenge target refresh
  beside `update_hunt_stalk`.
- `behavior.rs`: `contest_contacts` between `hunt_contacts` and `scavenge_contacts`;
  `events.rs`: `Contest`.
- Tests in `tests_territory.rs`: `foreign_is_zero_for_own_holder_and_for_social_readers`,
  `hungry_reader_ignores_scent`, `patrol_prefers_held_ground` (two cells of equal
  `prey_pressure`, one foreign), `resident_challenges_an_intruder_on_held_ground`,
  `nobody_challenges_off_held_ground`, `contest_evicts_loser_and_overmarks_cell` (rng
  seeded so the resident wins), `eviction_ends_after_evict_ticks_and_resumes_patrol`,
  `mutual_border_challenge_resolves_once`, and the tripwire
  `neutral_territory_reproduces_the_old_checksum`: seed 1, the pinned tick count, the
  neutral overlay, asserting `0xf883_9b51_57e3_c3f8`.
- Re-baseline `sim::tests::checksum_is_fnv_stable` with a comment; update `AGENTS.md`.
- Tier: `just test-unit sim::behavior`, `sim::tests::checksum`; `just test-chunk predators`
  and `herbivores` in the background (prey behaviour must be untouched: `herbivores` is the
  control for that).

### Step 3 — headless output and stats
- `main.rs`: `contests_<species>` (sum of `contests_won` over the living plus the dead
  tally kept in `DeathTallies`' neighbour, or simply the count of `Contest` events per
  species from the ring) and `nn_dist_<species>` from a new
  `stats::groups::nearest_neighbour_mean` computed in the midnight census beside
  `group_census`, O(living²) per species at worst but predators number in the tens.
- `examples/bench_c5.rs`: keys `mark_per_tick`, `avoid_w`, `resident_bonus`,
  `contest_injury`, and a `territory=off` shorthand for the neutral overlay; the summary
  line adds contests and the nearest-neighbour means.
- Tier: `just test-unit sim::stats`, `just test-chunk headless` in the background.

### Step 4 — screens
- S03: the two Life rows and the Condition line of D10; `docs/screens/s03-creature-inspector.md`.
- S07 needs nothing: `Contest` uses the generic event row; check the ticker text fits 155.
- S02j / S14: `Base::Scent`, `Layer::Base(Scent)` in the row table, the sidebar section
  (`‹Species› scent`, legend, `By region` holders count, `Reading the map`), `Tab` cycling
  predators only; `docs/screens/s02-map-overlay.md` row S02j and `s14-overlay-switcher.md`.
  Cut this bullet, not the others, if the modal has no spare row.
- `cargo test --lib -- --ignored regenerate_screen_renders` for S03b, S02j, S14; every glyph
  through `all_glyphs_are_cp437`.
- Tier: `just test-unit ui`; `cargo test --test components` only if a widget changed.

### Step 5 — documentation
- `docs/chunks/c5-predators.md`: "FR13 Territory" after FR12 with D1–D7 in requirement
  form, the D8 table, the neutral control, and a balance row; the status note gains the
  result of step 6.
- `docs/chunks/c8-sociality-and-maturity.md`: one line in Out pointing here.
- `AGENTS.md`: `VERSION = 17`, the checksum, the affected-tests line; `README.md` if it
  lists the behaviours.

### Step 6 — verification and the first balance read
- `just check`; the unit tiers above; the three chunk binaries.
- `scripts/sweep.sh 1 6 5` twice, defaults and `territory=off`, and compare in
  `summary.csv`: `nn_dist_fox`, `nn_dist_lynx`, `nn_dist_wolf` (the social control: it
  should not move), fox count and vole count at years 1 and 2, `contests_<sp>`, and the
  C5 bands `six_species_five_years`, `hunt_success_band`. The claim that must hold before
  any doc says so: solitary predators' nearest-neighbour distance is higher than the
  control on at least 5 of 6 seeds. The hoped-for and unpromised result is voles alive at
  year 2 on more seeds than today.
- One acceptance claim in `tests/predators.rs`, `territory_spaces_foxes_on_seed_42`:
  seed 42, one year, foxes' `nn_dist` under defaults exceeds the neutral overlay's by a
  margin fixed from the sweep; and `contests_are_non_lethal_and_logged`: at least one
  `Contest` event and no `Injury` death in that run.
- Record the numbers in the C5 balance row and in memory (`territory-findings`).

---

## Test plan summary

| Layer | Test | Guards |
|---|---|---|
| params | `territory` unit tests, `field_docs_complete`, `validate` | formulas, neutral control, docs |
| world | `mark`/`mark_mut`/`init_scent`, empty-grid default | indexing, old fixtures |
| save | `older_version_rejected`, `version_mismatch_rejected`, `round_trip_checksum_3_seeds` | format, grid round-trips |
| behaviour | `tests_territory` (12), `neutral_territory_reproduces_the_old_checksum` | D2–D7, exact gating |
| determinism | `checksum_is_fnv_stable` (re-baselined), the neutral tripwire | draw order |
| chunk | `predators`, `herbivores`, `headless` in release | C5 bands, prey untouched |
| UI | render regeneration, `ui` tier, CP437 scan | 155 × 45 budgets |

## Risks and how the plan handles them

- **Spacing starves the solitary predators.** A fox that keeps to its range loses prey it
  would have followed. `avoid` already fades with hunger; if the sweep shows fox starvation
  up against the control, the first lever is `avoid_w`, then `notice_min`; the tests are not.
- **Contests every tick at a border.** Two residents on adjacent cells could challenge each
  other on every replan. `contest_cooldown_days` on both sides and `challenge_ticks` bound
  it; `contests_<sp>` in the sweep is the number to watch, and a per-region daily aggregate
  like `Wary`'s is the fallback if the log floods.
- **Ghost ownership.** A dead holder's ground is foreign for up to 19 days. Accepted for
  the slice; if it visibly leaves holes, decay the holder to none when strength drops under
  `notice_min` (one comparison in `decay`).
- **Hot loop cost.** One index and one multiply per visited cell for predators only, plus a
  `Peer` scan the replan already does. `--profile` at 1 000 creatures should show under 2 %
  change against the control; measured in step 6, recorded in `docs/PERFORMANCE.md` only if
  it exceeds that.
- **Argument counts.** `contest_contacts` needs seven inputs; `preempt_predator` four. The
  contest pass takes a small `ContestCtx<'_>` borrow bundle, not an `allow`.
- **Save size.** 288 KB of marks per save; acceptable, and the grid could be stored sparse
  (non-zero marks only) at v18 if it ever matters.
- **The S14 modal row.** Adding a sixth base row may not fit 21 rows with the description
  pane; D10 names the cut and the fallback.

## Result (2026-09-21)

Everything through step 5 landed; step 6 ran as one-, two- and five-year sweeps on seeds
1–6 plus `bench_c5` on seed 42, all against the neutral overlay. The numbers are in
[C5 FR13](chunks/c5-predators.md#fr13-territory-scent-avoidance-and-contests-2026-09-21).
The short version:

- The **spacing claim failed**: fox nearest-neighbour distance is higher on three seeds and
  lower on three, and on the one seed with equal fox counts it is lower. The raw mean follows
  the fox count and the prey distribution; a count-normalised index is the follow-up.
- **Vole persistence improved** at year one on three seeds (80 vs 2, 36 vs 17, 14 vs 1) and
  was flat or slightly worse on the rest; year-one extinctions are equal over the six seeds.
- The **five-year collapse is unchanged** (every predator gone on every seed either way),
  so the C5 bands keep their status, as this plan promised rather than hoped.
- **Cost** +0.4 % of step time; the neutral tripwire holds the old checksum exactly.

## Follow-ups not in this plan

All recorded under *Territory follow-ups* in [feature-ideas.md](feature-ideas.md):
a count-normalised spacing index (Clark–Evans) for `--summary`, cross-species scent avoidance on the same grid, lethal contests through `Cause::Injury`
once it has an event kind and an S15 fate, the wander bias, home anchors and explicit
territory records, boundary patrol, dispersal at adulthood, mating at boundaries, range
quality and abandonment, the prey landscape of fear, kill-site defence, and the range
fidelity trait after the genome redesign.
