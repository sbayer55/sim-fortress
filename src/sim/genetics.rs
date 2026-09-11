//! Reproduction, inheritance and maturity (C4): mate eligibility and selection,
//! consummation, pregnancy, litters, inheritance with mutation and the
//! per-tick peer snapshot that lets a creature see potential partners and its
//! mother while the store is being iterated mutably.

use crate::sim::creatures::{Creature, CreatureId, CreatureStore, DeathTallies, Goal, Mutation, NameId, Sex};
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::geom;
use crate::sim::lineage::Lineage;
use crate::sim::params::{CreaturesParams, GeneticsParams};
use crate::sim::rng::Rng;
use crate::sim::species::{names, Genome, Kind, SpeciesId, TRAIT_NAMES};
use crate::sim::time::Time;
use crate::sim::world::World;

/// The 8-neighbour offsets, row-major.
pub const OFF8: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// What one living creature looks like to the others this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Peer {
    pub id: CreatureId,
    pub species: SpeciesId,
    pub sex: Sex,
    pub x: usize,
    pub y: usize,
    pub adult: bool,
    /// Passes the FR2 eligibility rule this tick.
    pub mate_ready: bool,
}

/// Per-tick snapshot of every living creature, sorted by id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TickView {
    peers: Vec<Peer>,
    pub total: usize,
    /// `total < max_population_soft_cap`: new pregnancies allowed.
    pub cap_ok: bool,
}

impl TickView {
    pub fn empty() -> Self {
        TickView { peers: Vec::new(), total: 0, cap_ok: true }
    }

    pub fn build(store: &CreatureStore, time: &Time, world: &World, gp: &GeneticsParams) -> Self {
        let mut peers: Vec<Peer> = store
            .living()
            .map(|c| Peer {
                id: c.id,
                species: c.species,
                sex: c.sex,
                x: c.x,
                y: c.y,
                adult: c.adult,
                mate_ready: eligible(c, time, world, gp),
            })
            .collect();
        peers.sort_unstable_by_key(|p| p.id);
        let total = peers.len();
        TickView { peers, total, cap_ok: (total as u32) < gp.max_population_soft_cap }
    }

    pub fn get(&self, id: CreatureId) -> Option<&Peer> {
        self.peers.binary_search_by_key(&id, |p| p.id).ok().map(|i| &self.peers[i])
    }
}

/// FR2 mate eligibility: adult, fed, watered, rested, off cooldown, in a
/// breeding season and (prey only) standing on vegetation ≥ the minimum.
pub fn eligible(c: &Creature, time: &Time, world: &World, gp: &GeneticsParams) -> bool {
    if !c.alive || !c.adult || c.pregnant_due.is_some() {
        return false;
    }
    if c.hunger >= gp.mate_hunger_max || c.thirst >= gp.mate_thirst_max || c.energy <= gp.mate_energy_min {
        return false;
    }
    if time.tick < c.cooldown_until {
        return false;
    }
    if !gp.breeding_seasons.contains(&time.season()) {
        return false;
    }
    if c.species.kind() == Kind::Prey && world.cell(c.x, c.y).vegetation < gp.mate_cell_vegetation_min {
        return false;
    }
    true
}

/// Nearest eligible opposite-sex adult of the same species among `candidates`
/// (perception ids, ascending), ties by id.
pub fn pick_mate(c: &Creature, candidates: &[CreatureId], view: &TickView) -> Option<(CreatureId, (usize, usize))> {
    let mut best: Option<(CreatureId, (usize, usize))> = None;
    let mut best_d = f32::INFINITY;
    let mut ids: Vec<CreatureId> = candidates.to_vec();
    ids.sort_unstable();
    for id in ids {
        if id == c.id {
            continue;
        }
        let Some(p) = view.get(id) else { continue };
        if p.species != c.species || p.sex == c.sex || !p.adult || !p.mate_ready {
            continue;
        }
        let d = geom::dist(c.x, c.y, p.x, p.y);
        if d < best_d {
            best_d = d;
            best = Some((id, (p.x, p.y)));
        }
    }
    best
}

/// FR3 inheritance: per trait a random parent's value, plus with probability
/// `mutation_rate` a gaussian `N(0, mutation_strength)` delta, clamped.
pub fn inherit(mother: &Genome, father: &Genome, generation: u32, gp: &GeneticsParams, rng: &mut Rng) -> (Genome, Vec<Mutation>) {
    let mut g = [0.0f32; 8];
    let mut mutations = Vec::new();
    for t in 0..8 {
        let base = if rng.chance(0.5) { mother.0[t] } else { father.0[t] };
        let mut v = base;
        if rng.chance(gp.mutation_rate) {
            let delta = rng.gauss(0.0, gp.mutation_strength);
            v = Genome::clamp_trait(base + delta);
            mutations.push(Mutation { trait_idx: t, delta: v - base, generation });
        }
        g[t] = Genome::clamp_trait(v);
    }
    (Genome(g), mutations)
}

/// Second pass of a tick: creatures seeking a mate that are adjacent to their
/// chosen partner mate (FR2). Both get the cooldown; the female becomes
/// pregnant and remembers the father in `mate_id`.
pub fn consummate(store: &mut CreatureStore, time: &Time, gp: &GeneticsParams, events: &mut EventRing, view: &TickView, soft_cap_noted: &mut bool) {
    let seekers: Vec<(CreatureId, CreatureId)> = store
        .living()
        .filter(|c| c.goal == Goal::Mate && c.pregnant_due.is_none())
        .filter_map(|c| c.mate_id.map(|m| (c.id, m)))
        .collect();
    let mut seekers = seekers;
    seekers.sort_unstable();

    for (a_id, b_id) in seekers {
        let Some(a) = store.get(a_id) else { continue };
        let Some(b) = store.get(b_id) else { continue };
        if !a.alive || !b.alive || a.species != b.species || a.sex == b.sex || !a.adult || !b.adult {
            continue;
        }
        if time.tick < a.cooldown_until || time.tick < b.cooldown_until || a.pregnant_due.is_some() || b.pregnant_due.is_some() {
            continue;
        }
        if geom::cheb(a.x, a.y, b.x, b.y) > 1 {
            continue;
        }
        if !view.cap_ok {
            if !*soft_cap_noted {
                *soft_cap_noted = true;
                events.push(Event {
                    year: time.year(),
                    day: time.day_of_year(),
                    hour: time.hour(),
                    kind: EventKind::Note,
                    species: None,
                    subject: None,
                    text: format!("population soft cap reached ({}); no new pregnancies", gp.max_population_soft_cap),
                    pos: None,
                    detail: String::new(),
                });
            }
            continue;
        }
        let (mother_id, father_id) = if a.sex == Sex::Female { (a_id, b_id) } else { (b_id, a_id) };
        let species = a.species;
        let cooldown = time.tick + gp.cooldown(species) as u64 * time.ticks_per_day as u64;
        let due = time.tick + gp.gestation(species) as u64 * time.ticks_per_day as u64;

        if let Some(m) = store.get_mut(mother_id) {
            m.cooldown_until = cooldown;
            m.pregnant_due = Some(due);
            m.mate_id = Some(father_id);
            m.goal = Goal::Wander;
            m.target = None;
            m.replan_at = time.tick;
        }
        if let Some(f) = store.get_mut(father_id) {
            f.cooldown_until = cooldown;
            f.mate_id = None;
            f.goal = Goal::Wander;
            f.target = None;
            f.replan_at = time.tick;
        }
    }
}

/// Third pass of a tick: every pregnant female whose `pregnant_due` has passed
/// delivers her litter (FR2/FR3). Returns the number of newborns.
#[allow(clippy::too_many_arguments)]
pub fn deliver(
    store: &mut CreatureStore,
    world: &World,
    events: &mut EventRing,
    time: &Time,
    gp: &GeneticsParams,
    cp: &CreaturesParams,
    rng: &mut Rng,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
) -> u32 {
    let due: Vec<CreatureId> = store
        .living()
        .filter(|c| c.pregnant_due.is_some_and(|d| d <= time.tick))
        .map(|c| c.id)
        .collect();
    let mut born_total = 0;
    for mother_id in due {
        let Some(m) = store.get(mother_id) else { continue };
        let species = m.species;
        let (mx, my) = (m.x, m.y);
        let mother_genome = m.genome;
        let mother_gen = m.generation;
        let father_id = m.mate_id.unwrap_or(mother_id);
        let litter = gp.litter_size(species, m.genome.fertility());
        let mother_label = format!("{} {}", m.name_str(), m.tag());
        let mother_water = m.last_water;

        let (father_genome, father_gen) = match store.get(father_id) {
            Some(f) => (f.genome, f.generation),
            None => match lineage.get(father_id) {
                Some(n) => (n.genome, n.generation),
                None => (mother_genome, mother_gen),
            },
        };
        let generation = mother_gen.max(father_gen) + 1;

        // Placement: the mother's cell, then 8-adjacent walkable cells.
        let mut cells: Vec<(usize, usize)> = vec![(mx, my)];
        for &(dx, dy) in &OFF8 {
            let (nx, ny) = (mx as i32 + dx, my as i32 + dy);
            if world.in_bounds(nx, ny) {
                let t = world.cell(nx as usize, ny as usize).terrain;
                if t.walkable() && !t.is_water() {
                    cells.push((nx as usize, ny as usize));
                }
            }
        }

        let pool = names(species).len();
        let mut born = 0u32;
        let mut notable: Vec<Event> = Vec::new();
        for i in 0..litter as usize {
            let (x, y) = cells[i % cells.len()];
            let (genome, mutations) = inherit(&mother_genome, &father_genome, generation, gp, rng);
            let sex = if rng.chance(0.5) { Sex::Male } else { Sex::Female };
            let name = rng.below(pool) as NameId;
            let child = Creature {
                id: CreatureId(0),
                species,
                name,
                sex,
                x,
                y,
                born_day: time.day_index() as i32,
                generation,
                parents: Some((mother_id, father_id)),
                genome,
                hp: gp.newborn_hp,
                hunger: 0.3,
                thirst: 0.3,
                energy: 0.8,
                adult: cp.adult_age(species) == 0,
                goal: Goal::Wander,
                target: None,
                replan_at: time.tick,
                trail: Vec::new(),
                alive: true,
                death: None,
                decay: 0.0,
                mutations,
                // Pups know their mother's drinking spot (they follow her anyway).
                last_water: mother_water,
                move_budget: 0.0,
                path: Vec::new(),
                rest_reason: None,
                pregnant_due: None,
                cooldown_until: 0,
                mate_id: None,
                mother: Some(mother_id),
                offspring: 0,
                kills: 0,
                attempts: 0,
                chased: 0,
                escaped: 0,
                threats_by_species: [0; 6],
                kills_by_species: [0; 6],
                last_kill: None,
                chase_stats: (0, 0),
            };
            let id = store.insert(child);
            let child = store.get(id).expect("just inserted");
            lineage.record(child, gp.mutation_notable);
            for mu in &child.mutations {
                if mu.delta.abs() >= gp.mutation_notable {
                    notable.push(Event {
                        year: time.year(),
                        day: time.day_of_year(),
                        hour: time.hour(),
                        kind: EventKind::Mutation,
                        species: Some(species),
                        subject: Some(id),
                        text: format!(
                            "{} {} was born with {} {:+.2} (gen {})",
                            child.name_str(),
                            child.tag(),
                            TRAIT_NAMES[mu.trait_idx],
                            mu.delta,
                            mu.generation
                        ),
                        pos: Some((x, y)),
                        detail: format!("mother {mother_label}"),
                    });
                }
            }
            born += 1;
        }

        if let Some(m) = store.get_mut(mother_id) {
            m.pregnant_due = None;
            m.mate_id = None;
            m.offspring += born;
        }
        if father_id != mother_id {
            if let Some(f) = store.get_mut(father_id) {
                if f.alive {
                    f.offspring += born;
                }
            }
        }
        tallies.births[species.index()] += born;
        born_total += born;

        let father_label = store
            .get(father_id)
            .map(|f| format!("{} {}", f.name_str(), f.tag()))
            .or_else(|| lineage.get(father_id).map(|n| format!("{} {}", n.name_str(), n.tag)))
            .unwrap_or_else(|| "unknown".to_string());
        events.push(Event {
            year: time.year(),
            day: time.day_of_year(),
            hour: time.hour(),
            kind: EventKind::Birth,
            species: Some(species),
            subject: Some(mother_id),
            text: format!(
                "{} bore {} {} in {}",
                mother_label,
                born,
                if born == 1 { "pup" } else { "pups" },
                world.region_name(mx, my)
            ),
            pos: Some((mx, my)),
            detail: format!("father {father_label}; generation {generation}"),
        });
        for e in notable {
            events.push(e);
        }
    }
    born_total
}

/// FR4: while younger than `follow_mother_days` and the mother is alive, a
/// wandering juvenile targets a walkable cell within 3 cells of her.
pub fn follow_target(c: &Creature, view: &TickView, world: &World, time: &Time, gp: &GeneticsParams, rng: &mut Rng) -> Option<(usize, usize)> {
    if c.age_days(time.day_index()) >= gp.follow_mother_days {
        return None;
    }
    let mother = view.get(c.mother?)?;
    let dx = rng.below(7) as i32 - 3;
    let dy = rng.below(7) as i32 - 3;
    let (nx, ny) = (mother.x as i32 + dx, mother.y as i32 + dy);
    if world.in_bounds(nx, ny) && world.cell(nx as usize, ny as usize).terrain.walkable() {
        return Some((nx as usize, ny as usize));
    }
    // Fallback: any walkable cell within 3 of the mother, row-major.
    for ddy in -3i32..=3 {
        for ddx in -3i32..=3 {
            let (px, py) = (mother.x as i32 + ddx, mother.y as i32 + ddy);
            if world.in_bounds(px, py) && world.cell(px as usize, py as usize).terrain.walkable() {
                return Some((px as usize, py as usize));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::params::{Params, WorldParams};
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
        }
    }

    #[test]
    fn mate_eligibility() {
        let w = world();
        let gp = gp();
        let t = time_at(12);
        let mut c = adult(5, 5, Sex::Female);
        assert!(eligible(&c, &t, &w, &gp));
        c.adult = false;
        assert!(!eligible(&c, &t, &w, &gp), "juveniles never mate");
        c.adult = true;
        c.hunger = 0.5;
        assert!(!eligible(&c, &t, &w, &gp), "too hungry");
        c.hunger = 0.1;
        c.cooldown_until = 100;
        assert!(!eligible(&c, &t, &w, &gp), "on cooldown");
        c.cooldown_until = 0;
        // Winter: day index 270+ → tick 270*24.
        assert!(!eligible(&c, &time_at(270 * 24), &w, &gp), "not a breeding season");
        let mut bare = world();
        bare.cell_mut(5, 5).vegetation = 0.0;
        assert!(!eligible(&c, &t, &bare, &gp), "prey need vegetation on the cell");
    }

    #[test]
    fn mating_sets_cooldown_and_pregnancy() {
        let w = world();
        let gp = gp();
        let t = time_at(12);
        let mut store = CreatureStore::new();
        let f = store.insert(adult(5, 5, Sex::Female));
        let m = store.insert(adult(6, 5, Sex::Male));
        let view = TickView::build(&store, &t, &w, &gp);
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
        let cd = 12 + gp.cooldown(SpeciesId::Vole) as u64 * 24;
        assert_eq!(female.cooldown_until, cd);
        assert_eq!(male.cooldown_until, cd);
        assert_eq!(female.pregnant_due, Some(12 + gp.gestation(SpeciesId::Vole) as u64 * 24));
        assert_eq!(female.mate_id, Some(m));
        assert!(male.pregnant_due.is_none());
        assert!(male.mate_id.is_none());
    }

    #[test]
    fn litter_size_from_fertility() {
        let gp = gp();
        assert_eq!(gp.litter_size(SpeciesId::Vole, 0.0), 1);
        assert_eq!(gp.litter_size(SpeciesId::Vole, 0.9), 4);
        assert_eq!(gp.litter_size(SpeciesId::Deer, 0.35), 1);
        assert_eq!(gp.litter_size(SpeciesId::Hare, 0.75), 3);
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
        let born = deliver(&mut store, &w, &mut events, &time_at(10), &gp, &cp, &mut Rng::new(3), &mut tallies, &mut lineage);
        assert_eq!(born, 4);
        assert_eq!(store.len_living(), 6);
        for c in store.living().filter(|c| c.parents.is_some()) {
            assert!(matches!((c.x, c.y), (5, 5) | (6, 5)), "pup at {:?}", (c.x, c.y));
            assert_eq!(c.parents, Some((f, m)));
            assert_eq!(c.mother, Some(f));
            assert_eq!(c.generation, 2);
            assert_eq!(c.hp, gp.newborn_hp);
            assert!(!c.adult);
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
        let mother = Genome([0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]);
        let father = Genome([0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.4, 0.3]);
        let mut sum = [0.0f64; 8];
        let n = 10_000;
        for _ in 0..n {
            let (g, _) = inherit(&mother, &father, 2, &gp, &mut rng);
            for t in 0..8 {
                sum[t] += g.0[t] as f64;
            }
        }
        for t in 0..8 {
            let want = (mother.0[t] + father.0[t]) as f64 / 2.0;
            let got = sum[t] / n as f64;
            assert!((got - want).abs() < 0.005, "trait {t}: mean {got} vs parental mean {want}");
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
        let freq = count as f32 / (n as f32 * 8.0);
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
        for _ in 0..(adult_age as u64 + 1) * 24 {
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
        let view = TickView::build(&store, &t, &w, &gp);
        let target = follow_target(store.get(k).unwrap(), &view, &w, &t, &gp, &mut Rng::new(1)).unwrap();
        assert!(geom::cheb(target.0, target.1, 20, 5) <= 3, "target {:?} not within 3 of the mother", target);
        // Past follow_mother_days: no following.
        let old = time_at(24 * (gp.follow_mother_days as u64 + 1));
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
        let view = TickView::build(&store, &t, &w, &gp);
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
        crate::sim::behavior::needs(&mut a, &w, &t, &cp, &ep, &gp);
        crate::sim::behavior::needs(&mut b, &w, &t, &cp, &ep, &gp);
        let da = a.hunger - 0.1;
        let db = b.hunger - 0.1;
        assert!((db - da * gp.pregnancy_hunger_factor).abs() < 1e-6, "pregnant gain {db} vs {da}×{}", gp.pregnancy_hunger_factor);
    }
}
