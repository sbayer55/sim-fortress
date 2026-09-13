//! Unit tests for the `genetics` module, extracted from `genetics.rs`.

use super::*;
use crate::sim::creatures::max_age_days;
use crate::sim::params::{Params, WorldParams};
use crate::sim::species::{IDX_MATURITY, N_TRAITS};
use crate::sim::world::Terrain;
use crate::sim::Sim;

fn world() -> World {
    let mut w = World::generate(7, &WorldParams { width: 40, height: 10, ..WorldParams::default() });
    for c in &mut w.cells {
        c.terrain = Terrain::Grass;
        c.vegetation = 0.6;
    }
    w
}

fn time_at(tick: u64) -> Time {
    let mut t = Time::new(6, 90, 24, 6, 20);
    t.tick = tick;
    t
}

/// The doc's FR1 starting values (independent of the tuned balance table).
fn gp() -> GeneticsParams {
    let counts = |vals: [u32; 6]| -> std::collections::BTreeMap<SpeciesId, u32> { SpeciesId::ALL.iter().copied().zip(vals).collect() };
    GeneticsParams {
        litter_max: SpeciesId::ALL.iter().copied().zip([3.0, 2.0, 1.0, 3.0, 2.0, 1.0]).collect(),
        mate_cooldown_days: counts([20, 30, 150, 120, 180, 180]),
        mate_hunger_max: 0.3,
        mate_cell_vegetation_min: 0.3,
        ..GeneticsParams::default()
    }
}

fn adult(x: usize, y: usize, sex: Sex) -> Creature {
    Creature {
        id: CreatureId(0),
        species: SpeciesId::Vole,
        name: 0,
        sex,
        x,
        y,
        born_day: -100,
        generation: 1,
        parents: None,
        genome: SpeciesId::Vole.base_genome(),
        hp: 1.0,
        hunger: 0.1,
        thirst: 0.1,
        energy: 0.9,
        adult: true,
        goal: Goal::Wander,
        target: None,
        replan_at: 0,
        trail: Vec::new(),
        alive: true,
        death: None,
        decay: 0.0,
        mutations: Vec::new(),
        last_water: None,
        move_budget: 0.0,
        path: Vec::new(),
        rest_reason: None,
        pregnant_due: None,
        cooldown_until: 0,
        mate_id: None,
        mother: None,
        offspring: 0,
        kills: 0,
        attempts: 0,
        chased: 0,
        escaped: 0,
        threats_by_species: [0; 6],
        kills_by_species: [0; 6],
        last_kill: None,
        chase_stats: (0, 0),
        chase_longest_year: 0,
        hunt_phase: HuntPhase::Stalk,
        hunt_target: None,
        chase_start_tick: None,
        hunt_cooldown_until: 0,
        eat_until: None,
        scavenge_target: None,
        flee_until: 0,
        threatened_by: None,
        predation_risk: 0.0,
        kin_nearby: 0,
        migrate_until: 0,
            // ---- C7 disease / parasites
            infection: None,
            immune_until: [0; 8],
            parasite_load: 0.0,
            infections_survived: 0,
            died_infected: None,
        migrate_target: None,
        path_for: None,
    }
}

#[test]
fn mate_eligibility() {
    let w = world();
    let gp = gp();
    let t = time_at(12);
    let mut c = adult(5, 5, Sex::Female);
    assert!(eligible(&c, &t, &w, &gp, &DiseaseParams::default()));
    c.adult = false;
    assert!(!eligible(&c, &t, &w, &gp, &DiseaseParams::default()), "juveniles never mate");
    c.adult = true;
    c.hunger = 0.5;
    assert!(!eligible(&c, &t, &w, &gp, &DiseaseParams::default()), "too hungry");
    c.hunger = 0.1;
    c.cooldown_until = 100;
    assert!(!eligible(&c, &t, &w, &gp, &DiseaseParams::default()), "on cooldown");
    c.cooldown_until = 0;
    // Winter: day index 270+ → tick 270*24.
    assert!(!eligible(&c, &time_at(270 * 24), &w, &gp, &DiseaseParams::default()), "not a breeding season");
    let mut bare = world();
    bare.cell_mut(5, 5).vegetation = 0.0;
    assert!(!eligible(&c, &t, &bare, &gp, &DiseaseParams::default()), "prey need vegetation on the cell");
}

#[test]
fn mating_sets_cooldown_and_pregnancy() {
    let w = world();
    let gp = gp();
    let t = time_at(12);
    let mut store = CreatureStore::new();
    let f = store.insert(adult(5, 5, Sex::Female));
    let m = store.insert(adult(6, 5, Sex::Male));
    let view = TickView::build(&store, &t, &w, &gp, &DiseaseParams::default());
    assert!(view.get(f).unwrap().mate_ready);
    let picked = pick_mate(store.get(f).unwrap(), &[m], &view).unwrap();
    assert_eq!(picked.0, m);
    {
        let c = store.get_mut(f).unwrap();
        c.goal = Goal::Mate;
        c.mate_id = Some(m);
    }
    let mut events = EventRing::new(10);
    let mut noted = false;
    consummate(&mut store, &t, &gp, &mut events, &view, &mut noted);
    let female = store.get(f).unwrap();
    let male = store.get(m).unwrap();
    let cd = 12 + u64::from(gp.cooldown(SpeciesId::Vole)) * 24;
    assert_eq!(female.cooldown_until, cd);
    assert_eq!(male.cooldown_until, cd);
    assert_eq!(female.pregnant_due, Some(12 + u64::from(gp.gestation(SpeciesId::Vole)) * 24));
    assert_eq!(female.mate_id, Some(m));
    assert!(male.pregnant_due.is_none());
    assert!(male.mate_id.is_none());
}

#[test]
fn maturity_scales_adult_age_litter_and_lifespan() {
    let gp = GeneticsParams::default();
    let cp = CreaturesParams::default();
    let id = SpeciesId::Deer;
    let neutral = id.base_genome(); // maturity 0.5 by construction
    let mut slow = id.base_genome();
    slow.0[IDX_MATURITY] = 0.98;
    let mut fast = id.base_genome();
    fast.0[IDX_MATURITY] = 0.02;

    // Maturity 0.5 reproduces the pre-maturity numbers exactly.
    assert_eq!(adult_age_days(id, &neutral, &cp, &gp), cp.adult_age(id));
    let old_max = cp.max_age_base + crate::cast!((neutral.longevity() * crate::cast!(cp.max_age_per_longevity => f32)) => u32);
    assert_eq!(max_age_days(&neutral, &cp, &gp), old_max);
    assert_eq!(gp.litter_size(id, neutral.fertility(), 0.5), 1 + crate::cast!((neutral.fertility() * gp.litter_max(id)).round() => u32));

    // Slow: later, larger, longer. Fast: the reverse.
    assert!(adult_age_days(id, &slow, &cp, &gp) > adult_age_days(id, &neutral, &cp, &gp));
    assert!(adult_age_days(id, &neutral, &cp, &gp) > adult_age_days(id, &fast, &cp, &gp));
    assert!(gp.litter_size(id, slow.fertility(), slow.maturity()) > gp.litter_size(id, fast.fertility(), fast.maturity()));
    assert!(max_age_days(&slow, &cp, &gp) > old_max);
    assert!(max_age_days(&fast, &cp, &gp) < old_max);
}

#[test]
fn inherit_covers_every_slot() {
    // Every trait index, including the two C8 additions, is inherited and can
    // mutate (mutation_rate 1 makes the draw deterministic).
    let gp = GeneticsParams { mutation_rate: 1.0, mutation_strength: 0.0, ..GeneticsParams::default() };
    let g = SpeciesId::Wolf.base_genome();
    let (out, muts) = inherit(&g, &g, 4, &gp, &mut Rng::new(3));
    assert_eq!(muts.len(), N_TRAITS, "one mutation per slot");
    let mut idx: Vec<usize> = muts.iter().map(|m| m.trait_idx).collect();
    idx.sort_unstable();
    assert_eq!(idx, (0..N_TRAITS).collect::<Vec<usize>>());
    assert_eq!(out.sociality(), g.sociality());
    assert_eq!(out.maturity(), g.maturity());
}

#[test]
fn litter_size_from_fertility() {
    let gp = gp();
    assert_eq!(gp.litter_size(SpeciesId::Vole, 0.0, 0.5), 1);
    assert_eq!(gp.litter_size(SpeciesId::Vole, 0.9, 0.5), 4);
    assert_eq!(gp.litter_size(SpeciesId::Deer, 0.35, 0.5), 1);
    assert_eq!(gp.litter_size(SpeciesId::Hare, 0.75, 0.5), 3);
}

/// Shared checks for each newborn in `birth_placement`.
fn assert_pup(c: &Creature, f: CreatureId, m: CreatureId, newborn_hp: f32) {
    assert!(matches!((c.x, c.y), (5 | 6, 5)), "pup at {:?}", (c.x, c.y));
    assert_eq!(c.parents, Some((f, m)));
    assert_eq!(c.mother, Some(f));
    assert_eq!(c.generation, 2);
    assert_eq!(c.hp, newborn_hp);
    assert!(!c.adult);
}

#[test]
fn birth_placement() {
    let mut w = world();
    // Wall off everything but the mother's cell and one neighbour.
    for c in &mut w.cells {
        c.terrain = Terrain::Rock;
    }
    w.cell_mut(5, 5).terrain = Terrain::Grass;
    w.cell_mut(6, 5).terrain = Terrain::Grass;
    let gp = gp();
    let cp = CreaturesParams::default();
    let mut store = CreatureStore::new();
    let mut mother = adult(5, 5, Sex::Female);
    mother.genome.0[6] = 0.9; // litter 1 + round(0.9×3) = 4
    let f = store.insert(mother);
    let m = store.insert(adult(6, 5, Sex::Male));
    store.get_mut(f).unwrap().pregnant_due = Some(10);
    store.get_mut(f).unwrap().mate_id = Some(m);
    let mut events = EventRing::new(10);
    let mut tallies = DeathTallies::default();
    let mut lineage = Lineage::new();
    let born = deliver(&mut store, &w, &mut events, &time_at(10), &gp, &cp, &DiseaseParams::default(), &mut Rng::new(3), &mut tallies, &mut lineage, &DiseaseState::new(&DiseaseParams::default()), &mut Rng::new(4));
    assert_eq!(born, 4);
    assert_eq!(store.len_living(), 6);
    for c in store.living().filter(|c| c.parents.is_some()) {
        assert_pup(c, f, m, gp.newborn_hp);
    }
    assert_eq!(store.get(f).unwrap().offspring, 4);
    assert_eq!(store.get(m).unwrap().offspring, 4);
    assert!(store.get(f).unwrap().pregnant_due.is_none());
    assert_eq!(tallies.births[0], 4);
    assert_eq!(events.iter().filter(|e| e.kind == EventKind::Birth).count(), 1, "one Birth per litter");
    assert_eq!(events.iter().find(|e| e.kind == EventKind::Birth).and_then(|e| e.subject), Some(f));
    assert_eq!(lineage.len(), 4);
}

#[test]
fn inheritance_mean() {
    let gp = gp();
    let mut rng = Rng::new(11);
    let mother = Genome([0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.5, 0.6, 0.4]);
    let father = Genome([0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.4, 0.3, 0.3, 0.2, 0.6]);
    let mut sum = [0.0f64; Genome::LEN];
    let n = 10_000;
    for _ in 0..n {
        let (g, _) = inherit(&mother, &father, 2, &gp, &mut rng);
        for t in 0..Genome::LEN {
            sum[t] += f64::from(g.0[t]);
        }
    }
    for t in 0..Genome::LEN {
        let want = f64::from(mother.0[t] + father.0[t]) / 2.0;
        let got = sum[t] / f64::from(n);
        // Per-sample sd: the parent draw (|m − f| / 2) plus the mutation term.
        // A 4-sigma bound keeps the test honest about a systematic bias (an
        // always-mother bug is ~0.3 off) without tripping on sampling noise.
        let sd = ((f64::from(mother.0[t] - father.0[t]) / 2.0).powi(2) + f64::from(gp.mutation_rate) * f64::from(gp.mutation_strength).powi(2)).sqrt();
        let tol = 4.0 * sd / f64::from(n).sqrt();
        assert!((got - want).abs() < tol, "trait {t}: mean {got} vs parental mean {want} (tol {tol:.4})");
    }
}

#[test]
fn mutation_rate() {
    let gp = gp();
    let mut rng = Rng::new(5);
    let g = SpeciesId::Hare.base_genome();
    let n = 10_000;
    let mut count = 0usize;
    for _ in 0..n {
        let (_, m) = inherit(&g, &g, 2, &gp, &mut rng);
        count += m.len();
    }
    let freq = crate::cast!(count => f32) / (crate::cast!(n => f32) * crate::cast!(Genome::LEN => f32));
    assert!((freq - gp.mutation_rate).abs() <= gp.mutation_rate * 0.10, "mutation frequency {freq} vs rate {}", gp.mutation_rate);
}

#[test]
fn maturity_switch() {
    let mut sim = Sim::new(3, Params::default());
    let adult_age = sim.params.creatures.adult_age(SpeciesId::Vole);
    // Insert a newborn vole and age it across the boundary.
    let mut c = adult(10, 5, Sex::Male);
    c.adult = false;
    c.born_day = 0;
    let id = sim.creatures.insert(c);
    for _ in 0..(u64::from(adult_age) + 1) * 24 {
        sim.step();
        if !sim.creatures.get(id).is_some_and(|c| c.alive) {
            return; // died of natural causes on this map; nothing to assert
        }
    }
    let c = sim.creatures.get(id).unwrap();
    assert!(c.adult, "creature aged {} should be adult at {}", c.age_days(sim.time.day_index()), adult_age);
}

#[test]
fn follow_mother() {
    let w = world();
    let gp = gp();
    let t = time_at(24 * 5);
    let mut store = CreatureStore::new();
    let m = store.insert(adult(20, 5, Sex::Female));
    let mut kid = adult(2, 2, Sex::Male);
    kid.adult = false;
    kid.born_day = 0;
    kid.mother = Some(m);
    let k = store.insert(kid);
    let view = TickView::build(&store, &t, &w, &gp, &DiseaseParams::default());
    let target = follow_target(store.get(k).unwrap(), &view, &w, &t, &gp, &mut Rng::new(1)).unwrap();
    assert!(geom::cheb(target.0, target.1, 20, 5) <= 3, "target {target:?} not within 3 of the mother");
    // Past follow_mother_days: no following.
    let old = time_at(24 * (u64::from(gp.follow_mother_days) + 1));
    assert!(follow_target(store.get(k).unwrap(), &view, &w, &old, &gp, &mut Rng::new(1)).is_none());
}

#[test]
fn soft_cap_blocks_pregnancy() {
    let w = world();
    let gp = GeneticsParams { max_population_soft_cap: 2, ..gp() };
    let t = time_at(12);
    let mut store = CreatureStore::new();
    let f = store.insert(adult(5, 5, Sex::Female));
    let m = store.insert(adult(6, 5, Sex::Male));
    store.get_mut(f).unwrap().goal = Goal::Mate;
    store.get_mut(f).unwrap().mate_id = Some(m);
    let view = TickView::build(&store, &t, &w, &gp, &DiseaseParams::default());
    assert!(!view.cap_ok);
    let mut events = EventRing::new(10);
    let mut noted = false;
    consummate(&mut store, &t, &gp, &mut events, &view, &mut noted);
    assert!(store.get(f).unwrap().pregnant_due.is_none());
    assert!(noted);
    assert_eq!(events.iter().filter(|e| e.kind == EventKind::Note).count(), 1);
    // A second blocked mating does not log again.
    consummate(&mut store, &t, &gp, &mut events, &view, &mut noted);
    assert_eq!(events.iter().filter(|e| e.kind == EventKind::Note).count(), 1);
}

#[test]
fn pregnancy_hunger_factor() {
    let w = world();
    let gp = gp();
    let cp = CreaturesParams::default();
    let ep = crate::sim::params::EcologyParams::default();
    let t = time_at(12);
    let mut a = adult(5, 5, Sex::Female);
    let mut b = a.clone();
    b.pregnant_due = Some(1000);
    crate::sim::behavior::needs(&mut a, &w, &t, &cp, &ep, &gp, &DiseaseParams::default(), 1.0);
    crate::sim::behavior::needs(&mut b, &w, &t, &cp, &ep, &gp, &DiseaseParams::default(), 1.0);
    let da = a.hunger - 0.1;
    let db = b.hunger - 0.1;
    assert!((db - da * gp.pregnancy_hunger_factor).abs() < 1e-6, "pregnant gain {db} vs {da}×{}", gp.pregnancy_hunger_factor);
}
