//! Names, chronicle and summary on the default worlds, and the generator
//! on its own.

use std::collections::BTreeSet;

use super::lexicon::{self, NAME_FULL_MAX, STEM_MAX, STEM_MIN, STYLE, STYLES};
use super::*;
use crate::sim::params::WorldParams;
use crate::sim::rng::Rng;
use crate::sim::world::{chronicle, classify, relief, summary, CHRONICLE_LINES, CHRONICLE_WIDTH};

const SEEDS: std::ops::RangeInclusive<u64> = 1..=20;

fn world(seed: u64) -> World {
    World::generate(seed, &WorldParams::default())
}

#[test]
fn names_are_stable_per_seed() {
    let (a, b) = (world(7), world(7));
    assert_eq!(a.names, b.names);
    let c = world(8);
    let names = |w: &World| w.names.features.iter().map(|f| f.name.clone()).collect::<Vec<_>>();
    assert_ne!(names(&a), names(&c), "two seeds share every name");
}

#[test]
fn every_trunk_and_big_lake_is_named() {
    let params = WorldParams::default();
    for seed in SEEDS {
        let w = world(seed);
        let grid = Grid { w: params.width, h: params.height };
        let relief = relief::build(&mut Rng::new(seed), grid, &params);
        let bodies = classify::water_bodies(grid, &relief, &params);
        for i in 0..grid.len() {
            let (x, y) = (i % grid.w, i.div_euclid(grid.w));
            if bodies.channel[i] == Channel::Course(Tier::Trunk) {
                let f = w.feature_at(x, y).unwrap_or_else(|| panic!("seed {seed}: trunk cell ({x}, {y}) has no name"));
                assert!(f.kind.is_river(), "seed {seed}: trunk cell ({x}, {y}) is {:?}", f.kind);
            }
            if bodies.water[i] == Water::Ocean {
                assert_eq!(w.feature_at(x, y).map(|f| f.kind), Some(FeatureKind::Ocean), "seed {seed}: ocean cell ({x}, {y})");
            }
        }
        let lakes = components(grid, |i| bodies.water[i] == Water::Lake);
        for members in lakes.iter().filter(|p| p.len() >= LAKE_MIN_CELLS).take(LAKE_CAP) {
            let ids: BTreeSet<u16> = members.iter().map(|&i| w.names.map[i]).collect();
            assert_eq!(ids.len(), 1, "seed {seed}: a lake of {} cells maps to {ids:?}", members.len());
            let f = &w.names.features[usize::from(*ids.iter().next().unwrap())];
            assert_eq!(f.kind, FeatureKind::Lake);
        }
        assert!(w.features_of_kind(FeatureKind::River).next().is_some(), "seed {seed}: no river stem");
    }
}

#[test]
fn names_fit_their_caps() {
    for seed in SEEDS {
        let w = world(seed);
        let n = &w.names;
        assert_eq!(n.map.len(), w.cells.len());
        let mut seen = BTreeSet::new();
        for (k, f) in n.features.iter().enumerate() {
            assert!(f.name.is_ascii() && f.name.chars().count() <= NAME_FULL_MAX, "seed {seed}: {:?}", f.name);
            assert!(seen.insert(f.name.clone()), "seed {seed}: duplicate {}", f.name);
            check_feature(&w, k, f, seed);
        }
        assert!(n.features.iter().filter(|f| f.kind.is_river()).count() <= RIVER_CAP);
        assert!(w.features_of_kind(FeatureKind::Lake).count() <= LAKE_CAP);
        assert!(w.features_of_kind(FeatureKind::Range).count() <= RANGE_CAP);
        assert!(w.features_of_kind(FeatureKind::Ocean).count() <= 1);
        check_order(&w, seed);
    }
}

/// The map agrees with the feature, its anchor is its own, and a
/// tributary's parent is a river.
fn check_feature(w: &World, k: usize, f: &Feature, seed: u64) {
    let cells = crate::cast!(f.cells => usize);
    let mapped = w.names.map.iter().filter(|&&m| usize::from(m) == k).count();
    if f.kind.is_river() {
        assert!(mapped >= cells, "seed {seed}: {} maps {mapped} < {cells}", f.name);
        assert!(cells >= STEM_MIN_CELLS || f.kind == FeatureKind::River, "seed {seed}: short {}", f.name);
    } else {
        assert_eq!(mapped, cells, "seed {seed}: {}", f.name);
    }
    assert_eq!(w.feature_at(f.anchor.0, f.anchor.1).map(|g| g.name.as_str()), Some(f.name.as_str()), "seed {seed}: anchor of {}", f.name);
    if let Some(p) = f.parent {
        assert_eq!(f.kind, FeatureKind::Tributary);
        assert_eq!(w.names.features[usize::from(p)].kind, FeatureKind::River);
    }
}

/// The list is ordered by kind, and by size within lakes and ranges.
fn check_order(w: &World, seed: u64) {
    let kinds: Vec<FeatureKind> = w.names.features.iter().map(|f| f.kind).collect();
    let mut sorted = kinds.clone();
    sorted.sort();
    assert_eq!(kinds, sorted, "seed {seed}");
    for k in [FeatureKind::Lake, FeatureKind::Range] {
        let sizes: Vec<u32> = w.features_of_kind(k).map(|f| f.cells).collect();
        assert!(sizes.windows(2).all(|p| p[0] >= p[1]), "seed {seed}: {k:?} {sizes:?}");
    }
}

#[test]
fn chronicle_fits_the_layout() {
    let age = WorldParams::default().age;
    for seed in SEEDS {
        let w = world(seed);
        let lines = chronicle(&w, age);
        assert!(!lines.is_empty() && lines.len() <= CHRONICLE_LINES, "seed {seed}: {} lines", lines.len());
        for l in &lines {
            assert!(l.chars().count() <= CHRONICLE_WIDTH, "seed {seed}: {l:?} is {} wide", l.chars().count());
            assert!(l.ends_with('.'), "seed {seed}: {l:?}");
        }
        assert!(lines[0].to_lowercase().contains(w.wind.name()), "seed {seed}: {:?}", lines[0]);
        assert!(lines.len() > w.history.len(), "seed {seed}: no line after the events");
        assert!(lines.iter().any(|l| l.contains(" runs from ") || l.contains(" winds through ")), "seed {seed}: no river line");
    }
}

#[test]
fn summary_is_consistent() {
    for seed in SEEDS {
        let w = world(seed);
        let s = summary(&w);
        assert_eq!(s.coast > 0, s.has_ocean, "seed {seed}");
        let top = w.cells.iter().map(|c| c.elevation).fold(f32::MIN, f32::max);
        assert!((s.peak.1 - top).abs() < f32::EPSILON, "seed {seed}");
        let longest = w.names.features.iter().filter(|f| f.kind.is_river()).map(|f| f.cells).max();
        assert_eq!(s.longest_river.as_ref().map(|r| r.1), longest, "seed {seed}");
        assert_eq!(s.rivers, w.names.features.iter().filter(|f| f.kind.is_river()).count());
    }
}

#[test]
fn tables_are_ascii_lowercase() {
    for st in &STYLE {
        for part in [st.onsets, st.nuclei, st.codas] {
            assert!(part.iter().all(|s| s.chars().all(|c| c.is_ascii_lowercase())), "{part:?}");
        }
        assert!(st.onsets.iter().take(12).all(|o| o.len() <= 1), "simple onsets must lead: {:?}", st.onsets);
        assert!(st.nuclei.iter().all(|n| !n.is_empty()));
    }
}

#[test]
fn stems_are_bounded_capitalised_and_unique() {
    for style in 0..STYLES {
        let mut rng = Rng::new(0x5EED + crate::cast!(style => u64));
        let mut used = BTreeSet::new();
        for _ in 0..1000 {
            let s = lexicon::stem(&mut rng, style, &mut used);
            assert!((STEM_MIN..=STEM_MAX).contains(&s.len()), "{s:?}");
            assert!(s.chars().next().is_some_and(|c| c.is_ascii_uppercase()), "{s:?}");
            assert!(s.chars().skip(1).all(|c| c.is_ascii_lowercase()), "{s:?}");
        }
        assert_eq!(used.len(), 1000);
    }
}

#[test]
fn every_template_fits() {
    let mut rng = Rng::new(1);
    for kind in [FeatureKind::Ocean, FeatureKind::Lake, FeatureKind::River, FeatureKind::Tributary, FeatureKind::Range] {
        for noun in ["Mountains", "Hills", "Fells"] {
            for _ in 0..50 {
                let mut used = BTreeSet::new();
                let name = lexicon::full_name(&mut rng, 0, &mut used, kind, noun);
                assert!(name.chars().count() <= NAME_FULL_MAX, "{name:?}");
            }
        }
    }
}

#[test]
fn place_names_prefer_water_beside_a_cell() {
    let w = world(3);
    let lake = w.features_of_kind(FeatureKind::Lake).next().expect("a lake");
    let (x, y) = lake.anchor;
    assert_eq!(w.place_name(x, y), format!("by {}", lake.name));
    assert_eq!(w.feature_near(x, y).map(|f| &f.name), Some(&lake.name));
    // A cell far from every feature falls back to its region.
    let far = (0..w.cells.len()).find(|&i| w.feature_near(i % w.width, i.div_euclid(w.width)).is_none()).expect("an unnamed cell");
    let (fx, fy) = (far % w.width, far.div_euclid(w.width));
    assert_eq!(w.place_name(fx, fy), format!("in {}", w.region_name(fx, fy)));
}

#[test]
#[ignore = "diagnostic: prints the style, every name, the chronicle and the summary per seed, and the naming time at 200x60"]
fn print_names() {
    let params = WorldParams::default();
    for seed in SEEDS {
        let w = world(seed);
        println!("seed {seed}: style {}", w.names.style);
        for f in &w.names.features {
            println!("  {:<10} {:<24} {:>4} cells  anchor {:?}", f.kind.label(), f.name, f.cells, f.anchor);
        }
        for l in chronicle(&w, params.age) {
            println!("  | {l}");
        }
        println!("  {:?}", summary(&w));
    }
    let big = WorldParams { width: 200, height: 60, ..WorldParams::default() };
    let grid = Grid { w: big.width, h: big.height };
    let relief = relief::build(&mut Rng::new(42), grid, &big);
    let (cells, _, bodies) = classify::cells(&mut Rng::new(42), grid, &relief, &big);
    let started = std::time::Instant::now();
    let names = build(42, &Source { grid, relief: &relief, bodies: &bodies, cells: &cells });
    println!("200x60 naming: {} features in {:?}", names.features.len(), started.elapsed());
}
