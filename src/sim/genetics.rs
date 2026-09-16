//! Reproduction, inheritance and maturity (C4): mate eligibility and selection,
//! consummation, pregnancy, litters, inheritance with mutation and the
//!
//! per-tick peer snapshot that lets a creature see potential partners and its
//! mother while the store is being iterated mutably.

use crate::sim::creatures::{
    adult_age_days, Creature, CreatureId, CreatureStore, DeathTallies, Goal, HuntPhase, Mutation, NameId, Sex,
};
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::geom;
use crate::sim::lineage::Lineage;
use crate::sim::params::{GeneticsParams, Roster};
use crate::sim::rng::Rng;
use crate::sim::disease::{self, DiseaseState};
use crate::sim::params::DiseaseParams;
use crate::sim::species::{Genome, Kind, SpeciesId, TRAIT_NAMES};
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
    /// Predation fields (C5): camouflage for the hiding rule and current goal
    /// for den protection / prey detection.
    pub camouflage: f32,
    pub goal: Goal,
    /// C8 FR4: the prey this predator is currently hunting, so a packmate can see
    /// the pack's shared target.
    pub hunt_target: Option<CreatureId>,
}

/// A dead-but-not-freed prey carcass, for scavenging (C5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Carcass {
    pub id: CreatureId,
    pub species: SpeciesId,
    pub x: usize,
    pub y: usize,
    pub decay: f32,
}

/// Per-tick snapshot of every living creature, sorted by id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TickView {
    peers: Vec<Peer>,
    pub total: usize,
    /// `total < max_population_soft_cap`: new pregnancies allowed.
    pub cap_ok: bool,
    /// Prey carcasses in range for scavenging (C5), slot order.
    pub carcasses: Vec<Carcass>,
}

impl TickView {
    pub const fn empty() -> Self {
        Self { peers: Vec::new(), total: 0, cap_ok: true, carcasses: Vec::new() }
    }

    pub fn build(store: &CreatureStore, time: &Time, world: &World, roster: &Roster, gp: &GeneticsParams, dp: &DiseaseParams) -> Self {
        let mut peers: Vec<Peer> = store
            .living()
            .map(|c| Peer {
                id: c.id,
                species: c.species,
                sex: c.sex,
                x: c.x,
                y: c.y,
                adult: c.adult,
                mate_ready: eligible(c, time, world, roster, gp, dp),
                camouflage: c.genome.camouflage(),
                goal: c.goal,
                hunt_target: if c.goal == Goal::Hunt && c.hunt_phase != HuntPhase::Eat { c.hunt_target } else { None },
            })
            .collect();
        peers.sort_unstable_by_key(|p| p.id);
        let total = peers.len();
        let carcasses: Vec<Carcass> = store
            .carcasses()
            .map(|c| Carcass { id: c.id, species: c.species, x: c.x, y: c.y, decay: c.decay })
            .collect();
        Self { peers, total, cap_ok: (crate::cast!(total => u32)) < gp.max_population_soft_cap, carcasses }
    }

    pub fn get(&self, id: CreatureId) -> Option<&Peer> {
        self.peers.binary_search_by_key(&id, |p| p.id).ok().map(|i| &self.peers[i])
    }
}

/// FR2 mate eligibility: adult, fed, watered, rested, off cooldown, in a
/// breeding season and (prey only) standing on vegetation ≥ the minimum.
pub fn eligible(c: &Creature, time: &Time, world: &World, roster: &Roster, gp: &GeneticsParams, dp: &DiseaseParams) -> bool {
    if !c.alive || !c.adult || c.pregnant_due.is_some() {
        return false;
    }
    // C7 FR6: the infectious do not mate.
    if !disease::effects(c, dp).can_mate {
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
    if roster.kind(c.species) == Kind::Prey && world.cell(c.x, c.y).vegetation < gp.mate_cell_vegetation_min {
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
    let mut g = [0.0f32; Genome::LEN];
    let mut mutations = Vec::new();
    for t in 0..Genome::LEN {
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
pub fn consummate(store: &mut CreatureStore, time: &Time, roster: &Roster, gp: &GeneticsParams, events: &mut EventRing, view: &TickView, soft_cap_noted: &mut bool) {
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
        let sp = roster.get(species);
        let cooldown = time.tick + u64::from(sp.mate_cooldown_days) * u64::from(time.ticks_per_day);
        let due = time.tick + u64::from(sp.gestation_days) * u64::from(time.ticks_per_day);

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
/// Build one newborn creature (all per-birth state at its defaults).
#[allow(clippy::too_many_arguments)]
fn newborn(
    species: SpeciesId,
    n_species: usize,
    name: NameId,
    sex: Sex,
    pos: (usize, usize),
    born_day: i32,
    generation: u32,
    parents: (CreatureId, CreatureId),
    mother_id: CreatureId,
    genome: Genome,
    mutations: Vec<Mutation>,
    mother_water: Option<(usize, usize)>,
    born_tick: u64,
    hp: f32,
    adult: bool,
) -> Creature {
    let (x, y) = pos;
    Creature {
                id: CreatureId(0),
                species,
                name,
                sex,
                x,
                y,
                born_day,
                generation,
                parents: Some(parents),
                genome,
                hp,
                hunger: 0.3,
                thirst: 0.3,
                energy: 0.8,
                adult,
                goal: Goal::Wander,
                target: None,
                replan_at: born_tick,
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
                threats_by_species: vec![0; n_species],
                kills_by_species: vec![0; n_species],
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

pub fn deliver(
    store: &mut CreatureStore,
    world: &World,
    events: &mut EventRing,
    time: &Time,
    roster: &Roster,
    gp: &GeneticsParams,
    dp: &DiseaseParams,
    rng: &mut Rng,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
    dstate: &DiseaseState,
    drng: &mut Rng,
) -> u32 {
    let due: Vec<CreatureId> = store
        .living()
        .filter(|c| c.pregnant_due.is_some_and(|d| d <= time.tick))
        .map(|c| c.id)
        .collect();
    let mut born_total = 0;
    for mother_id in due {
        born_total += deliver_litter(store, world, events, time, roster, gp, dp, rng, tallies, lineage, dstate, drng, mother_id);
    }
    born_total
}

/// Everything a single pup needs from its parents.
struct LitterCtx<'a> {
    species: SpeciesId,
    mother_id: CreatureId,
    father_id: CreatureId,
    mother_genome: Genome,
    father_genome: Genome,
    generation: u32,
    mother_water: Option<(usize, usize)>,
    mother_label: &'a str,
}

/// Deliver one mother's litter and return how many pups survived to birth.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn deliver_litter(
    store: &mut CreatureStore,
    world: &World,
    events: &mut EventRing,
    time: &Time,
    roster: &Roster,
    gp: &GeneticsParams,
    dp: &DiseaseParams,
    rng: &mut Rng,
    tallies: &mut DeathTallies,
    lineage: &mut Lineage,
    dstate: &DiseaseState,
    drng: &mut Rng,
    mother_id: CreatureId,
) -> u32 {
    let Some(m) = store.get(mother_id) else { return 0 };
    let species = m.species;
    let (mx, my) = (m.x, m.y);
    let mother_genome = m.genome;
    let mother_gen = m.generation;
    let father_id = m.mate_id.unwrap_or(mother_id);
    // C7 FR6: parasites lower the effective fertility; C8 maturity scales the
    // litter (a slow life history has fewer, larger litters).
    let litter = gp.litter_size(roster.get(species).litter_max, m.genome.fertility() * disease::effects(m, dp).fertility_factor, m.genome.maturity());
    let mother_label = m.label(roster);
    let mother_water = m.last_water;
    let mother_snapshot = m.clone();

    let (father_genome, father_gen) = parent_genome(store, lineage, father_id, mother_genome, mother_gen);
    let generation = mother_gen.max(father_gen) + 1;
    let ctx = LitterCtx {
        species,
        mother_id,
        father_id,
        mother_genome,
        father_genome,
        generation,
        mother_water,
        mother_label: &mother_label,
    };

    let cells = litter_cells(world, mx, my);
    let mut born = 0u32;
    let mut notable: Vec<Event> = Vec::new();
    for i in 0..crate::cast!(litter => usize) {
        let pos = cells[i % cells.len()];
        if bear_pup(store, lineage, rng, time, roster, gp, dp, dstate, drng, &ctx, pos, &mother_snapshot, &mut notable) {
            born += 1;
        }
    }

    credit_parents(store, tallies, mother_id, father_id, born, species);
    announce_litter(world, events, time, roster, store, lineage, &ctx, born, (mx, my));
    for e in notable {
        events.push(e);
    }
    born
}

/// The father's genome and generation, from the store or the lineage.
fn parent_genome(
    store: &CreatureStore,
    lineage: &Lineage,
    father_id: CreatureId,
    fallback_genome: Genome,
    fallback_gen: u32,
) -> (Genome, u32) {
    match store.get(father_id) {
        Some(f) => (f.genome, f.generation),
        None => match lineage.get(father_id) {
            Some(n) => (n.genome, n.generation),
            None => (fallback_genome, fallback_gen),
        },
    }
}

/// The mother's cell plus the walkable 8-adjacent cells a pup may occupy.
fn litter_cells(world: &World, mx: usize, my: usize) -> Vec<(usize, usize)> {
    let mut cells: Vec<(usize, usize)> = vec![(mx, my)];
    for &(dx, dy) in &OFF8 {
        let (nx, ny) = (crate::cast!(mx => i32) + dx, crate::cast!(my => i32) + dy);
        if world.in_bounds(nx, ny) {
            let t = world.cell(crate::cast!(nx => usize), crate::cast!(ny => usize)).terrain;
            if t.walkable() && !t.is_water() {
                cells.push((crate::cast!(nx => usize), crate::cast!(ny => usize)));
            }
        }
    }
    cells
}

/// Create one pup; push any notable-mutation event and report whether it lived.
#[allow(clippy::too_many_arguments)]
fn bear_pup(
    store: &mut CreatureStore,
    lineage: &mut Lineage,
    rng: &mut Rng,
    time: &Time,
    roster: &Roster,
    gp: &GeneticsParams,
    dp: &DiseaseParams,
    dstate: &DiseaseState,
    drng: &mut Rng,
    ctx: &LitterCtx<'_>,
    pos: (usize, usize),
    mother_snapshot: &Creature,
    notable: &mut Vec<Event>,
) -> bool {
    let (x, y) = pos;
    let (genome, mutations) = inherit(&ctx.mother_genome, &ctx.father_genome, ctx.generation, gp, rng);
    let sex = if rng.chance(0.5) { Sex::Male } else { Sex::Female };
    let sp = roster.get(ctx.species);
    let pool = sp.name_pool_len();
    let name = crate::cast!(rng.below(pool) => NameId);
    let adult = adult_age_days(sp, &genome, gp) == 0;
    let mut child = newborn(
        ctx.species, roster.len(), name, sex, (x, y), crate::cast!(time.day_index() => i32), ctx.generation,
        (ctx.mother_id, ctx.father_id), ctx.mother_id, genome, mutations, ctx.mother_water, time.tick, gp.newborn_hp, adult,
    );
    disease::at_birth(&mut child, mother_snapshot, time, dp, dstate, drng);
    let id = store.insert(child);
    let Some(child) = store.get(id) else { return false };
    lineage.record(child, roster, gp.mutation_notable);
    for mu in &child.mutations {
        if mu.delta.abs() >= gp.mutation_notable {
            notable.push(Event {
                year: time.year(),
                day: time.day_of_year(),
                hour: time.hour(),
                kind: EventKind::Mutation,
                species: Some(ctx.species),
                subject: Some(id),
                text: format!(
                    "{} was born with {} {:+.2} (gen {})",
                    child.label(roster),
                    TRAIT_NAMES[mu.trait_idx],
                    mu.delta,
                    mu.generation
                ),
                pos: Some((x, y)),
                detail: format!("mother {}", ctx.mother_label),
            });
        }
    }
    true
}

/// Clear the pregnancy and credit both parents with the litter.
fn credit_parents(
    store: &mut CreatureStore,
    tallies: &mut DeathTallies,
    mother_id: CreatureId,
    father_id: CreatureId,
    born: u32,
    species: SpeciesId,
) {
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
}

/// Emit the birth event for the litter.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn announce_litter(
    world: &World,
    events: &mut EventRing,
    time: &Time,
    roster: &Roster,
    store: &CreatureStore,
    lineage: &Lineage,
    ctx: &LitterCtx<'_>,
    born: u32,
    (mx, my): (usize, usize),
) {
    let father_label = store
        .get(ctx.father_id)
        .map(|f| f.label(roster))
        .or_else(|| lineage.get(ctx.father_id).map(|n| format!("{} {}", n.name_str(roster), n.tag)))
        .unwrap_or_else(|| "unknown".to_string());
    events.push(Event {
        year: time.year(),
        day: time.day_of_year(),
        hour: time.hour(),
        kind: EventKind::Birth,
        species: Some(ctx.species),
        subject: Some(ctx.mother_id),
        text: format!(
            "{} bore {} {} in {}",
            ctx.mother_label,
            born,
            if born == 1 { "pup" } else { "pups" },
            world.region_name(mx, my)
        ),
        pos: Some((mx, my)),
        detail: format!("father {father_label}; generation {}", ctx.generation),
    });
}

/// FR4: while younger than `follow_mother_days` and the mother is alive, a
/// wandering juvenile targets a walkable cell within 3 cells of her.
pub fn follow_target(c: &Creature, view: &TickView, world: &World, time: &Time, gp: &GeneticsParams, rng: &mut Rng) -> Option<(usize, usize)> {
    if c.age_days(time.day_index()) >= gp.follow_mother_days {
        return None;
    }
    let mother = view.get(c.mother?)?;
    let dx = crate::cast!(rng.below(7) => i32) - 3;
    let dy = crate::cast!(rng.below(7) => i32) - 3;
    let (nx, ny) = (crate::cast!(mother.x => i32) + dx, crate::cast!(mother.y => i32) + dy);
    if world.in_bounds(nx, ny) && world.cell(crate::cast!(nx => usize), crate::cast!(ny => usize)).terrain.walkable() {
        return Some((crate::cast!(nx => usize), crate::cast!(ny => usize)));
    }
    // Fallback: any walkable cell within 3 of the mother, row-major.
    for ddy in -3i32..=3 {
        for ddx in -3i32..=3 {
            let (px, py) = (crate::cast!(mother.x => i32) + ddx, crate::cast!(mother.y => i32) + ddy);
            if world.in_bounds(px, py) && world.cell(crate::cast!(px => usize), crate::cast!(py => usize)).terrain.walkable() {
                return Some((crate::cast!(px => usize), crate::cast!(py => usize)));
            }
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests;
