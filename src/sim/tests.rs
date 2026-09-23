//! `Sim` tests: the determinism checksum lock, the FNV vectors, the save
//! round trip and the sim-purity scan (no `ratatui`, `HashMap` or `HashSet`
//! under `src/sim`). Split out of `mod.rs` for the 800-line ceiling; pure
//! code motion.

use super::*;
use crate::sim::species::testing::{FOX, HARE, VOLE};

#[test]
fn fnv1a64_vectors() {
    assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
}

#[test]
fn determinism_10k_ticks() {
    let mut a = Sim::new(42, Params::default());
    let mut b = Sim::new(42, Params::default());
    for _ in 0..10_000 {
        a.step();
        b.step();
    }
    assert_eq!(a.checksum(), b.checksum());
}

#[test]
fn checksum_is_fnv_stable() {
    let mut a = Sim::new(7, Params::default());
    for _ in 0..8640 {
        a.step();
    }
    let mut b = Sim::new(7, Params::default());
    for _ in 0..8640 {
        b.step();
    }
    assert_eq!(a.checksum(), b.checksum());
    // Lock the exact value so accidental algorithm changes fail loudly.
    // Re-baselined for C8: the genome grew to eleven traits (Sociality and
    // Maturity), which shifts the founder jitter and every downstream draw.
    // Re-baselined for C5 FR5b: prey now spend wary ticks steering away from
    // predators that are not hunting them, so every trajectory downstream of
    // the first such encounter moves. Deliberate, not an accident.
    // Re-baselined for the erosion-based world generator: every cell's
    // terrain, elevation and moisture changed, so founders land elsewhere.
    // Re-baselined for climate physics: the wind and pole draws precede
    // the tectonic noise and orographic rain replaces the free rain
    // field, so every cell's moisture and terrain moved again.
    // Re-baselined for wetlands: marsh and riparian corridors recut the
    // land and the initial vegetation reads moisture and temperature.
    // Re-baselined for biomes as regions: the forest quantile skips the
    // treeless biomes, vegetation scales by biome, and regions are
    // drainage basins, so rain, drought and migration act on new areas.
    // Re-baselined for river morphology: rivers widen by tier, trunks
    // run deep, deltas fan, falls keep their rock and arid brooks start
    // as dry washes, so the water cells and everything near them moved.
    // Re-baselined for Mutability: the genome grew to twelve traits, so
    // every founder draws one more jitter gaussian and a sterility roll,
    // and every birth draws a sterility roll after inheritance.
    // Re-baselined for history: the event draws precede the epochs, the
    // events reshape the relief, the vegetation warm-up replaces the
    // seeded biomass founders land on, and regions are split into
    // 4-connected pieces.
    // Re-baselined for Diet breadth: the genome grew to thirteen traits,
    // so every founder draws one more jitter gaussian, and grazing now
    // depends on terrain, so every later draw shifts.
    // Re-baselined for territory (C5 FR13): solitary predators discount
    // patrol and hunt targets on a rival's scent and roll contests on the
    // creature stream, so predator trajectories and every later draw move.
    // `behavior::tests_territory::neutral_territory_reproduces_the_old_checksum`
    // still pins the previous value under the neutral overlay.
    // Re-baselined for succession and trampling (C2 FR12): prey traffic
    // scales the vegetation target, so every grazed cell's biomass moves
    // from the first day, and ripe cells roll a flip on the ecology stream,
    // so rain and regrowth draws shift. Deliberate. (Re-baselined once more
    // when `trample_w` went from the planned 0.5 to 0.25 on the sweep's evidence.)
    // `ecology::tests::neutral_succession_reproduces_the_old_checksum`
    // pins the previous value under the succession-neutral overlay.
    assert_eq!(a.checksum(), 0x10cd_7594_03e3_d96c);
}

#[test]
fn checksum_includes_creatures() {
    let a = Sim::new(42, Params::default());
    let mut b = Sim::new(42, Params::default());
    let id = b.creatures.living_ids()[0];
    b.creatures.get_mut(id).unwrap().x += 1;
    assert_ne!(a.checksum(), b.checksum(), "checksum must reflect creature state");
}

/// A sim with only the given species (and `n` founders), all killed on the
/// first tick by zeroing hp, then stepped to the next midnight.
fn extinction_sim(species: SpeciesId, n: u32) -> Sim {
    let mut p = Params::default();
    p.species.clear_initial_counts();
    p.species.get_mut(species).initial_count = n;
    let mut sim = Sim::new(7, p);
    for id in sim.creatures.living_ids() {
        let c = sim.creatures.get_mut(id).unwrap();
        c.hp = 0.0;
        c.hunger = 1.0; // so `needs` drives hp below zero instead of regenerating
        c.thirst = 0.0;
    }
    sim
}

/// Step until the first midnight (hour 0) and collect every alert.
fn step_to_midnight(sim: &mut Sim) -> Vec<Alert> {
    let mut alerts = Vec::new();
    loop {
        let r = sim.step();
        alerts.extend(r.alerts);
        if sim.time.hour() == 0 {
            return alerts;
        }
    }
}

#[test]
fn extinction_once_and_not_for_absent_species() {
    let mut sim = extinction_sim(HARE, 3);
    let alerts = step_to_midnight(&mut sim);
    assert!(alerts.iter().any(|a| matches!(a, Alert::Extinction { species: HARE, .. })), "hare extinction alert: {alerts:?}");
    // Fox has initial_count == 0 and must never emit.
    assert!(!alerts.iter().any(|a| matches!(a, Alert::Extinction { species: FOX, .. })));
    // A species emits at most once.
    let mut more = Vec::new();
    for _ in 0..200 {
        more.extend(sim.step().alerts);
    }
    let hare = alerts.into_iter().chain(more).filter(|a| matches!(a, Alert::Extinction { species: HARE, .. })).count();
    assert_eq!(hare, 1, "a species must emit at most once");
}

#[test]
fn alert_queue_two_species_same_day() {
    let mut p = Params::default();
    p.species.clear_initial_counts();
    p.species.set_initial_count("vole", 2);
    p.species.set_initial_count("hare", 2);
    let mut sim = Sim::new(7, p);
    for id in sim.creatures.living_ids() {
        let c = sim.creatures.get_mut(id).unwrap();
        c.hp = 0.0;
        c.hunger = 1.0;
        c.thirst = 0.0;
    }
    let alerts = step_to_midnight(&mut sim);
    let species: Vec<SpeciesId> = alerts
        .iter()
        .filter_map(|a| match a {
            Alert::Extinction { species, .. } => Some(*species),
            Alert::Epidemic { .. } => None,
        })
        .collect();
    assert_eq!(species, vec![VOLE, HARE], "species-table order on the same day");
}

#[test]
fn no_ratatui_in_sim() {
    for path in sim_sources() {
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("ratatui"), "{} must not reference ratatui", path.display());
    }
}

#[test]
fn no_hashmap_in_sim() {
    for path in sim_sources() {
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("HashMap"), "{} uses HashMap", path.display());
        assert!(!text.contains("HashSet"), "{} uses HashSet", path.display());
    }
}

fn sim_sources() -> Vec<std::path::PathBuf> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sim");
    let mut out = Vec::new();
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            // Skip this file (the harness itself names "ratatui"/"HashMap")
            // and `mod.rs`, whose module doc states the rule.
            let this_file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(file!());
            if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && path != this_file
                && path.file_name().and_then(|f| f.to_str()) != Some("mod.rs")
            {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}
