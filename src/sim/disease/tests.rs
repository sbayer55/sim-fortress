//! Tests for the disease module.

use super::types::{DiseaseState, Infection, MAX_PATHOGENS, Outbreak, PathogenId, Stage};
use super::effects::{effects, infectious_days};
use super::contagion::{contagion_pass, on_eat, parasite_shed, parasite_uptake};
use super::spillover::at_birth;
use super::daily::{daily_update, decay_cells, progress_daily};
use crate::sim::creatures::{Cause, CreatureId, CreatureStore, DeathTallies};
use crate::sim::events::{EventKind, EventRing};
use crate::sim::geom;
use crate::sim::lineage::Lineage;
use crate::sim::params::DiseaseParams;
use crate::sim::rng::Rng;
use crate::sim::spatial::SpatialIndex;
use crate::sim::species::SpeciesId;
use crate::sim::time::Time;
use crate::sim::world::World;
use crate::sim::Alert;
use crate::sim::creatures::place_founders;
use crate::sim::params::{CreaturesParams, GeneticsParams, Params, WorldParams};
use crate::sim::stats::census;
use crate::sim::Sim;

fn world() -> World {
    World::generate(7, &WorldParams::default())
}

fn store() -> CreatureStore {
    let mut s = CreatureStore::new();
    for c in place_founders(&world(), &CreaturesParams::default(), &GeneticsParams::default(), 0.20, &mut Rng::new(1)) {
        s.insert(c);
    }
    s
}

fn day(d: u32) -> Time {
    let mut t = Time::new(6, 90, 24, 6, 20);
    for _ in 0..d * 24 {
        t.advance();
    }
    t
}

fn first_of(store: &CreatureStore, s: SpeciesId) -> CreatureId {
    store.living().find(|c| c.species == s).map(|c| c.id).unwrap()
}

#[test]
fn effects_table() {
    // Fixed weights so the expectations do not follow the balance table.
    let mut dp = DiseaseParams::default();
    dp.resist_hunger_cost = 0.25;
    let mut st = store();
    let id = first_of(&st, SpeciesId::Vole);
    let c = st.get_mut(id).unwrap();
    c.genome.0[8] = 0.5;
    c.parasite_load = 0.0;
    let e = effects(c, &dp);
    assert!((e.hunger_factor - 1.125).abs() < 1e-4, "resistance cost only: {}", e.hunger_factor);
    assert_eq!(e.speed_factor, 1.0);
    assert!(e.can_mate);
    c.infection = Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 0, ends_day: 9, severity: 0.8, source: None, outbreak: 0 });
    let e = effects(c, &dp);
    assert!((e.speed_factor - 0.6).abs() < 1e-5);
    assert!(!e.can_mate);
    assert!((e.kill_bonus - 0.2).abs() < 1e-5);
    assert!((e.hunger_factor - 1.125 * 1.3).abs() < 1e-4);
    c.parasite_load = 0.5;
    let e = effects(c, &dp);
    assert!((e.fertility_factor - 0.75).abs() < 1e-5);
}

#[test]
fn contact_probability_formula() {
    // Two voles adjacent; the infectious one rolls on the other every tick.
    let mut dp = DiseaseParams::default();
    dp.pathogens[0].transmissibility = 0.02;
    dp.susceptibility_w = 0.8;
    let w = world();
    let state = DiseaseState::new(&dp);
    let mut hits = 0u32;
    let n = 10_000;
    let mut rng = Rng::new(3);
    let mut st = store();
    let a = first_of(&st, SpeciesId::Vole);
    let b = st.living().filter(|c| c.species == SpeciesId::Vole && c.id != a).map(|c| c.id).next().unwrap();
    let (ax, ay) = {
        let c = st.get(a).unwrap();
        (c.x, c.y)
    };
    // Move b next to a, on a non-den cell, and fix b's resistance.
    let (bx, by) = crate::sim::behavior::find_walkable_near(ax, ay, &w).unwrap();
    {
        let cb = st.get_mut(b).unwrap();
        cb.x = bx;
        cb.y = by;
        cb.genome.0[8] = 0.30;
    }
    let expect = 0.02 * (1.0 - 0.8 * 0.30);
    for _ in 0..n {
        for c in st.living_mut() {
            c.infection = None;
        }
        st.get_mut(a).unwrap().infection = Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 0, ends_day: 9, severity: 0.8, source: None, outbreak: 0 });
        let mut idx = SpatialIndex::new(&w);
        idx.rebuild(&st, &w);
        contagion_pass(&mut st, &idx, &w, &day(1), &dp, &state, &mut rng);
        if st.get(b).unwrap().infection.is_some() {
            hits += 1;
        }
    }
    let got = crate::cast!(hits => f32) / crate::cast!(n => f32);
    // Other voles nearby may also be infected by `a`, but b's roll is independent.
    assert!((got - expect).abs() < expect * 0.15, "got {got} want ≈ {expect}");
    let mut idx = SpatialIndex::new(&w);
    idx.rebuild(&st, &w);
    let _ = geom::cheb(ax, ay, bx, by);
}

#[test]
fn immune_and_non_host_never_infected() {
    let dp = DiseaseParams::default();
    let w = world();
    let state = DiseaseState::new(&dp);
    let mut st = store();
    let a = first_of(&st, SpeciesId::Vole);
    let (ax, ay) = {
        let c = st.get(a).unwrap();
        (c.x, c.y)
    };
    let fox = first_of(&st, SpeciesId::Fox);
    let b = st.living().filter(|c| c.species == SpeciesId::Vole && c.id != a).map(|c| c.id).next().unwrap();
    let (bx, by) = crate::sim::behavior::find_walkable_near(ax, ay, &w).unwrap();
    for id in [fox, b] {
        let c = st.get_mut(id).unwrap();
        c.x = bx;
        c.y = by;
    }
    st.get_mut(b).unwrap().immune_until[0] = u32::MAX;
    let mut rng = Rng::new(5);
    for _ in 0..2000 {
        st.get_mut(a).unwrap().infection = Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 0, ends_day: 9, severity: 0.8, source: None, outbreak: 0 });
        let mut idx = SpatialIndex::new(&w);
        idx.rebuild(&st, &w);
        contagion_pass(&mut st, &idx, &w, &day(1), &dp, &state, &mut rng);
        assert!(st.get(fox).unwrap().infection.is_none(), "fox is not a Greyfever host");
        assert!(st.get(b).unwrap().infection.is_none(), "immune vole");
        for c in st.living_mut() {
            if c.id != a {
                c.infection = None;
            }
        }
    }
}

#[test]
fn incubation_progresses_and_recovers() {
    let mut dp = DiseaseParams::default();
    dp.pathogens[0].lethality_per_day = 0.0;
    let mut w = world();
    let mut state = DiseaseState::new(&dp);
    let mut st = store();
    let mut events = EventRing::new(100);
    let mut tallies = DeathTallies::default();
    let mut lineage = Lineage::new();
    let mut rng = Rng::new(1);
    let a = first_of(&st, SpeciesId::Vole);
    st.get_mut(a).unwrap().genome.0[8] = 0.5;
    st.get_mut(a).unwrap().infection = Some(Infection { pathogen: PathogenId(0), stage: Stage::Incubating, since_day: 0, ends_day: 3, severity: 0.8, source: None, outbreak: 0 });
    progress_daily(&mut st, &mut w, &mut events, &day(2), &dp, &mut state, &mut tallies, &mut lineage, &mut rng);
    assert_eq!(st.get(a).unwrap().infection.unwrap().stage, Stage::Incubating);
    progress_daily(&mut st, &mut w, &mut events, &day(3), &dp, &mut state, &mut tallies, &mut lineage, &mut rng);
    let inf = st.get(a).unwrap().infection.unwrap();
    assert_eq!(inf.stage, Stage::Infectious);
    // 10 × (1 − 0.4 × 0.5) = 8 days.
    assert_eq!(inf.ends_day, 3 + 8);
    assert_eq!(state.stats[0].total_cases, 1);
    for d in 4..11 {
        progress_daily(&mut st, &mut w, &mut events, &day(d), &dp, &mut state, &mut tallies, &mut lineage, &mut rng);
        assert!(st.get(a).unwrap().alive);
    }
    progress_daily(&mut st, &mut w, &mut events, &day(11), &dp, &mut state, &mut tallies, &mut lineage, &mut rng);
    let c = st.get(a).unwrap();
    assert!(c.infection.is_none(), "recovered");
    assert_eq!(c.immune_until[0], 11 + 360);
    assert_eq!(c.infections_survived, 1);
    assert!(events.iter().any(|e| e.kind == EventKind::Recovery), "severity 0.8 ≥ 0.7 emits Recovery");
}

#[test]
fn lethality_kills_on_first_infectious_day() {
    let dp = DiseaseParams::default();
    let mut w = world();
    let mut state = DiseaseState::new(&dp);
    let mut st = store();
    let mut events = EventRing::new(100);
    let mut tallies = DeathTallies::default();
    let mut lineage = Lineage::new();
    let mut rng = Rng::new(1);
    // Hazard 1.0 kills on the first infectious day (the runtime pathogen list
    // is what `progress_daily` reads, not the params roster).
    state.pathogens[0].params.lethality_per_day = 1.0;
    let b = first_of(&st, SpeciesId::Hare);
    st.get_mut(b).unwrap().genome.0[8] = 0.0;
    st.get_mut(b).unwrap().infection = Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 11, ends_day: 21, severity: 0.8, source: None, outbreak: 0 });
    progress_daily(&mut st, &mut w, &mut events, &day(12), &dp, &mut state, &mut tallies, &mut lineage, &mut rng);
    let c = st.get(b).unwrap();
    assert!(!c.alive);
    assert_eq!(c.death.unwrap().cause, Cause::Disease);
    assert_eq!(c.died_infected, Some(PathogenId(0)));
    assert_eq!(tallies.disease, 1);
    assert!(events.iter().any(|e| e.kind == EventKind::DeathDisease && e.text.contains("Greyfever")));
}

#[test]
fn lethality_by_resistance() {
    let dp = DiseaseParams::default();
    // hazard = 0.06 × host × (1 − 1.4 r), clamped at 0: r = 0.3 → 0.0348/day,
    // r = 0.9 → 0 (fully resistant).
    let p = &dp.pathogens[0];
    let h0 = (p.lethality_per_day * 1.0 * (1.0 - dp.lethality_resist_w * 0.0)).clamp(0.0, 1.0);
    let h3 = (p.lethality_per_day * 1.0 * (1.0 - dp.lethality_resist_w * 0.3)).clamp(0.0, 1.0);
    let h9 = (p.lethality_per_day * 1.0 * (1.0 - dp.lethality_resist_w * 0.9)).clamp(0.0, 1.0);
    assert!((h0 - 0.06).abs() < 1e-6);
    assert!((h3 - 0.0348).abs() < 1e-5);
    assert_eq!(h9, 0.0);
    assert_eq!(infectious_days(p, 0.0, &dp), 10);
    assert_eq!(infectious_days(p, 0.98, &dp), 6);
}

#[test]
fn vertical_transmission_and_birth_load() {
    let mut dp = DiseaseParams::default();
    dp.vertical_transmission = 1.0;
    let state = DiseaseState::new(&dp);
    let st = store();
    let a = first_of(&st, SpeciesId::Vole);
    let mut mother = st.get(a).unwrap().clone();
    mother.parasite_load = 0.6;
    mother.infection = Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 0, ends_day: 9, severity: 0.8, source: None, outbreak: 3 });
    let mut child = mother.clone();
    child.infection = None;
    child.parasite_load = 0.0;
    at_birth(&mut child, &mother, &day(1), &dp, &state, &mut Rng::new(1));
    assert!((child.parasite_load - 0.18f32.max(dp.parasite_baseline)).abs() < 1e-5);
    let inf = child.infection.unwrap();
    assert_eq!(inf.stage, Stage::Incubating);
    assert_eq!(inf.outbreak, 3);
    assert_eq!(inf.source, Some(mother.id));
}

#[test]
fn carcass_transmission_and_parasite_transfer() {
    let mut dp = DiseaseParams::default();
    dp.carcass_transmission = 1.0;
    let w = world();
    let mut state = DiseaseState::new(&dp);
    let mut st = store();
    let mut events = EventRing::new(10);
    let mut rng = Rng::new(2);
    let fox = first_of(&st, SpeciesId::Fox);
    let vole = first_of(&st, SpeciesId::Vole);
    let hare = first_of(&st, SpeciesId::Hare);
    {
        let k = st.get_mut(vole).unwrap();
        k.alive = false;
        k.parasite_load = 0.8;
        k.died_infected = Some(PathogenId(0)); // Greyfever: fox is not a host
    }
    dp.spillover_chance = 0.0;
    on_eat(&mut st, fox, vole, &w, &mut events, &day(1), &dp, &mut state, &mut rng);
    let f = st.get(fox).unwrap();
    assert!((f.parasite_load - 0.4).abs() < 1e-5, "trophic transfer ≥ 0.35: {}", f.parasite_load);
    assert!(f.infection.is_none(), "non-host, spillover disabled");
    // A host eater (hare scavenging is not a thing, but the rule is general).
    st.get_mut(hare).unwrap().genome.0[8] = 0.0;
    on_eat(&mut st, hare, vole, &w, &mut events, &day(1), &dp, &mut state, &mut rng);
    assert!(st.get(hare).unwrap().infection.is_some(), "host eater is infected at transmission 1.0");
}

#[test]
fn spillover_creates_strain_and_index_case() {
    let mut dp = DiseaseParams::default();
    dp.spillover_chance = 1.0;
    let w = world();
    let mut state = DiseaseState::new(&dp);
    let mut st = store();
    let mut events = EventRing::new(10);
    let mut rng = Rng::new(2);
    let fox = first_of(&st, SpeciesId::Fox);
    let vole = first_of(&st, SpeciesId::Vole);
    {
        let k = st.get_mut(vole).unwrap();
        k.alive = false;
        k.died_infected = Some(PathogenId(0));
    }
    on_eat(&mut st, fox, vole, &w, &mut events, &day(5), &dp, &mut state, &mut rng);
    assert_eq!(state.pathogens.len(), 4);
    let strain = &state.pathogens[3];
    assert_eq!(strain.name(), "Greyfever (foxes strain)");
    assert_eq!(strain.parent, Some(PathogenId(0)));
    assert_eq!(strain.host(SpeciesId::Fox), 1.0);
    assert_eq!(strain.host(SpeciesId::Vole), 0.0);
    let f = st.get(fox).unwrap();
    let inf = f.infection.unwrap();
    assert_eq!(inf.pathogen, PathogenId(3));
    assert_eq!(inf.stage, Stage::Infectious);
    assert_eq!(state.outbreaks.len(), 1);
    assert_eq!(state.outbreaks[0].index_case, fox);
    assert!(events.iter().any(|e| e.kind == EventKind::Spillover && e.text.contains("jumped to the foxes")));
    assert_eq!(state.root(PathogenId(3)), PathogenId(0));
}

#[test]
fn spillover_slot_reuse_zeroes_immunity() {
    let mut dp = DiseaseParams::default();
    dp.spillover_chance = 1.0;
    dp.max_pathogens = 4;
    dp.reservoir_days = 10;
    let w = world();
    let mut state = DiseaseState::new(&dp);
    let mut st = store();
    let mut events = EventRing::new(10);
    let mut rng = Rng::new(2);
    let vole = first_of(&st, SpeciesId::Vole);
    st.get_mut(vole).unwrap().alive = false;
    st.get_mut(vole).unwrap().died_infected = Some(PathogenId(0));
    let foxes: Vec<CreatureId> = st.living().filter(|c| c.species == SpeciesId::Fox).map(|c| c.id).take(2).collect();
    on_eat(&mut st, foxes[0], vole, &w, &mut events, &day(5), &dp, &mut state, &mut rng);
    assert_eq!(state.pathogens.len(), 4);
    // Second spillover: no free slot, strain not extinct → fails.
    on_eat(&mut st, foxes[1], vole, &w, &mut events, &day(6), &dp, &mut state, &mut rng);
    assert_eq!(state.failed_spillovers, 1);
    // Mark the strain extinct and past its reservoir; give a fox immunity to it.
    state.pathogens[3].extinct = true;
    state.last_case_day[3] = 0;
    st.get_mut(foxes[1]).unwrap().immune_until[3] = u32::MAX;
    st.get_mut(foxes[1]).unwrap().infection = None;
    on_eat(&mut st, foxes[1], vole, &w, &mut events, &day(40), &dp, &mut state, &mut rng);
    assert_eq!(state.pathogens.len(), 4);
    assert_eq!(state.pathogens[3].born_day, Some(40));
    assert!(st.get(foxes[1]).unwrap().infection.is_some(), "immunity to the old strain was cleared");
}

#[test]
fn emergence_needs_min_hosts_and_reservoir() {
    let mut dp = DiseaseParams::default();
    dp.emergence_per_day = 1.0;
    dp.emergence_host_ref = 1;
    let w = world();
    let mut state = DiseaseState::new(&dp);
    let mut st = store();
    let mut events = EventRing::new(10);
    let mut rng = Rng::new(9);
    let pop = census(&st).population;
    // Enough hosts: Greyfever emerges on day 1.
    let alerts = daily_update(&mut st, &w, &mut events, &day(1), &dp, &mut state, &pop, &mut rng);
    assert!(alerts.is_empty());
    assert_eq!(state.stats[0].outbreaks, 1);
    assert!(events.iter().any(|e| e.kind == EventKind::Outbreak && e.text.starts_with("Greyfever breaks out")));
    let o = &state.outbreaks[0];
    let idx = st.get(o.index_case).unwrap();
    assert_eq!(idx.infection.unwrap().stage, Stage::Infectious);
    // The index case is the lowest-resistance host of the densest region.
    let region = crate::cast!(o.origin_region => usize);
    let min_r = st
        .living()
        .filter(|c| c.species.kind() == crate::sim::species::Kind::Prey && w.region_index(c.x, c.y).min(7) == region)
        .map(|c| c.genome.resistance())
        .fold(f32::INFINITY, f32::min);
    assert!((idx.genome.resistance() - min_r).abs() < 1e-6);
    // Reservoir: clear the case, no re-emergence within reservoir_days.
    st.get_mut(o.index_case).unwrap().infection = None;
    let _ = daily_update(&mut st, &w, &mut events, &day(2), &dp, &mut state, &pop, &mut rng);
    assert_eq!(state.stats[0].outbreaks, 1, "reservoir cooldown holds");
    assert!(state.outbreaks[0].ended_day.is_some());
    let _ = daily_update(&mut st, &w, &mut events, &day(200), &dp, &mut state, &pop, &mut rng);
    assert_eq!(state.stats[0].outbreaks, 2, "re-emerges after the reservoir");
    // Too few hosts: no emergence.
    let few = [10u32, 10, 10, 0, 0, 0];
    for c in st.living_mut() {
        c.infection = None;
    }
    state.last_case_day = [u32::MAX; MAX_PATHOGENS];
    let mut state2 = DiseaseState::new(&dp);
    let _ = daily_update(&mut st, &w, &mut events, &day(300), &dp, &mut state2, &few, &mut rng);
    assert_eq!(state2.stats[0].outbreaks, 0);
}

#[test]
fn epidemic_threshold_once_and_outbreak_ends() {
    let mut dp = DiseaseParams::default();
    dp.emergence_per_day = 0.0;
    dp.epidemic_min_cases = 3;
    dp.epidemic_share = 0.001;
    let w = world();
    let mut state = DiseaseState::new(&dp);
    let mut st = store();
    let mut events = EventRing::new(10);
    let mut rng = Rng::new(9);
    let pop = census(&st).population;
    let voles: Vec<CreatureId> = st.living().filter(|c| c.species == SpeciesId::Vole).map(|c| c.id).take(3).collect();
    let index = state.push_outbreak(Outbreak {
        pathogen: PathogenId(0),
        started_day: 0,
        ended_day: None,
        origin_region: 0,
        index_case: voles[0],
        cases: 3,
        deaths: 0,
        recovered: 0,
        peak_active: 1,
        peak_day: 0,
        species_cases: [3, 0, 0, 0, 0, 0],
        species_deaths: [0; 6],
        epidemic: false,
        resist_at_start: [0.0; 6],
        resist_at_end: [0.0; 6],
        active: 0,
        cases_today: 0,
    });
    for id in &voles {
        st.get_mut(*id).unwrap().infection = Some(Infection { pathogen: PathogenId(0), stage: Stage::Infectious, since_day: 0, ends_day: 9, severity: 0.8, source: None, outbreak: index });
    }
    let alerts = daily_update(&mut st, &w, &mut events, &day(1), &dp, &mut state, &pop, &mut rng);
    assert_eq!(alerts.len(), 1);
    assert!(matches!(alerts[0], Alert::Epidemic { pathogen: PathogenId(0), .. }));
    let alerts = daily_update(&mut st, &w, &mut events, &day(2), &dp, &mut state, &pop, &mut rng);
    assert!(alerts.is_empty(), "epidemic fires once per outbreak");
    for id in &voles {
        st.get_mut(*id).unwrap().infection = None;
    }
    let _ = daily_update(&mut st, &w, &mut events, &day(3), &dp, &mut state, &pop, &mut rng);
    assert_eq!(state.outbreaks[0].ended_day, Some(3));
    assert!(events.iter().any(|e| e.kind == EventKind::EpidemicOver));
}

#[test]
fn parasite_uptake_shed_clear() {
    let dp = DiseaseParams::default();
    let mut w = world();
    let mut st = store();
    let a = first_of(&st, SpeciesId::Deer);
    let (x, y) = {
        let c = st.get(a).unwrap();
        (c.x, c.y)
    };
    w.cell_mut(x, y).parasite_load = 0.5;
    {
        let c = st.get_mut(a).unwrap();
        c.genome.0[8] = 0.5;
        parasite_uptake(c, &w, &dp);
        assert!((c.parasite_load - dp.parasite_uptake * 0.5 * 0.5).abs() < 1e-6);
        c.parasite_load = 0.5;
        parasite_shed(c, &mut w, &dp);
    }
    let after_shed = 0.5 + dp.parasite_shed * 0.5;
    assert!((w.cell(x, y).parasite_load - after_shed).abs() < 1e-6);
    w.cell_mut(x, y).prey_pressure = 0.0;
    w.cell_mut(x, y).pred_pressure = 0.0;
    decay_cells(&mut w, &dp);
    assert!((w.cell(x, y).parasite_load - after_shed * dp.parasite_cell_decay).abs() < 1e-5);
}

#[test]
fn disabled_is_inert() {
    let mut p = Params::default();
    p.disease.enabled = false;
    let mut sim = Sim::new(3, p);
    for _ in 0..24 * 60 {
        sim.step();
    }
    assert!(sim.creatures.living().all(|c| c.infection.is_none() && c.parasite_load == 0.0));
    assert!(sim.disease.outbreaks.is_empty());
}
