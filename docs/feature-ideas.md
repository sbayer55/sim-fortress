The roadmap's eight chunks are all shipped, so the next feature is genuinely open. Here are the candidates I find most interesting, grouped by what they add, with the one I'd pick first at the top.

**My pick: burrows and dens (the vole refuge).** The predator chunk's own status note says the population bands are unreachable by tuning alone and names the missing mechanism: voles have no refuge, and wolves and lynxes die without meeting a mate. A burrow cell is a home tile that a vole retreats to when fleeing, is undetectable inside, and rests in at night. A den is the predator version: a pack home that gives wolves and lynxes a mate-seeking target and a place to raise a litter. This is one mechanic that fixes two open balance problems, adds a visible new glyph on the map, and gives the inspector a "home" field to follow. It builds directly on the existing Flee, Rest and Mate goals.

**Other simulation-side ideas, roughly in order of payoff:**

- **Carcasses and scavenging as real objects.** Scavenge is already a goal, but a kill could leave a decaying carcass tile that foxes and voles compete over, and that fertilises the vegetation beneath it. Cheap to add, and it produces the kind of emergent scene players screenshot.
- **Weather beyond drought.** Snow cover that hides grass in winter, floods that push rivers outward, wildfire that burns forest into sparse grass and regrows. Fire in particular is visually striking in CP437 and interacts with every layer already there.
- **Speciation.** When two lineages of the same species drift far enough apart in genome distance, they stop interbreeding and the browser shows a split. The lineage screen and genome already exist, so this is mostly a distance check plus a naming scheme.
- ~~**Territory and scent marking.**~~ *First slice shipped 2026-09-21 as C5 FR13 (plan in [territory-plan.md](territory-plan.md)): the per-species scent grid, scent avoidance and the intrusion response. The sweep did not show the spacing the sentence below promises; see the plan's Result section. The rest of the brainstorm is parked under [Territory follow-ups](#territory-follow-ups).* Predators leave a decaying scent field that repels rivals and attracts mates. It naturally spaces predators out and gives you a new map overlay.

**C8 follow-ups: seeing sociality and packs.** The mechanics shipped, and S04a/S05e now count groups, but a pack still has no identity of its own. This idea does not change behaviour; it makes the behaviour that already exists legible and testable.

- **Pack identity tracking.** A pack today is inferred fresh each tick from `hunt_target` and `kin_nearby`, so it has no name, no members and no continuity: two wolves on the same deer are a pack this tick and strangers the next, and the event log can only say "a pack of 4 wolves" when they happen to migrate together. A persistent pack record — members, a leader, a tag like `W#110`, and formed/dissolved events — would let the inspector show "pack: Ashfall, 4 members, leader Y10", let the lineage screen group by pack rather than by family, and let the map draw members together. It is also the stable group that the dens idea above and territory marking both need to attach to.

### Territory follow-ups

Parked on 2026-09-21 when the territory brainstorm was narrowed to the scent grid, scent
avoidance and the intrusion response. Each of these builds on that slice; none is needed for it.

*Other ways to represent a territory.* The scent grid is deliberately ownerless. These add
ownership, in increasing order of weight:

- **Home anchor per creature.** `Creature.home: Option<(x, y)>`, set at adulthood or first kill,
  with a spring bias back toward it in `wander`/`patrol` and a radius scaled by size and local prey
  density. No new world state; a territory is just the set of anchors. The natural place to hang
  "distance from home" and "range quality" in S03.
- **Explicit territory records.** `World.territories: Vec<Territory>` with owner (creature or
  pack), centre, radius, species and claim day, in the style of `world.dens`. Claim, lose,
  inherit and abandon become events. Group territories need *pack identity tracking* above first.
- **Region tenure.** Cap residents per (species, region) and let `migration_daily` enforce it.
  Crude and invisible beyond S02e, but a one-afternoon experiment on whether spacing predators
  moves the C5 bands at all.

*Behaviours that hang off a territory.*

- **Boundary patrol.** Patrol today heads for the highest `prey_pressure` cell; bias it toward the
  rim of the home range where foreign scent is strongest, which is literal patrolling and reads
  differently on the map.
- **Dispersal at adulthood.** Juveniles walk out of the natal range until scent is low. This is
  what actually spaces predators out, creates a floater population, and gives wolves and lynxes
  a reason to travel toward other territories (the C5 mate-encounter problem). Pairs with the
  *Dispersal* trait below.
- **Mating at boundaries.** Holders of adjacent ranges of opposite sex meet at the shared edge, or
  a mated pair shares one anchor; a den could be the pair's anchor, tying this to burrows and dens.
- **Range quality and abandonment.** Sum vegetation or prey pressure over the range each day; a
  holder whose range quality collapses abandons it and falls into the migration path, so territory
  and migration stop being separate systems.
- **Landscape of fear for prey.** Let prey read predator scent as risk in their graze score, not
  only live predators, so voles concentrate in the gaps between fox ranges. Prey home ranges
  anchored on the existing dens give the same refuge from the other side.
- **Kill-site defence.** A carcass inside the holder's range deters rival scavengers, so
  scavenging becomes spatially structured.

*Measuring it.* The raw mean nearest-neighbour distance (`nn_dist_<species>`) follows the
population count, which is why the first slice could not show spacing. A **Clark–Evans
index** (observed mean nearest-neighbour distance over the expected value for that many
animals on that much land) would make territory's spacing effect comparable across seeds
and years; one more `--summary` column and a line on S04a.

*Seeing it.* The S02j scent overlay, the S03 territory rows and the `Contest` event shipped
with the first slice. Still open: S03 rows for home, distance from home and range quality;
event kinds for claimed, usurped and abandoned feeding S07 and the chronicle prompt; and
`--summary` columns for mean range size and floater share per species.

**Player-facing ideas, if you want a break from ecology:**

- **The player's hand.** A small set of interventions: place a water hole, plant a forest, cull or introduce a founder group, start a fire. Turns pure observation into experimentation, and the headless runner could take the same commands from a script.
- **Chronicle screen.** An auto-written history of the world: "Year 3, the voles of the eastern meadow vanished; the foxes followed by winter." Generated from the event ring buffer, in the Dwarf Fortress legends style.
- **Quirks** *(built 2026-09-22, [quirks-plan.md](quirks-plan.md))*. WorldBox / CK3-style named birth oddities (Giant, Albino, Undying, Cannibal …), 44 in a `[[quirks.catalog]]`, off by default and switched on at world creation.
- **Named notable creatures.** The oldest, the most prolific, the best hunter, automatically named and tracked, with an obituary event on death.
- **Replay and time scrubbing.** Save periodic snapshots and let the charts screen jump the map back to any year.

**One infrastructure option:** you have a determinism rule and a headless mode, so a **scenario file** format (seed, params, scripted interventions per tick, assertions on counts) would let you regression-test balance claims like the C5 bands instead of re-running sweeps by hand.

If you want, I can write the burrows-and-dens idea up as a ninth chunk doc in the same format as the existing eight, with requirements, acceptance bands and a checkpoint demo script.


---

The current genome has thirteen slots: speed, size, sense, metabolism, aggression, camouflage, fertility, longevity, resistance (C7), sociality and maturity (C8), mutability, and diet breadth (2026-09-17). The most interesting additions are ones that trade off against an existing slot or against a specific mechanic, so selection can pull them in different directions in different worlds.

**Traits that exploit the existing environment**

- **Cold tolerance.** Lower winter metabolism cost, but a higher summer cost. Harsh-winter presets would select it up, lush ones down, so the same species diverges across presets.
- **Drought tolerance.** Thirst rises more slowly, at the price of a slower speed cap. It would make your dry-world migration events much less lethal for some lineages and not others.
- ~~**Diet breadth.**~~ *Shipped 2026-09-17 as genome slot 12 (`Dbr`), the `[diet]` table and C4 FR3; plan in [diet-breadth-plan.md](diet-breadth-plan.md).* For herbivores, the terrain types the creature can graze on, from dense-grass specialist to forest browser. Specialists eat faster, generalists survive dieback. This also spreads deer and hares across terrain instead of stacking on the best cells.
- **Nocturnality.** A phase shift on the sleep cycle. A nocturnal vole avoids a diurnal fox entirely, which is a refuge that needs no new tiles, and a nocturnal fox lineage would then be selected to chase them. Day and night tint already exists, so it is visible.

**Traits that shape predator and prey encounters**

- **Boldness.** The distance at which prey starts to flee. Timid animals lose grazing time, bold ones get eaten. Classic and very legible in the sense-ring overlay.
- **Vigilance versus foraging.** Fraction of a graze tick spent scanning. It directly trades hunger against detection, and it is the natural counterpart to camouflage.
- **Stamina.** Chase duration before speed collapses. Lets a slow predator with high stamina beat a fast prey, breaking the current speed arms race into two axes.
- **Prey preference.** For predators, a bias toward small or large prey. A fox lineage that stops bothering voles is another route to the vole refuge your predator notes ask for.

**Life-history traits**

- **Parental care.** Offspring start with more reserves and the parent loses hunger for a period after birth. Litters get smaller but survive. Nice interaction with any future dens.
- **Dispersal.** How far a newborn wanders from its parent before settling. Low values create local inbred pockets, high values spread genes and spark migrations. Very visible on the lineage tree.

**Traits that need one new mechanic**

- **Toxicity or spines,** a cost to the predator on a kill. Gives prey a defence that is not running or hiding.
- **Coat colour** as a numeric value matched against the terrain palette under the creature. Camouflage becomes terrain specific, so a forest vole and a meadow vole drift apart, and you could tint the glyph with the value.
- **Range fidelity** (nomad 0 … resident 1), once territory exists. Residents remember their water and dens so their needs cost less; nomads survive local dieback. Harsh presets and lush presets should select it in opposite directions. Note the UI budget: a fourteenth slot needs the S03/S04 genome redesign recorded in the diet-breadth notes, so until then territoriality is best derived from existing genes (aggression × (1 − sociality), range radius from size, mark reach from sense).

If you want the most result for the least code, I'd take nocturnality, boldness and mutation rate first. All three slot into behaviour code that already exists, and each one can be watched drifting on the charts screen without any new UI.

---

## AI-assisted features (opt in, via a local Bifrost gateway)

Every idea here is bound by [docs/ai-requirements.md](ai-requirements.md): AI is **off by
default and opted into per feature**, the game is byte-for-byte unchanged when it is off or
the gateway is unreachable, and **no model output ever enters the simulation step**. The one
provider is a local [Bifrost](https://github.com/maximhq/bifrost) instance speaking the
OpenAI-compatible chat API, which routes to Ollama for cheap interactive calls and to AWS
Bedrock for heavy reasoning. Each idea names its fallback: what the player gets with AI off.

**Shipped as C9 (2026-09-16): the species designer plus the chronicle narrator** — see
[chunks/c9-ai-designer-and-chronicle.md](chunks/c9-ai-designer-and-chronicle.md). One
upstream feature and one downstream feature, so together they forced the whole plumbing — the
`[ai]` config, the worker thread, the fake gateway for tests, the Options section and the
headless flags — that every later idea reuses. Neither touches `src/sim`.

**Downstream: the model reads the world and writes text.**

- ~~**Chronicle narrator.**~~ *Shipped in C9 as S07c and `--chronicle`; the template fallback shipped too.* The Dwarf Fortress legends screen above, written by a model from
  each season's slice of the event ring buffer plus the per-species census: "Year 3, autumn:
  the voles of Sedgehollow vanished; the foxes followed by winter." A chronicle mode on S07 and
  a `--chronicle` headless flag that writes Markdown next to `summary.csv`. *Fallback:* the
  event log as today; optionally a deterministic template chronicle ("Year 3: 2 extinctions,
  1 epidemic") that needs no model.
- **Named places.** The biome-region work gives each basin a terrain and moisture profile. A
  model names them once at world generation ("Sedgehollow", "the Ashfall shelf") and the names
  go into the save as an optional, decorative table, so events read "in Sedgehollow" instead of
  "in region 7". One call per world. *Fallback:* the index-based region labels used today.
- **Creature biographies and obituaries.** S03 already shows a genome, age, lineage, kills, kin
  and pack state; a key produces a three-line bio. Pairs with *named notable creatures* above:
  the oldest, the most prolific and the best hunter get a name and an obituary event.
  *Fallback:* the inspector as today; the key hint is not drawn.
- **Ask the world.** Natural-language questions answered by a tool-using model over the
  read-only `&Sim` API: species counts, region stats, event search, lineage lookup. "Why did
  the lynx die out?" becomes a chain of tool calls and a grounded answer. Bifrost's MCP support
  may carry the tool round-trips so the game needs no agent loop; spike before committing.
  *Fallback:* the menu entry is absent.
- **Run post-mortem.** After a headless run or sweep, hand `summary.csv` and the population
  series to a model for an ecologist's report: what crashed, when, and which lever it correlates
  with. `--postmortem` on the headless runner. *Fallback:* the CSV.

**Upstream: the model writes inputs the sim already accepts.**

- ~~**Species designer.**~~ *Shipped in C9 as S09b and `--design-species`.* "Add a boar: omnivore, big litters, forest dweller." The model emits a
  `[[species]]` overlay in the schema the roster already loads, `Params::validate` runs, and
  validation errors go back to the model until it passes. Species are data, so this needs no
  sim change at all. *Fallback:* hand-write the overlay and pass it with `--params`.
- **Preset and world from a prompt.** "A harsh archipelago where predators barely hold on"
  becomes a params overlay plus the worldgen levers (moisture, roughness, edge outlets). A text
  field on S09 beside the five presets. *Fallback:* the five presets.
- **Scenario files.** The infrastructure idea above (seed, params, scripted interventions per
  tick, assertions on counts), drafted by a model from a sentence. The same format is what the
  balance agent needs. *Fallback:* write the file by hand.
- **Balance-tuning agent** *(dev tool, `examples/`, never in the game's default path)*. The C4
  and C5 notes say some population bands are unreachable by levers alone. A loop where the
  model proposes parameter deltas, `scripts/sweep.sh` runs them and the model reads
  `summary.csv` and iterates is a real experiment on that claim. Routed to Bedrock: each
  iteration is one large reasoning call over a lot of numbers.

**Player-facing flavour.**

- **Field guide.** S11 help backed by retrieval over `docs/`: chunk specs, screen requirements
  and the `--dump-params` field comments. "What does Resistance do?" gets a grounded answer with
  the file it came from. Ollama embeddings plus a small chat model, fully local. *Fallback:*
  S11 as today.
- **Creature thoughts.** One line in S03 from the current goal and needs, cached per goal
  transition so it costs one call per goal change, not per tick. Bifrost's semantic cache makes
  the repeats free. CP437 only, so a text line rather than a speech bubble. *Fallback:* nothing
  drawn.

**Ranked by payoff per line of code:** species designer, chronicle, named places, post-mortem,
bio, preset-from-prompt, field guide, thoughts, ask-the-world, scenarios, balance agent.

---

## Cause of death by species and lineage

*Idea logged 2026-09-21. Not planned; the notes below are what the code already gives us
and what a build would need.*

**The question it answers.** Today you can find out *what* killed one creature (S03c, the
S07b death detail) and *which trait* correlates with each cause across one species (S15).
What nobody can see is the middle scale: for a species as a whole, and for each bloodline
within it, what is killing them, and how that mix shifts season by season. "Why did the
lynx die out?" is an S12 alert with no answer. "Are the Fenrir line dying of hunger while
the Howl line gets eaten?" is a question the lineage store could answer and no screen
asks. A cause-of-death breakdown is the piece that turns the death counts on S04a into an
explanation.

**What is already recorded.** No new simulation state is needed for the species half.

- `Cause` (`src/sim/creatures.rs`) has six variants: Starved, Thirst, Age, Injury,
  Predation, Disease. `Death` on the creature carries the cause, the day, the killer's id
  and the chase length.
- The life log (`src/sim/lineage/lifelog.rs`) keeps every death of the last 240 days per
  species with cause, genome, born day and the outcome counters, capped at 50 000 records,
  and is saved with the world (format 16).
- Every `LineageNode` carries `cause: Option<Cause>`, `outbreak` and `died_day`, and its
  `parents` link, so any node can be walked up to its oldest surviving ancestor.
- The event ring buffer has `DeathStarved`, `DeathThirst`, `DeathPredation`, `DeathAge`
  and `DeathDisease` events, each with a species, for the seasonal series.

**Two gaps worth closing first.**

1. **A death record does not remember the killer's species.** `Death.killer` is a
   `CreatureId` and the life log drops it entirely, so "predation" cannot be split into
   "by foxes" and "by lynxes" once the killer's slot is reused. Add `killer_species:
   Option<SpeciesId>` to `LifeRecord` (and to the lineage node). One field, snapshot at
   `behavior::kill`, save-format bump.
2. **A creature has no lineage id.** A lineage is only reconstructable by walking
   `parents` to the root, and `Lineage::prune` drops old generations, so two descendants
   of the same founder stop sharing a root once it is pruned. Give every founder a
   `LineageId` (its own `CreatureId`) and inherit the mother's at birth, stored on the
   creature, the lineage node and the life record. Cheap, deterministic, and the same key
   the speciation and pack-identity ideas above want.

**Species view: extend S04b or S15.** The species detail already shows births, deaths and
disease deaths; the natural home for the breakdown is a section on S04b, or the
`Population` bar at the bottom of the S15 sidebar promoted to a panel. Proposed content
for one species:

- A 240-day cause bar and table: cause, count, share, and the mean age at death, in the
  same order as S15's outcome columns so the two screens read alike.
- Predation split by killer species (needs gap 1): `preyed  63%  by foxes 51%, by lynxes
  12%`.
- A seasonal strip, one column per season over the last two years, with the dominant
  cause's letter and share, built from the death events (which outlive the 240-day window).
  Winter starvation against summer predation is the pattern this should make obvious.
- Age at death by cause as a small histogram, so "old age" and "starved young" separate.

**Lineage view: a column on S08.** The S08 "By generation" table (`gen wolves alive sick
mutations members`) gains a `died of` cell per generation: the commonest cause's letter and
share, using the node's `cause`. The focused-creature sidebar gains a `Line` section:
deaths of this lineage over the window by cause, against the species mean, so a bloodline
that dies differently from its species stands out. Rows whose lineage share of one cause
is well above the species share get the cause letter in the `BAD` colour in the tree.
Grouping the whole species by lineage id (gap 2) also enables an S04b sub-table, one row
per lineage still alive: members, founder, dominant cause, and the one trait that differs
most from the species mean, which is the link back to S15.

**Headless.** A `--causes` column set in `summary.csv`: per species, deaths by cause per
year, plus predation by killer species. This is the number the C5 predator balance notes
keep wanting ("do wolves die of hunger or of not finding a mate?" is answerable only with
a cause breakdown per species per year), and it makes the AI post-mortem idea above far
better grounded.

**Determinism and cost.** Reading side only, apart from the two snapshot fields; no RNG,
no events, no change to the checksum. The per-species aggregation is one pass over at
most 50 000 life records, computed on screen entry and cached per day like S15's matrix.
Per-lineage grouping is a `BTreeMap<LineageId, [u32; 6]>` over the same pass.

**Open questions.**

- `Injury` is real but invisible: `behavior/death.rs` assigns it when hp reaches zero with
  no hunger, thirst or parasite cause, yet `kill` emits no event for it (the comment still
  says "no event until C5"). The life log and lineage node do record it, so the 240-day
  views will show it, but the seasonal strip built from events will not. Add a
  `DeathInjury` event, or fold it into the strip from the log, before shipping.
- Window: 240 days matches S15 but hides slow trends. The seasonal strip from the event
  buffer covers longer spans; decide whether the lineage view needs the same.
- Whether a lineage id should follow the mother only (simple, deterministic, one line per
  creature) or record both parents' lines (truer, but a creature then belongs to two
  lineages and every table needs a rule for it). Mother-only is the recommendation.
