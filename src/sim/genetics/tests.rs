//! Unit tests for the `genetics` module, extracted from `genetics.rs`.

use super::*;
use crate::sim::species::testing::*;
use crate::sim::params::CreaturesParams;
use crate::sim::params::Roster;
use crate::sim::creatures::max_age_days;
use crate::sim::params::{Params, WorldParams};
use crate::sim::species::{IDX_MATURITY, IDX_MUTABILITY, N_TRAITS};
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
    GeneticsParams { mate_hunger_max: 0.3, mate_cell_vegetation_min: 0.3, ..GeneticsParams::default() }
}

/// The doc's FR1 litter and cooldown table, on top of the default roster.
fn test_roster() -> &'static Roster {
    static ROSTER: std::sync::OnceLock<Roster> = std::sync::OnceLock::new();
    ROSTER.get_or_init(|| {
        let mut r = Roster::default();
        for (s, (litter, cooldown)) in r.0.iter_mut().zip([(3.0, 20), (2.0, 30), (1.0, 150), (3.0, 120), (2.0, 180), (1.0, 180)]) {
            s.litter_max = litter;
            s.mate_cooldown_days = cooldown;
        }
        r
    })
}

fn adult(x: usize, y: usize, sex: Sex) -> Creature {
    Creature {
        id: CreatureId(0),
        species: VOLE,
        name: 0,
        sex,
        x,
        y,
        born_day: -100,
        generation: 1,
        parents: None,
        genome: genome(VOLE),
        hp: 1.0,
        hunger: 0.1,
        thirst: 0.1,
        energy: 0.9,
        adult: true,
        sterile: false,
        goal: Goal::Wander,
        target: None,
        replan_at: 0,
        trail: Vec::new(),
        alive: true,
        death: None,
        decay: 0.0,
        mutations: Vec::new(),
        last_water: None,
        last_ate: None,
        last_drank: None,
        last_slept: None,
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
        threats_by_species: vec![0; N_SPECIES],
        kills_by_species: vec![0; N_SPECIES],
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
        wary_until: 0,
        wary_by: None,
        wary_count: 0,
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
    assert!(eligible(&c, &t, &w, test_roster(), &gp, &DiseaseParams::default()));
    c.adult = false;
    assert!(!eligible(&c, &t, &w, test_roster(), &gp, &DiseaseParams::default()), "juveniles never mate");
    c.adult = true;
    c.hunger = 0.5;
    assert!(!eligible(&c, &t, &w, test_roster(), &gp, &DiseaseParams::default()), "too hungry");
    c.hunger = 0.1;
    c.cooldown_until = 100;
    assert!(!eligible(&c, &t, &w, test_roster(), &gp, &DiseaseParams::default()), "on cooldown");
    c.cooldown_until = 0;
    // Winter: day index 270+ → tick 270*24.
    assert!(!eligible(&c, &time_at(270 * 24), &w, test_roster(), &gp, &DiseaseParams::default()), "not a breeding season");
    c.sterile = true;
    assert!(!eligible(&c, &t, &w, test_roster(), &gp, &DiseaseParams::default()), "sterile adults never mate");
    c.sterile = false;
    let mut bare = world();
    bare.cell_mut(5, 5).vegetation = 0.0;
    assert!(!eligible(&c, &t, &bare, test_roster(), &gp, &DiseaseParams::default()), "prey need vegetation on the cell");
}

#[test]
fn mating_sets_cooldown_and_pregnancy() {
    let w = world();
    let gp = gp();
    let t = time_at(12);
    let mut store = CreatureStore::new();
    let f = store.insert(adult(5, 5, Sex::Female));
    let m = store.insert(adult(6, 5, Sex::Male));
    let view = TickView::build(&store, &t, &w, test_roster(), &gp, &DiseaseParams::default());
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
    consummate(&mut store, &t, test_roster(), &gp, &mut events, &view, &mut noted);
    let female = store.get(f).unwrap();
    let male = store.get(m).unwrap();
    let cd = 12 + u64::from(test_roster().get(VOLE).mate_cooldown_days) * 24;
    assert_eq!(female.cooldown_until, cd);
    assert_eq!(male.cooldown_until, cd);
    assert_eq!(female.pregnant_due, Some(12 + u64::from(test_roster().get(VOLE).gestation_days) * 24));
    assert_eq!(female.mate_id, Some(m));
    assert!(male.pregnant_due.is_none());
    assert!(male.mate_id.is_none());
}

#[test]
fn maturity_scales_adult_age_litter_and_lifespan() {
    let gp = GeneticsParams::default();
    let cp = CreaturesParams::default();
    let id = DEER;
    let neutral = genome(id); // maturity 0.5 by construction
    let mut slow = genome(id);
    slow.0[IDX_MATURITY] = 0.98;
    let mut fast = genome(id);
    fast.0[IDX_MATURITY] = 0.02;

    // Maturity 0.5 reproduces the pre-maturity numbers exactly.
    assert_eq!(adult_age_days(test_roster().get(id), &neutral, &gp), test_roster().get(id).adult_age_days);
    let old_max = cp.max_age_base + crate::cast!((neutral.longevity() * crate::cast!(cp.max_age_per_longevity => f32)) => u32);
    assert_eq!(max_age_days(&neutral, &cp, &gp), old_max);
    assert_eq!(gp.litter_size(test_roster().get(id).litter_max, neutral.fertility(), 0.5), 1 + crate::cast!((neutral.fertility() * test_roster().get(id).litter_max).round() => u32));

    // Slow: later, larger, longer. Fast: the reverse.
    assert!(adult_age_days(test_roster().get(id), &slow, &gp) > adult_age_days(test_roster().get(id), &neutral, &gp));
    assert!(adult_age_days(test_roster().get(id), &neutral, &gp) > adult_age_days(test_roster().get(id), &fast, &gp));
    assert!(gp.litter_size(test_roster().get(id).litter_max, slow.fertility(), slow.maturity()) > gp.litter_size(test_roster().get(id).litter_max, fast.fertility(), fast.maturity()));
    assert!(max_age_days(&slow, &cp, &gp) > old_max);
    assert!(max_age_days(&fast, &cp, &gp) < old_max);
}

#[test]
fn inherit_covers_every_slot() {
    // Every trait index, including the two C8 additions, is inherited and can
    // mutate (mutation_rate 1 makes the draw deterministic).
    let gp = GeneticsParams { mutation_rate: 1.0, mutation_strength: 0.0, ..GeneticsParams::default() };
    let g = genome(WOLF);
    let (out, muts) = inherit(&g, &g, 4, &gp, &mut Rng::new(3));
    assert_eq!(muts.len(), N_TRAITS, "one mutation per slot");
    let mut idx: Vec<usize> = muts.iter().map(|m| m.trait_idx).collect();
    idx.sort_unstable();
    assert_eq!(idx, (0..N_TRAITS).collect::<Vec<usize>>());
    assert_eq!(out.sociality(), g.sociality());
    assert_eq!(out.maturity(), g.maturity());
    assert_eq!(out.mutability(), g.mutability());

    // Parents at the bottom of the Mutability range scale the rate down, so
    // the effective rate is a valid probability below 1 and some slots are
    // inherited untouched; at the top it clamps to 1 without panicking.
    let mut low = g;
    low.0[IDX_MUTABILITY] = 0.02;
    let (_, muts) = inherit(&low, &low, 4, &gp, &mut Rng::new(3));
    assert!(muts.len() < N_TRAITS, "low mutability must scale a rate of 1 below 1");
    let mut high = g;
    high.0[IDX_MUTABILITY] = 0.98;
    let (_, muts) = inherit(&high, &high, 4, &gp, &mut Rng::new(3));
    assert_eq!(muts.len(), N_TRAITS, "rate clamps to 1 at high mutability");
}

#[test]
fn mutability_is_neutral_at_half() {
    let gp = GeneticsParams::default();
    assert_eq!(gp.effective_mutation(0.5), (gp.mutation_rate, gp.mutation_strength));
    // The base genomes all sit at 0.5, so a fresh world mutates exactly as before.
    for id in test_roster().ids() {
        assert_eq!(gp.effective_mutation(genome(id).mutability()), (gp.mutation_rate, gp.mutation_strength));
    }
    let (lo_rate, lo_sd) = gp.effective_mutation(0.02);
    let (hi_rate, hi_sd) = gp.effective_mutation(0.98);
    assert!(lo_rate < gp.mutation_rate && gp.mutation_rate < hi_rate);
    assert!(lo_sd < gp.mutation_strength && gp.mutation_strength < hi_sd);
    assert!(lo_rate > 0.0 && lo_sd > 0.0, "the low end must never freeze a lineage");
}

/// 10 000 births for parents at one Mutability: (mutations per slot, mean |delta|).
fn mutation_profile(mutability: f32, gp: &GeneticsParams, seed: u64) -> (f32, f32) {
    let mut rng = Rng::new(seed);
    let mut g = genome(HARE);
    g.0[IDX_MUTABILITY] = mutability;
    let n = 10_000;
    let mut count = 0usize;
    let mut abs_sum = 0.0f64;
    for _ in 0..n {
        let (_, m) = inherit(&g, &g, 2, gp, &mut rng);
        count += m.len();
        abs_sum += m.iter().map(|mu| f64::from(mu.delta.abs())).sum::<f64>();
    }
    let freq = crate::cast!(count => f32) / (crate::cast!(n => f32) * crate::cast!(Genome::LEN => f32));
    let mean_abs = crate::cast!((abs_sum / f64::from(crate::cast!(count.max(1) => u32))) => f32);
    (freq, mean_abs)
}

#[test]
fn mutability_scales_rate_and_strength() {
    let gp = gp();
    let (lo_freq, lo_abs) = mutation_profile(0.02, &gp, 21);
    let (hi_freq, hi_abs) = mutation_profile(0.98, &gp, 22);
    let (lo_rate, lo_sd) = gp.effective_mutation(0.02);
    let (hi_rate, hi_sd) = gp.effective_mutation(0.98);
    // Frequencies track the effective rates within 10 %.
    assert!((lo_freq - lo_rate).abs() <= lo_rate * 0.10, "low freq {lo_freq} vs rate {lo_rate}");
    assert!((hi_freq - hi_rate).abs() <= hi_rate * 0.10, "high freq {hi_freq} vs rate {hi_rate}");
    // Mean |delta| scales with the sd (clamping at 0.02..0.98 barely bites for a
    // hare genome, so the ratio is within 10 % of the sd ratio).
    let want = hi_sd / lo_sd;
    let got = hi_abs / lo_abs;
    assert!((got - want).abs() <= want * 0.10, "|delta| ratio {got} vs sd ratio {want}");
    assert!(lo_freq > 0.0, "the low end still mutates");
}

#[test]
fn sterility_chance_ramps_to_the_cap() {
    let gp = GeneticsParams::default();
    assert_eq!(gp.sterility_chance(0.02), 0.0);
    assert_eq!(gp.sterility_chance(0.5), 0.0);
    assert_eq!(gp.sterility_chance(gp.sterility_onset), 0.0);
    assert_eq!(gp.sterility_chance(0.98), gp.sterility_max);
    let mut prev = 0.0f32;
    for i in 0..=20 {
        let m = gp.sterility_onset + (0.98 - gp.sterility_onset) * crate::cast!(i => f32) / 20.0;
        let p = gp.sterility_chance(m);
        assert!(p >= prev, "monotone at {m}");
        prev = p;
    }
    // Quadratic: halfway up the ramp is a quarter of the max.
    let mid = gp.sterility_chance((gp.sterility_onset + 0.98) / 2.0);
    assert!((mid - gp.sterility_max / 4.0).abs() < 1e-5, "mid {mid}");
    for id in test_roster().ids() {
        assert_eq!(gp.sterility_chance(genome(id).mutability()), 0.0, "{id:?} base genome must carry no risk");
    }
    // A degenerate onset at (or above) the cap only bites at the cap itself.
    let edge = GeneticsParams { sterility_onset: 0.98, ..GeneticsParams::default() };
    assert_eq!(edge.sterility_chance(0.9), 0.0);
    assert_eq!(edge.sterility_chance(0.98), edge.sterility_max);
}

/// Deliver one litter from parents at `mutability`; returns the pups' `sterile` flags.
fn litter_sterility(mutability: f32, gp: &GeneticsParams) -> Vec<bool> {
    let w = world();
    let mut store = CreatureStore::new();
    let mut mother = adult(5, 5, Sex::Female);
    mother.genome.0[6] = 0.9; // litter 1 + round(0.9×3) = 4
    mother.genome.0[IDX_MUTABILITY] = mutability;
    let mut father = adult(6, 5, Sex::Male);
    father.genome.0[IDX_MUTABILITY] = mutability;
    let f = store.insert(mother);
    let m = store.insert(father);
    store.get_mut(f).unwrap().pregnant_due = Some(10);
    store.get_mut(f).unwrap().mate_id = Some(m);
    let mut events = EventRing::new(10);
    let mut tallies = DeathTallies::new(N_SPECIES);
    let mut lineage = Lineage::new();
    let born = deliver(&mut store, &w, &mut events, &time_at(10), test_roster(), gp, &DiseaseParams::default(), &mut Rng::new(3), &mut tallies, &mut lineage, &DiseaseState::new(&DiseaseParams::default(), test_roster()), &mut Rng::new(4));
    assert_eq!(born, 4);
    store.living().filter(|c| c.parents.is_some()).map(|c| c.sterile).collect()
}

#[test]
fn high_mutability_pups_can_be_born_sterile() {
    // Mutations cannot move a 0.98 parent pair's pup past the cap, so with
    // `sterility_max` 1 every pup is sterile; base genomes carry no risk.
    let certain = GeneticsParams { sterility_max: 1.0, mutation_rate: 0.0, ..gp() };
    assert!(litter_sterility(0.98, &certain).iter().all(|&s| s));
    assert!(litter_sterility(0.5, &certain).iter().all(|&s| !s));
    // At the default max the roll is real: the same seed yields a mix or all
    // one way, but never a panic and never sterility below the onset.
    assert!(litter_sterility(0.7, &gp()).iter().all(|&s| !s));
}

#[test]
fn litter_size_from_fertility() {
    let gp = gp();
    assert_eq!(gp.litter_size(test_roster().get(VOLE).litter_max, 0.0, 0.5), 1);
    assert_eq!(gp.litter_size(test_roster().get(VOLE).litter_max, 0.9, 0.5), 4);
    assert_eq!(gp.litter_size(test_roster().get(DEER).litter_max, 0.35, 0.5), 1);
    assert_eq!(gp.litter_size(test_roster().get(HARE).litter_max, 0.75, 0.5), 3);
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

    let mut store = CreatureStore::new();
    let mut mother = adult(5, 5, Sex::Female);
    mother.genome.0[6] = 0.9; // litter 1 + round(0.9×3) = 4
    let f = store.insert(mother);
    let m = store.insert(adult(6, 5, Sex::Male));
    store.get_mut(f).unwrap().pregnant_due = Some(10);
    store.get_mut(f).unwrap().mate_id = Some(m);
    let mut events = EventRing::new(10);
    let mut tallies = DeathTallies::new(N_SPECIES);
    let mut lineage = Lineage::new();
    let born = deliver(&mut store, &w, &mut events, &time_at(10), test_roster(), &gp, &DiseaseParams::default(), &mut Rng::new(3), &mut tallies, &mut lineage, &DiseaseState::new(&DiseaseParams::default(), test_roster()), &mut Rng::new(4));
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
    let mother = Genome([0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.5, 0.6, 0.4, 0.5, 0.7]);
    let father = Genome([0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.4, 0.3, 0.3, 0.2, 0.6, 0.5, 0.3]);
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
    let g = genome(HARE);
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
    let adult_age = sim.roster().get(VOLE).adult_age_days;
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
    let view = TickView::build(&store, &t, &w, test_roster(), &gp, &DiseaseParams::default());
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
    let view = TickView::build(&store, &t, &w, test_roster(), &gp, &DiseaseParams::default());
    assert!(!view.cap_ok);
    let mut events = EventRing::new(10);
    let mut noted = false;
    consummate(&mut store, &t, test_roster(), &gp, &mut events, &view, &mut noted);
    assert!(store.get(f).unwrap().pregnant_due.is_none());
    assert!(noted);
    assert_eq!(events.iter().filter(|e| e.kind == EventKind::Note).count(), 1);
    // A second blocked mating does not log again.
    consummate(&mut store, &t, test_roster(), &gp, &mut events, &view, &mut noted);
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
