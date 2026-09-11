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