# Quirks — named birth oddities (built 2026-09-22)

Quirks are a `WorldBox` / Crusader Kings 3 style layer of named oddities a creature
is born with: Giant, Albino, Swift, Bloodthirsty, Undying and so on. They are
**for fun, not realism**, and they are **off by default**. The switch is the
`Quirks` row on the S09 New World form, or `quirks.enabled = true` in a
`--params` overlay for headless runs.

"Trait" already means a genome slot (S03/S04/S15, the `t` key), so this feature
is called *quirks* everywhere: code, params and UI.

## Decisions

- **The effects are real, and sometimes big.** Most quirks are moderate
  multipliers. Nine legendary quirks are strong.
- **Quirks are birth-only.** They are rolled or inherited at birth and never
  earned later.
- **Species-as-data applies.** The catalogue lives in `[[quirks.catalog]]` and
  merges by `name` like `[[species]]`. An overlay can retune or add a quirk
  without a rebuild. Catalogue order is the bit index of the mask, so append
  new quirks at the end.

## Mechanism

| Piece | Where |
|---|---|
| `QuirkParams`, `QuirkDef`, validation, `summary()` | `src/sim/params/quirks.rs` |
| The default catalogue (44 entries) | `src/sim/params/quirks/catalog.rs` |
| `QuirkSet` (u64 mask), `QuirkMods` (folded multipliers), `roll`, `assign_from` | `src/sim/quirks.rs` |
| Per-creature state | `Creature.quirks`, `Creature.qm`; `LineageNode.quirks` |
| Event | `EventKind::Quirk` (rare/legendary births only; shares the S07 mutation chip) |

**Rolling.** A founder or pup first inherits: each inheritable quirk of each
parent passes with `inherit_chance`. Then, with `birth_chance`, it rolls a fresh
quirk weighted by tier (`common_weight`, `rare_weight`, `legendary_weight`). With
`extra_chance` it rolls another, up to `max_per_creature`. The `kinds` field and
both directions of `excludes` are respected throughout.

**Determinism.** Each roll uses a throwaway
`Rng::new(seed ^ QUIRK_SALT ^ id·k)`, so no stream is threaded through the step,
and the three existing streams never move. Founders are rolled at the end of
`Sim::new`. Newborns are rolled right after `behavior::tick_creatures`: every
living creature with `id ≥ next_id` from before the tick.

**Off is exactly off.** With quirks off:
- nothing is rolled;
- every mask is 0 and every multiplier is exactly 1.0;
- `scaled` and `scale_ticks` return their input unchanged at 1.0;
- the mask is hashed into `Sim::checksum` only while enabled.

So `checksum_is_fnv_stable` keeps `0xf9ea_eb02_3e27_c085`. The save format is
version 21.

**Hooks.** Hot loops read `c.qm`, never the catalogue.

| Multiplier | Hook |
|---|---|
| speed | `behavior/movement.rs` `move_speed` |
| hunger | `behavior.rs` `needs` |
| kill / evade | `predation::Contest.quirk = pred.qm.kill / prey.qm.evade`, applied to the clamped total; S17 shows the same odds |
| sense, camouflage, sociality | the `Creature::{sense, sense_cells, camouflage, sociality}` accessors, used by detection, threat, perception, hunt, groups and territory |
| flee | `quirks::scale_ticks(pp.flee_ticks, qm.flee)` in `goals.rs` and `hunt.rs` |
| litter | the fertility term of `litter_size` in `genetics.rs` |
| lifespan, maturity | `Creature::{max_age_days, adult_age_days}` |
| susceptibility | `disease::effects::susceptibility` |
| Cannibal | `hunt::pick_hunt_target`: at or above `cannibal_hunger`, a predator may target juveniles of its own species. A missed juvenile predator is not forced to flee. |

A founder whose lifespan shrinks has its `born_day` scaled by the same factor,
because its age was drawn against the unquirked lifespan.

## UI

- **S09:** the `Quirks  ◄ off ►` row in the Evolution section. `←` `→` or
  `Space` toggles it. The form carries the whole `[quirks]` table, so a
  `params.toml` catalogue survives.
- **S03:** a `─ Quirks ─` block in the Identity column, shown only while
  enabled. Each row is `φ`/`Φ`, the name and the effect summary. The name line
  also gets a `φ`/`Φ` badge.
- **S01e (follow sidebar):** one `φ Swift · Giant` line, only for a creature
  that has quirks.
- **S11:** a two-glyph legend under Seasons & time.

## Catalogue (44)

Tiers are C (common, weight 10), R (rare, 3) and L (legendary, 0.2, never
inherited). Kinds are `any` unless marked P (prey) or Pd (predator).

- **Physical:**
  - Giant (hunger 1.3, kill 1.15, evade 1.1, speed 0.9)
  - Dwarf (hunger 0.75, camo 1.2, speed 1.05, kill 0.85)
  - Swift (speed 1.3, hunger 1.1)
  - Sluggish (speed 0.75, hunger 0.9)
  - Keen-eyed (sense 1.4)
  - Myopic (sense 0.6)
  - Albino R (camo 0.3, sense 0.85)
  - Melanistic R (camo 1.3)
  - Iron Gut (hunger 0.8, disease 0.9)
  - Glutton (hunger 1.35, litter 1.1)
  - Frail (evade 0.8, lifespan 0.85, disease 1.2)
  - Robust (evade 1.2, lifespan 1.1)
  - Thick Hide R (evade 1.35, speed 0.95)
  - Lean (hunger 0.85, evade 0.9)
- **Temperament:**
  - Brave (flee 0.5, kill 1.1)
  - Cowardly (flee 2, evade 1.15, kill 0.8)
  - Bloodthirsty Pd (kill 1.25, hunger 1.15)
  - Gentle Pd (kill 0.8, hunger 0.9)
  - Loner (social 0.3)
  - Gregarious (social 1.8)
  - Skittish P (flee 1.5, sense 1.15)
  - Stoic (flee 0.7, disease 0.9)
  - Reckless (kill 1.15, evade 0.8, flee 0.6)
- **Life history:**
  - Fertile (litter 1.4)
  - Poor Breeder (litter 0.6)
  - Long-lived (lifespan 1.3)
  - Short-lived (lifespan 0.7)
  - Precocious (maturity 0.7, lifespan 0.9)
  - Late Bloomer (maturity 1.4, lifespan 1.15)
  - Twin-bearer R (litter 1.8, hunger 1.15)
- **Health:**
  - Hardy (disease 0.6)
  - Sickly (disease 1.6, lifespan 0.9)
  - Plague-proof R (disease 0.15)
- **Luck:**
  - Lucky R (evade 1.2, kill 1.1)
  - Cursed R (evade 0.8, kill 0.9, disease 1.2)
- **Legendary:**
  - Undying (lifespan 4)
  - Titan (hunger 2, kill 1.6, evade 1.6, speed 0.9)
  - Ghost (camo 3, clamped; sense 1.3)
  - Windrunner (speed 1.8, flee 0.8)
  - Oracle (sense 2.5)
  - Blessed (evade 1.3, disease 0.4, litter 1.4, lifespan 1.3)
  - Brood Queen (litter 2.2, lifespan 1.5)
  - Alpha Pd (kill 1.4, social 2, litter 1.3)
  - Cannibal Pd (eats own young, hunger 0.9)

Opposites exclude each other. The exact pairs are the `excludes` lists in
`catalog.rs`.

**Phase 2 ideas** (each needs its own code path, so none are built):
- Nemesis: a bonus against one prey species.
- Wanderer: migration.
- Scavenger: prefers carcasses.
- Nocturnal: an individual override.
- Territorial: a scent-contest bonus.

## Acceptance

- **`tests/quirks.rs`** (`just test-chunk quirks`):
  - Off is checksum-identical to a run without the feature.
  - On is deterministic and announces rare births.
  - Inheritance at `inherit_chance = 1` carries parental quirks in at least 80%
    of pups.
  - An all-Undying world has under a quarter of the baseline's age deaths.
  - Saves keep masks and multipliers.
- **`sim::quirks` unit tests:**
  - catalogue validity;
  - identity fold;
  - exclusions, kinds and max over 2 000 rolls;
  - inheritance rate within ±0.04;
  - legendary quirks are not inherited;
  - founders and newborns are rolled in a live sim.
- **UI tests:**
  - `s09_worldgen::tests::quirks_toggle_is_off_by_default_and_reaches_the_sim`;
  - `s03_inspector::tests::s03_quirks_section_only_when_on`.

## Balance note (2026-09-22)

The sweep used seeds 1–8, 5 years, default params, `--summary`.

| | Extinctions |
|---|---|
| Quirks off | 36 |
| Quirks on | 37 |

Worlds collapse about as often either way. That collapse is the known C5/C6
open item, and quirks neither cause it nor fix it.

At the default weights, legendary quirks are roughly 0.5% of fresh rolls, about
one legendary pup per 1 200 births. Raise `legendary_weight` for a more
mythic world.
