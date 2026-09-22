//! Unit tests for the `stats` module, extracted from `stats.rs`.

use super::*;
use crate::sim::species::testing::*;
use crate::sim::creatures::place_founders;
use crate::sim::params::{CreaturesParams, GeneticsParams, WorldParams};
use crate::sim::rng::Rng;
use crate::sim::world::World;

#[test]
fn census_per_species() {
    let w = World::generate(7, &WorldParams::default());
    let params = CreaturesParams::default();
    let mut store = CreatureStore::new();
    for c in place_founders(&w, roster(), &params, &GeneticsParams::default(), 0.20, &mut Rng::new(5)) {
        store.insert(c);
    }
    let c = census(&store,N_SPECIES);
    let _ = &params;
    for id in roster().ids() {
        let i = id.index();
        let want = roster().get(id).initial_count;
        assert_eq!(c.population[i], want, "{id:?}");
        assert_eq!(c.adults[i] + c.juveniles[i], want, "{id:?}");
        if want > 0 {
            for t in 0..Genome::LEN {
                assert!(c.genome_min[i].0[t] <= c.genome_mean[i].0[t]);
                assert!(c.genome_mean[i].0[t] <= c.genome_max[i].0[t]);
            }
        }
    }
}

fn sample(day: u32) -> Sample {
    Sample {
        day,
        biomass_total: 0.0,
        veg_mean: 0.0,
        water_cells: 0,
        water_level: 0.0,
        moisture_mean: 0.0,
        seeds: 0,
        dens: 0,
        carcasses: 0,
        drought_regions: 0,
        drought_flags: [false; 8],
        region_veg: [0.0; 8],
        region_moist: [0.0; 8],
        population: vec![0; 6],
        adults: vec![0; 6],
        juveniles: vec![0; 6],
        deaths_starved: 0,
        deaths_thirst: 0,
        deaths_age: 0,
        genome_mean: vec![Genome([0.0; Genome::LEN]); 6],
        genome_min: vec![Genome([0.0; Genome::LEN]); 6],
        genome_max: vec![Genome([0.0; Genome::LEN]); 6],
        births: vec![0; 6],
        deaths: vec![0; 6],
        generation_mean: vec![0.0; 6],
        generation_max: vec![0; 6],
        infected: vec![0; 6],
        immune: vec![0; 6],
        deaths_disease: 0,
        parasite_mean: vec![0.0; 6],
        active_by_pathogen: [0; 8],
    }
}

#[test]
fn series_ring_buffer() {
    let mut s = Series::new(3);
    assert!(s.is_empty());
    for day in 0..5 {
        s.push(sample(day));
    }
    // Only the last 3 samples remain, oldest first.
    assert_eq!(s.len(), 3);
    assert_eq!(s.day0(), 2);
    let days: Vec<u32> = s.samples().iter().map(|s| s.day).collect();
    assert_eq!(days, vec![2, 3, 4]);
}

#[test]
fn histogram_buckets() {
    assert_eq!(hist_bucket(0.0), 0);
    assert_eq!(hist_bucket(0.02), 0);
    assert_eq!(hist_bucket(1.0 / 12.0), 1);
    assert_eq!(hist_bucket(0.5), 6);
    assert_eq!(hist_bucket(0.98), 11);
    assert_eq!(hist_bucket(1.0), 11);
    let w = World::generate(7, &WorldParams::default());
    let mut store = CreatureStore::new();
    for c in place_founders(&w, roster(), &CreaturesParams::default(), &GeneticsParams::default(), 0.20, &mut Rng::new(5)) {
        store.insert(c);
    }
    let c = census(&store,N_SPECIES);
    for i in 0..6 {
        for t in 0..Genome::LEN {
            let n: u32 = c.hist[i][t].iter().map(|&v| u32::from(v)).sum();
            assert_eq!(n, c.population[i], "histogram {i}/{t} must count every living member");
        }
    }
}

#[test]
fn species_record_incremental_equals_full() {
    // The incremental birth/death counters carried by the species record
    // must equal a full recount from the event log and the daily samples.
    let mut sim = crate::sim::Sim::new(42, crate::sim::Params::default());
    for _ in 0..24 * 120 {
        sim.step();
    }
    // Compare at a day boundary (the record is refreshed at midnight).
    while sim.time.hour() != 0 {
        sim.step();
    }
    for (i, s) in sim.species.iter().enumerate() {
        let id = SpeciesId::from_index(i);
        let full = census(&sim.creatures,N_SPECIES);
        assert_eq!(s.count, full.population[i], "{id:?} count");
        assert_eq!(s.adults, full.adults[i], "{id:?} adults");
        assert_eq!(s.juveniles, full.juveniles[i], "{id:?} juveniles");
        assert_eq!(s.hist, full.hist[i], "{id:?} histogram");
        let births_series: u32 = sim.series.samples().iter().map(|x| x.births[i]).sum();
        let deaths_series: u32 = sim.series.samples().iter().map(|x| x.deaths[i]).sum();
        let births_events: u32 = sim
            .events
            .iter()
            .filter(|e| e.kind == crate::sim::EventKind::Birth && e.species == Some(id))
            .map(|e| e.text.split_whitespace().skip_while(|w| *w != "bore").nth(1).and_then(|w| w.parse::<u32>().ok()).unwrap_or(0))
            .sum();
        assert_eq!(births_series, births_events, "{id:?} births: series vs events");
        let last = sim.series.last().unwrap();
        assert_eq!(s.births_yesterday, last.births[i], "{id:?} births_yesterday");
        assert_eq!(s.deaths_yesterday, last.deaths[i], "{id:?} deaths_yesterday");
        assert!(s.peak >= s.count);
        assert!(s.generation >= full.max_generation[i]);
        let _ = deaths_series;
    }
}

#[test]
fn drift_sample_cadence() {
    let w = World::generate(7, &WorldParams::default());
    let mut store = CreatureStore::new();
    for c in place_founders(&w, roster(), &CreaturesParams::default(), &GeneticsParams::default(), 0.20, &mut Rng::new(5)) {
        store.insert(c);
    }
    let c = census(&store,N_SPECIES);
    let mut stats = SpeciesStats::all(&c, roster(), 0, 2);
    let vole = &stats[0];
    assert_eq!(vole.drift.len(), 1, "the founding census is the first sample");
    assert_eq!(vole.drift[0].0, 1);
    // Generation grows by one: no new sample; by two: a sample.
    let tallies = DeathTallies::new(N_SPECIES);
    let mut c2 = c;
    c2.max_generation[0] = 2;
    update_species_daily(&mut stats, &c2, &tallies, 1, 2);
    assert_eq!(stats[0].drift.len(), 1);
    c2.max_generation[0] = 3;
    update_species_daily(&mut stats, &c2, &tallies, 2, 2);
    assert_eq!(stats[0].drift.len(), 2);
    assert_eq!(stats[0].drift[1].0, 3);
    // Bounded to 12 samples.
    for g in 0..40u32 {
        c2.max_generation[0] = 5 + 2 * g;
        update_species_daily(&mut stats, &c2, &tallies, 3 + g, 2);
    }
    assert_eq!(stats[0].drift.len(), 12);
    assert_eq!(stats[0].trend.len(), 30, "trend keeps the last 30 daily counts");
}

fn lineage_creature(id: u32, gen: u32, parents: Option<(u32, u32)>, alive: bool) -> crate::sim::Creature {
    let w = World::generate(7, &WorldParams::default());
    let mut c = place_founders(&w, roster(), &CreaturesParams::default(), &GeneticsParams::default(), 0.20, &mut Rng::new(1)).remove(0);
    c.id = crate::sim::CreatureId(id);
    c.generation = gen;
    c.parents = parents.map(|(m, f)| (crate::sim::CreatureId(m), crate::sim::CreatureId(f)));
    c.born_day = crate::cast!(gen => i32) * 10;
    c.alive = alive;
    c
}

#[test]
fn lineage_prune_keeps_ancestors() {
    let mut lin = Lineage::new();
    let mut store = CreatureStore::new();
    // g1: 1 (mother), 2 (father) → g2: 3 → g3: 4 (living) ; 5 is a dead g1 with no living descendants.
    for (id, gen, parents, alive) in [(1, 1, None, false), (2, 1, None, false), (5, 1, None, false), (3, 2, Some((1, 2)), false), (4, 3, Some((3, 3)), true)] {
        let c = lineage_creature(id, gen, parents, alive);
        lin.record(&c, roster(), 0.1);
        if !alive {
            lin.record_death(c.id, 50, crate::sim::creatures::Cause::Age, None, 0);
        }
        if alive {
            let mut cc = c.clone();
            cc.id = crate::sim::CreatureId(0);
            let got = store.insert(cc);
            // The store hands out its own ids; make the living creature's id match.
            assert_eq!(got, crate::sim::CreatureId(1));
        }
    }
    // Emulate species max generation 20 with keep 8: cutoff 12 → all g1..g3 dead nodes are candidates.
    let living_store = {
        // Build a store whose only living creature has id 4.
        let mut s = CreatureStore::new();
        for _ in 0..3 {
            let mut filler = lineage_creature(0, 1, None, false);
            filler.alive = false;
            s.insert(filler);
        }
        let alive = lineage_creature(0, 3, Some((3, 3)), true);
        let id = s.insert(alive);
        assert_eq!(id, crate::sim::CreatureId(4));
        s
    };
    let _ = store;
    let removed = lin.prune(&[20; 6], 8, &living_store);
    assert_eq!(removed, 1, "only the dead node with no living descendants is pruned");
    assert!(lin.get(crate::sim::CreatureId(5)).is_none());
    for id in [1, 2, 3, 4] {
        assert!(lin.get(crate::sim::CreatureId(id)).is_some(), "ancestor {id} of a living creature must survive pruning");
    }
}

#[test]
fn lineage_root_depth_and_cap() {
    // A mother chain 1 → 2 → 3 → 4 → 5 (focus) with many siblings per level.
    let mut lin = Lineage::new();
    let mut next = 100u32;
    for id in 1..=5u32 {
        let parents = if id == 1 { None } else { Some((id - 1, id - 1)) };
        lin.record(&lineage_creature(id, id, parents, true), roster(), 0.1);
        if id > 1 {
            for _ in 0..200 {
                lin.record(&lineage_creature(next, id, Some((id - 1, id - 1)), true), roster(), 0.1);
                next += 1;
            }
        }
    }
    let tree = lin.tree(crate::sim::CreatureId(5), 3, 400).unwrap();
    assert_eq!(tree.root, crate::sim::CreatureId(2), "root is 3 generations up the mother line");
    assert!(tree.node_count <= 400, "tree has {} nodes", tree.node_count);
    let ids = tree.node_ids();
    for id in [2, 3, 4, 5] {
        assert!(ids.contains(&crate::sim::CreatureId(id)), "chain node {id} missing");
    }
    assert!(tree.items.iter().any(|it| matches!(it, TreeItem::More { .. })), "truncated branches show `… and N more`");
    // A shallow chain stops early.
    let t2 = lin.tree(crate::sim::CreatureId(2), 3, 400).unwrap();
    assert_eq!(t2.root, crate::sim::CreatureId(1));
}

#[test]
fn daily_sampling_fields() {
    let mut s = Series::new(720);
    let mut x = sample(1);
    x.biomass_total = 12.5;
    x.veg_mean = 0.4;
    x.water_cells = 300;
    x.water_level = 0.5;
    x.drought_regions = 2;
    x.drought_flags[0] = true;
    s.push(x);
    let last = s.last().unwrap();
    assert_eq!(last.day, 1);
    assert_eq!(last.biomass_total, 12.5);
    assert_eq!(last.veg_mean, 0.4);
    assert_eq!(last.water_cells, 300);
    assert_eq!(last.water_level, 0.5);
    assert_eq!(last.drought_regions, 2);
    assert!(last.drought_flags[0]);
    assert!(!last.drought_flags[1]);
}

#[test]
fn local_maxima_rule() {
    // A flat series has no local maxima (nothing rises 15 % above the mean).
    let flat = vec![1.0f32; 200];
    assert!(local_maxima(&flat).is_empty(), "a flat series has no local maxima");

    // Two broad peaks well separated (> 90 days apart): both are detected.
    let mut v = vec![0.5f32; 600];
    for i in 100..=140 {
        v[i] = 2.0;
    }
    for i in 400..=440 {
        v[i] = 2.5;
    }
    let maxima = local_maxima(&v);
    assert_eq!(maxima, vec![100, 400], "lowest index on each plateau: {maxima:?}");
}

#[test]
fn peak_lag_on_synthetic_series() {
    // A predator series that is the prey series delayed by 20 days must yield lag 20.
    let n = 800usize;
    let mut prey = vec![0.0f32; n];
    for i in 0..n {
        prey[i] = ((crate::cast!(i => f32) / 60.0).sin() + 1.0) * 100.0;
    }
    let mut pred = vec![0.0f32; n];
    pred[20..n].copy_from_slice(&prey[..n - 20]);
    assert_eq!(peak_lag(&prey, &pred), Some(20));

    // Fewer than two local maxima → None.
    let flat = vec![50.0f32; n];
    assert_eq!(peak_lag(&flat, &flat), None);
}

fn root(lin: &Lineage, id: u32) -> Option<crate::sim::CreatureId> {
    lin.get(crate::sim::CreatureId(id)).map(|n| n.root)
}

#[test]
fn lineage_root_follows_the_mother_line_and_survives_pruning() {
    let mut lin = Lineage::new();
    // Founders 1 and 2; 3 is their child, 4 is 3's child by 2; 6 has an unknown mother.
    for (id, gen, parents, alive) in [(1, 1, None, false), (2, 1, None, false), (3, 2, Some((1, 2)), false), (4, 3, Some((3, 2)), true), (6, 3, Some((9, 2)), true)] {
        let c = lineage_creature(id, gen, parents, alive);
        lin.record(&c, roster(), 0.1);
        if !alive {
            lin.record_death(c.id, 50, crate::sim::creatures::Cause::Age, None, 0);
        }
    }
    assert_eq!(root(&lin, 1), Some(crate::sim::CreatureId(1)), "a founder is its own root");
    assert_eq!(root(&lin, 3), Some(crate::sim::CreatureId(1)), "a child takes its mother's root");
    assert_eq!(root(&lin, 4), Some(crate::sim::CreatureId(1)), "and so does a grandchild");
    assert_eq!(root(&lin, 6), Some(crate::sim::CreatureId(6)), "an orphan founds a new line");
    // Prune everything dead: the living nodes keep the root id even when the chain is gone.
    let store = CreatureStore::new();
    lin.prune(&[20; 6], 0, &store);
    assert_eq!(root(&lin, 4), Some(crate::sim::CreatureId(1)));
    assert_eq!(root(&lin, 1), None, "the founder's node is gone");
}
