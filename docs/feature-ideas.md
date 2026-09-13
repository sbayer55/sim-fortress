The roadmap's six chunks are all shipped, so the next feature is genuinely open. Here are the candidates I find most interesting, grouped by what they add, with the one I'd pick first at the top.

**My pick: burrows and dens (the vole refuge).** The predator chunk's own status note says the population bands are unreachable by tuning alone and names the missing mechanism: voles have no refuge, and wolves and lynxes die without meeting a mate. A burrow cell is a home tile that a vole retreats to when fleeing, is undetectable inside, and rests in at night. A den is the predator version: a pack home that gives wolves and lynxes a mate-seeking target and a place to raise a litter. This is one mechanic that fixes two open balance problems, adds a visible new glyph on the map, and gives the inspector a "home" field to follow. It builds directly on the existing Flee, Rest and Mate goals.

**Other simulation-side ideas, roughly in order of payoff:**

- **Carcasses and scavenging as real objects.** Scavenge is already a goal, but a kill could leave a decaying carcass tile that foxes and voles compete over, and that fertilises the vegetation beneath it. Cheap to add, and it produces the kind of emergent scene players screenshot.
- **Disease or parasites.** A contagious status that spreads by proximity, gated by a new genome slot for resistance. It gives evolution a second selective pressure besides predation and would make the lineage tree far more dramatic after an epidemic event.
- **Weather beyond drought.** Snow cover that hides grass in winter, floods that push rivers outward, wildfire that burns forest into sparse grass and regrows. Fire in particular is visually striking in CP437 and interacts with every layer already there.
- **Speciation.** When two lineages of the same species drift far enough apart in genome distance, they stop interbreeding and the browser shows a split. The lineage screen and genome already exist, so this is mostly a distance check plus a naming scheme.
- **Herd and pack behaviour.** Deer that cluster and flee together, wolves that hunt in groups with a shared target. This also addresses the wolf mate-finding problem from a different angle than dens.
- **Territory and scent marking.** Predators leave a decaying scent field that repels rivals and attracts mates. It naturally spaces predators out and gives you a new map overlay.

**Player-facing ideas, if you want a break from ecology:**

- **The player's hand.** A small set of interventions: place a water hole, plant a forest, cull or introduce a founder group, start a fire. Turns pure observation into experimentation, and the headless runner could take the same commands from a script.
- **Chronicle screen.** An auto-written history of the world: "Year 3, the voles of the eastern meadow vanished; the foxes followed by winter." Generated from the event ring buffer, in the Dwarf Fortress legends style.
- **Named notable creatures.** The oldest, the most prolific, the best hunter, automatically named and tracked, with an obituary event on death.
- **Replay and time scrubbing.** Save periodic snapshots and let the charts screen jump the map back to any year.

**One infrastructure option:** you have a determinism rule and a headless mode, so a **scenario file** format (seed, params, scripted interventions per tick, assertions on counts) would let you regression-test balance claims like the C5 bands instead of re-running sweeps by hand.

If you want, I can write the burrows-and-dens idea up as a seventh chunk doc in the same format as the existing six, with requirements, acceptance bands and a checkpoint demo script.


---

The current genome has eleven slots: speed, size, sense, metabolism, aggression, camouflage, fertility, longevity, resistance (C7), sociality and maturity (C8). The most interesting additions are ones that trade off against an existing slot or against a specific mechanic, so selection can pull them in different directions in different worlds.

**Traits that exploit the existing environment**

- **Cold tolerance.** Lower winter metabolism cost, but a higher summer cost. Harsh-winter presets would select it up, lush ones down, so the same species diverges across presets.
- **Drought tolerance.** Thirst rises more slowly, at the price of a slower speed cap. It would make your dry-world migration events much less lethal for some lineages and not others.
- **Diet breadth.** For herbivores, the terrain types the creature can graze on, from dense-grass specialist to forest browser. Specialists eat faster, generalists survive dieback. This also spreads deer and hares across terrain instead of stacking on the best cells.
- **Nocturnality.** A phase shift on the sleep cycle. A nocturnal vole avoids a diurnal fox entirely, which is a refuge that needs no new tiles, and a nocturnal fox lineage would then be selected to chase them. Day and night tint already exists, so it is visible.

**Traits that shape predator and prey encounters**

- **Boldness.** The distance at which prey starts to flee. Timid animals lose grazing time, bold ones get eaten. Classic and very legible in the sense-ring overlay.
- **Vigilance versus foraging.** Fraction of a graze tick spent scanning. It directly trades hunger against detection, and it is the natural counterpart to camouflage.
- **Stamina.** Chase duration before speed collapses. Lets a slow predator with high stamina beat a fast prey, breaking the current speed arms race into two axes.
- ~~**Sociality.** Preferred group size. High values cluster into herds that detect predators earlier but strip vegetation; for predators it is the seed of pack hunting.~~ **Shipped: genome slot 10 — herds cohere and bias grazing, a sighted prey alarms social kin, and predators join a packmate's target and share the kill.**
- **Prey preference.** For predators, a bias toward small or large prey. A fox lineage that stops bothering voles is another route to the vole refuge your predator notes ask for.

**Life-history traits**

- ~~**Maturity age.** Breed early with small litters or late with large ones, paired against longevity. This gives r-versus-K strategy shifts you could see in the species browser.~~ **Shipped: genome slot 11 — one trait scales adult age, litter size and max lifespan, and S04b names the r/K direction in Selection pressure.**
- **Parental care.** Offspring start with more reserves and the parent loses hunger for a period after birth. Litters get smaller but survive. Nice interaction with any future dens.
- **Dispersal.** How far a newborn wanders from its parent before settling. Low values create local inbred pockets, high values spread genes and spark migrations. Very visible on the lineage tree.
- **Mutation rate itself.** Evolvability as a trait. Stable worlds select it down, worlds with droughts and epidemics select it up. Cheap to add and a genuinely interesting experiment.

**Traits that need one new mechanic**

- **Disease resistance,** if you add contagion. A second selective pressure independent of predation.
- **Toxicity or spines,** a cost to the predator on a kill. Gives prey a defence that is not running or hiding.
- **Coat colour** as a numeric value matched against the terrain palette under the creature. Camouflage becomes terrain specific, so a forest vole and a meadow vole drift apart, and you could tint the glyph with the value.

If you want the most result for the least code, I'd take nocturnality, boldness and mutation rate first. All three slot into behaviour code that already exists, and each one can be watched drifting on the charts screen without any new UI.