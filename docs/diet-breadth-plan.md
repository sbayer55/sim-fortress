# Diet breadth — implementation plan

*Written against `main` at 85fd8fc (the overlay switcher merge, PR #22) on 2026-09-17.
Save format `VERSION = 13`, checksum `0xd0e3_ee1a_c665_f531`, twelve genome slots.*

*Implemented 2026-09-17 on this branch. Deviations from the text below: `perceive` kept a
plain seventh argument (`clippy::too_many_arguments` is allowed crate-wide in `src/lib.rs`,
so no `GrazeRules` bundle was needed); S04a paid for the `Dbr` column with the trend strip
(14→13) and the pre-Diet spacer (3→2) instead of the count columns, whose headers collide
when trimmed; S04b's grid keeps three histogram rows per block and reclaims the panel's top
spacer row rather than folding the axis into the footer; S03 carries `none recorded` in the
Mutation-history section rule and drops both blank rows to fit the thirteenth trait;
`src/sim/mod.rs` crossed the 800-line ceiling by three lines, so its test module moved to
`src/sim/tests.rs` (pure code motion). Checksum re-baselined to `0xf883_9b51_57e3_c3f8`,
save `VERSION = 15` (main took 14 for the last-ate/drank/slept stamps while this was in review). The sweep result is recorded in the C4 balance table.*

**Goal.** Add the thirteenth genome slot, **Diet breadth**, from
[feature-ideas.md](feature-ideas.md): for herbivores, which terrains the animal can graze,
from dense-grass specialist to forest browser. Specialists bite faster; generalists keep
eating when the meadows are grazed down or die back in winter. The expected visible effect
is deer and hares spreading across terrain instead of stacking on the best cells, and the
`Dbr` drift row moving in opposite directions on lush and harsh worlds.

**Non-goals.** No change to vegetation growth, caps or regrowth (`sim::ecology` is not
touched). No new terrain, glyph, overlay, event or key. No per-species diet centre: the
`diet` string on a species stays display-only. Predators carry the slot but it is inert for
them, exactly as their grazing code is. No save migration: v13 files are refused like every
earlier bump.

**Starting point.** Grazing is three sites and one parameter block. `perception::perceive`
scores every vegetated cell in the sense ellipse as `vegetation / (1 + d/4)` (herd bias
aside) with no terrain term; `goals.rs` re-scores the current cell with the same rule to
decide whether to graze in place; `vitals::graze` bites `min(vegetation, graze_per_hour)`
and relieves `graze_nutrition × bite` of hunger. Terrain only enters through the vegetation
cap (`ecology.max_vegetation`: meadow 1.00, marsh 0.90, forest 0.85, grassland 0.80, sparse
0.50, dirt 0.30, sand 0.10), so a forest cell and a meadow cell are the same food today.
The Mutability commit (`383e3c8`) is the template for a new slot: it touched 32 files, and
its footprint is the checklist in step 1 below.

**Standing constraints.** Everything in `AGENTS.md`. The three that bite here: the
`perceive` cell loop is the hot path (`docs/PERFORMANCE.md`), so no map lookup per cell;
every tunable has a `FIELD_DOCS` entry; the S04a species row has zero spare columns
(the Mutability commit trimmed the trend strip to fit twelve traits), and the S04b
histogram grid is a full 3 × 4.

---

## Design decisions

**D1 — Slot 12, one axis, anchored at grass.** `IDX_DIET_BREADTH = 12`, `N_TRAITS = 13`,
name `"Diet breadth"`, abbreviation `"Dbr"`, colour `theme::FOREST_FG`. Each vegetated
terrain gets a fixed **position** on a grass→browse axis, and a creature with breadth `b`
eats everything at or below `b` on that axis, with a soft edge above it:

| Terrain | position | eaten fully once `b` ≥ |
|---|---|---|
| GrassDense (meadow) | 0.00 | always |
| Grass | 0.10 | 0.10 |
| GrassSparse | 0.25 | 0.25 |
| Marsh | 0.40 | 0.40 |
| Dirt | 0.55 | 0.55 |
| Sand | 0.65 | 0.65 |
| Forest | 0.90 | 0.90 |

The alternative — a per-species diet centre with breadth as a window width around it —
was rejected: it needs a new `[[species]]` field, a fourth number to tune per animal, and
does not match the feature text ("from dense-grass specialist to forest browser"), which
is one axis. Anchoring at grass also means the base genomes read directly: a vole at 0.35
is a grass animal, a deer at 0.85 is a browser.

**D2 — Two formulas, both in `DietParams`, shared by behaviour and the inspector.**
- `edibility(b, p) = clamp(1 − (p − b) / edge, 0, 1)` — 1 for terrain at or below the
  creature's reach, falling to 0 over `edge` (default 0.15) above it.
- `bite(b) = 1 + (1 − b) × specialist_bonus` — a pure specialist bites 1.5× per hour at
  the default `specialist_bonus = 0.5`; a full generalist bites 1.0×.

Specialists pay in range, generalists pay in bite rate. There is no third cost: the point
is a trade-off selection can resolve differently per world, not a handicap.

**D3 — Where the formulas act.** Three sites, no new goal, no new state on `Creature`:
- `perceive`: cell score becomes `vegetation × edibility / (1 + d/4)` (herd divisor
  unchanged after it), and a cell is a candidate only when `vegetation × edibility ≥
  graze_min_vegetation`. Edibility is read from a `[f32; 10]` table built **once per
  `perceive` call** from the creature's breadth (`DietParams::table(b)`), indexed by
  `terrain as usize`, never a `BTreeMap` lookup in the loop.
- `goals.rs` graze-in-place check: same edible-vegetation rule for the current cell, so a
  specialist standing in forest walks to grass instead of grazing nothing.
- `vitals::graze`: bite `g = min(vegetation, graze_per_hour × bite(b))`, the cell loses
  `g`, hunger drops by `graze_nutrition × g × edibility`. Removal is the bite, nutrition
  is what the animal can digest.

`genetics::eligible`'s "prey stands on vegetation ≥ minimum" mate check is left as is: it
is a fed-ness proxy, and switching it to edible vegetation would move mating for reasons
unrelated to diet. Noted as a known simplification in the C4 doc.

**D4 — A neutral control exists by parameters alone.** Setting every `terrain_position`
to 0 and `specialist_bonus` to 0 makes every terrain fully edible at every breadth and the
bite 1.0×: behaviour is then identical to today except for the extra founder jitter draw.
Tests and sweeps use that overlay as the control, in the same spirit as
`social.group_size_max = 0` for C8. No `enabled` flag in the hot loop.

**D5 — Base genomes.** vole 0.35 (meadow, grassland, sparse; marsh at half), hare 0.50
(adds marsh fully and dirt at two thirds), deer 0.85 (everything but forest at two thirds,
so forest is a browser's food only once selection lifts it past 0.90). Fox, wolf and lynx
0.5, inert. Founders jitter with the usual `N(0, 0.12)`, so a deer lineage can start
either side of the forest threshold.

**D6 — Save `VERSION = 14`, checksum re-baselined.** `Params` gains a `[diet]` table and
`BaseGenome` a field, both serde-visible, and every founder draws one more gaussian. Both
bumps are deliberate; the checksum test comment says why, as the Mutability one does.

**D7 — Screen budgets are paid explicitly.**
- S04a: the thirteenth 4-cell column comes from the fixed count columns (`Adults` 7→6,
  `Birth/d` and `Death/d` 8→7, the 3-cell spacer before Diet →2), leaving the Diet fill at
  its current width so `grass, leaves` still fits.
- S04b left panel: the histogram grid goes from 3 × 4 blocks of 7 rows to **3 × 5 blocks of
  6 rows** (the axis line folds into the footer: `0 ────┼──── mode .71 1`), the key line
  moves into the panel's right-hand hint, and the spare blank in Selection pressure goes.
  The "Compared with other species" table drops its two-cell indent and one space between
  `count` and `gen` so thirteen 4-cell columns stay inside 78. Two blocks stay empty.
- S04b right panel: thirteen drift rows and thirteen per-generation rows fit (the render has
  blank rows under both).
- S03 Genome column: the trait table, the Derived list and the Offspring forecast each grow
  one row. The three rows come from the blank after `from N lines; rate …`, the blank after
  Derived, and the forecast's header merging into its section rule. Verified by the render
  test, not by eye.

**D8 — Documentation lands in C4.** The genome is C4's; Mutability was documented there as
an FR, and Diet breadth follows it as the next FR. C3's graze requirement gets a one-line
pointer. `feature-ideas.md` strikes the idea through with the date, as the shipped AI ideas
are.

---

## Data model

```rust
// src/sim/params/diet.rs — `[diet]`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DietParams {
    /// Grass→browse position of each vegetated terrain, 0..=1 (D1 table).
    pub terrain_position: BTreeMap<Terrain, f32>,
    /// Width of the soft edge above a creature's reach: edibility falls 1→0 over it.
    pub edge: f32,               // 0.15
    /// Bite multiplier at breadth 0: bite = 1 + (1 − b) × bonus.
    pub specialist_bonus: f32,   // 0.5
}
impl DietParams {
    pub fn edibility(&self, terrain: Terrain, breadth: f32) -> f32;
    pub fn bite(&self, breadth: f32) -> f32;
    /// Edibility by `terrain as usize`, built once per replan.
    pub fn table(&self, breadth: f32) -> [f32; Terrain::COUNT];
    /// Widest terrain fully eaten at this breadth, for the inspector.
    pub fn reach_name(&self, breadth: f32) -> &'static str;
}
```

`Terrain::COUNT = 10` is a new const beside `from_code`; water and rock have position 1.0
in `table` (never candidates anyway, `is_water` and `walkable` run first).

Genome: `IDX_DIET_BREADTH = 12`, `Genome::diet_breadth()`, `BaseGenome.diet_breadth`,
`TRAIT_NAMES[12] = "Diet breadth"`, `TRAIT_ABBR[12] = "Dbr"`. Nothing new on `Creature`.

`Params.diet: DietParams` sits after `social` in the struct (serde order is load-bearing;
this is the VERSION bump).

---

## Steps

Each step ends green on its tier before the next starts; steps 1 and 2 are the sim change
and are the only ones that move the checksum.

### Step 0 — `[diet]` params
- `src/sim/params/diet.rs` with `DietParams`, `Default`, the four helpers, and unit tests:
  `edibility_is_one_below_reach_and_zero_past_the_edge`, `bite_is_neutral_for_a_generalist`,
  `neutral_overlay_makes_every_terrain_edible`, `table_matches_edibility`.
- Register in `params.rs` (`pub mod diet; pub use diet::DietParams;`), add the field to
  `Params`, add the ten `FIELD_DOCS` lines (`params::tests::field_docs_complete` enforces
  both directions), `Terrain::COUNT`.
- `save::VERSION = 14`. `older_version_rejected` builds a synthetic v4 file in the test
  body, so it needs no fixture change; `version_mismatch_rejected` likewise.
- Tier: `just test-unit sim::params`, `just test-unit sim::save`.

### Step 1 — the slot (the Mutability checklist)
- `species.rs`: constants, accessor, names, abbreviation, base genomes in slot order, the
  three existing assertions extended (`TRAIT_NAMES[IDX_DIET_BREADTH]`, abbreviation is 3
  cells, predators at 0.5).
- `params/species.rs`: `BaseGenome.diet_breadth`, `genome()`/`from_genome()`, the six
  roster defaults.
- `ui/screens/common.rs`: `trait_color(12) => theme::FOREST_FG`.
- `main.rs`: `--summary`/`--header` column `diet_breadth_<species>` for prey, beside
  `mutability_<species>`.
- Everything that loops `Genome::LEN` (stats, CSV, inheritance, founders, S03/S04/S08)
  follows automatically; `inherit_covers_every_slot` proves the wiring.
- Re-baseline `sim::tests::checksum_is_fnv_stable` with a comment (one more founder
  gaussian per founder shifts every later draw). Update the value in `AGENTS.md`.
- Tier: `just test-unit sim::species`, `sim::genetics`, `sim::stats`, `sim::tests::checksum`.

### Step 2 — the mechanic
- `perception.rs`: `perceive` already has exactly six arguments (`c, spatial, world, cp,
  view, sp`), so a seventh trips clippy. Bundle `cp`, `sp` and the new `dp` into a small
  `GrazeRules<'_>` struct (three borrows, built once per tick in the caller), build the
  edibility table once per call, apply D3.
- `goals.rs`: the in-place check through the same table.
- `vitals::graze`: bite and nutrition per D3.
- `src/sim/behavior/tests_diet.rs`: `specialist_ignores_forest_and_walks_to_grass`,
  `generalist_grazes_forest`, `specialist_bites_faster_on_meadow`,
  `neutral_diet_reproduces_the_old_score` (score equality against the pre-change formula
  on the `all_grass_world` fixture). Add the module to `scripts/affected-tests.sh` if the
  glob does not already catch `behavior/*` → `herbivores`.
- Tier: `just test-unit sim::behavior`; `just test-chunk herbivores` and `evolution` in the
  background.

### Step 3 — screens
- S03: thirteenth genome row (label width 11→12 for "Diet breadth"), the forecast row, and a
  Derived line for prey only: `diet  grazes up to marsh · bite ×1.3` from `reach_name` and
  `bite`. Predators show nothing new.
- S04a: the column and the width trims of D7.
- S04b: the 3 × 5 grid, footer fold, key-line move, comparison trim.
- `cargo test --lib -- --ignored regenerate_screen_renders` for S03a, S04a, S04b; check the
  155 × 45 renders have no overflow and every glyph passes `all_glyphs_are_cp437`.
- Tier: `just test-unit ui`, `cargo test --test components` only if a widget changed.

### Step 4 — documentation
- `docs/chunks/c4-evolution.md`: a "Diet breadth (slot 12)" FR after Mutability: formulas,
  defaults table, the neutral control, the mate-check simplification, a balance row.
- `docs/chunks/c3-herbivores.md`: one line under the graze requirement pointing at it.
- `docs/screens/s03-creature-inspector.md`, `s04-species-browser.md`: the new row, column,
  grid and trims. `docs/feature-ideas.md`: strike-through with the date.
- `AGENTS.md` and `README.md`: "twelve-trait" → "thirteen-trait", `VERSION = 14`, checksum.

### Step 5 — verification and a first balance read
- `just check`; the unit tiers above; `just test-chunk headless` (params, save, main).
- `scripts/sweep.sh 1 6 5` twice, defaults and the neutral overlay, and compare in
  `summary.csv`: prey counts at year 5, `diet_breadth_<prey>` means, and the C3/C4 bands
  (`five_year_survival`, `floor_five_percent`, `food_is_findable`). If a band breaks, the
  levers are `specialist_bonus` and `edge`, and deer's base value; the tests are not.
- One integration claim in `tests/herbivores.rs`, `diet_breadth_spreads_grazers`: prey-only
  world, one seed, one year, deer base breadth 0.2 versus 0.95; the share of grazing deer
  standing on Forest is higher for the browser lineage and near zero for the specialist.
- Record the numbers in the C4 balance row and in memory (`diet-breadth-findings`).

---

## Test plan summary

| Layer | Test | Guards |
|---|---|---|
| params | `diet` unit tests, `field_docs_complete` | formulas, neutral control, docs |
| genome | `species` assertions, `inherit_covers_every_slot`, checksum lock | wiring, determinism |
| save | `older_version_rejected`, `version_mismatch_rejected`, `round_trip_checksum_3_seeds` | format |
| behaviour | `tests_diet` (4) | the three sites, old score under the neutral overlay |
| chunk | `herbivores`, `evolution`, `headless` in release | C3/C4 bands hold |
| UI | render regeneration, `ui` unit tier, CP437 scan | 155 × 45 budgets |

## Risks and how the plan handles them

- **Starvation on harsh worlds.** A specialist vole in a dieback winter now has fewer cells.
  The neutral overlay isolates the effect; `edge` softens the cliff; the sweep in step 5 is
  run before any doc claims a band. If voles collapse, raise vole's base to 0.45 before
  touching the formula.
- **Hot loop cost.** One table build per replan and one multiply per cell; `--profile` at
  1 000 creatures should show under 2 % `step_ns` change against the control. Measured in
  step 5, recorded in `docs/PERFORMANCE.md` only if it exceeds that.
- **Argument-count lint on `perceive`.** Resolved by the bundle in step 2, not by an
  `allow`.
- **S04b relayout is the largest UI edit.** It is isolated in `histograms.rs` and guarded by
  the render test; if the 6-row block loses too much, the fallback is 5 rows of 7 with the
  Compared table moved to the right panel's blank rows.
- **Selection may be weak at 5 years**, as it was for Mutability and Resistance. The plan
  claims spread across terrain, which is immediate, not drift, which is a 10-year run.

## Follow-ups not in this plan

- A **diet overlay** on S14 (tint each cell by the selected species' mean edibility) —
  the first overlay that would show a genome on the map.
- **Prey preference as a trait** for predators, the carnivore counterpart named in
  `feature-ideas.md`.
- Switching the mate check to edible vegetation once dens or refuges make it matter.
