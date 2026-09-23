//! C5 FR13 territory: the scent grid, scent avoidance, the challenge and the
//! contest.
//!
//! Adult predators lay scent on the cell under them every tick
//! ([`deposit`], called from the pressure write) and at a kill
//! ([`deposit_kill`]); every mark decays at the day boundary ([`decay`]). A
//! mark carries the id of its holder, which changes hands only while the
//! mark is faint (`hold_min`) or when a contest is won. A **solitary** reader
//! (sociality below the herding gate) treats same-species scent held by
//! anyone else as foreign and steers its patrol and its hunts away from it
//! ([`Scent`]); a social reader shares ground. A resident that sees a
//! same-species adult on ground it holds walks at it (`Goal::Challenge`,
//! [`pick_intruder`]) and the post-loop [`contest_contacts`] pass rolls one
//! contest when they meet: the loser is evicted through the prey flee
//! fields, unchanged, and the winner over-marks the cell.
//!
//! With both mark rates at zero nothing here changes a run: no scent means no
//! foreign ground, no resident, no challenge and no contest roll.

use crate::sim::creatures::{Creature, CreatureId, CreatureStore, DeathTallies, Goal, HuntPhase};
use crate::sim::events::{Event, EventKind, EventRing};
use crate::sim::genetics::TickView;
use crate::sim::geom;
use crate::sim::params::{PredationParams, Roster, SocialParams, TerritoryParams};
use crate::sim::rng::Rng;
use crate::sim::species::SpeciesId;
use crate::sim::time::Time;
use crate::sim::world::{Mark, World};
use super::threat::flee_target;

/// Below the herding gate an animal holds ground for itself alone.
fn solitary(c: &Creature, sp: &SocialParams) -> bool {
    c.sociality() < sp.cohesion_min
}

/// Add `amount` of `depositor`'s scent to a mark; a faint mark changes hands.
fn lay(m: &mut Mark, depositor: CreatureId, amount: f32, tp: &TerritoryParams) {
    if m.strength < tp.hold_min {
        m.holder = depositor;
    }
    m.strength = (m.strength + amount).min(1.0);
}

/// The per-tick deposit: adults of a predator species only (the caller checks
/// the kind). Nothing happens when the rate is zero or the grid is not sized.
pub(super) fn deposit(c: &Creature, world: &mut World, tp: &TerritoryParams) {
    if !c.adult || tp.mark_per_tick <= 0.0 {
        return;
    }
    if let Some(m) = world.mark_mut(c.species, c.x, c.y) {
        lay(m, c.id, tp.mark_per_tick, tp);
    }
}

/// The kill deposit at the kill cell, credited to the killer.
pub(super) fn deposit_kill(species: SpeciesId, killer: CreatureId, x: usize, y: usize, world: &mut World, tp: &TerritoryParams) {
    if tp.kill_mark <= 0.0 {
        return;
    }
    if let Some(m) = world.mark_mut(species, x, y) {
        lay(m, killer, tp.kill_mark, tp);
    }
}

/// The daily decay of every mark; holders stay until re-marked.
pub(super) fn decay(world: &mut World, tp: &TerritoryParams) {
    for m in &mut world.scent {
        m.strength *= tp.decay_per_day;
    }
}

/// What one reader sees of its species' scent this replan: the block to
/// index, who it is, the notice threshold and how much it cares.
/// [`Scent::NONE`] (an empty block) makes every lookup 0.
pub(super) struct Scent<'a> {
    block: &'a [Mark],
    me: CreatureId,
    notice_min: f32,
    /// `TerritoryParams::avoid` for this reader and its hunger.
    pub avoid: f32,
}

impl Scent<'_> {
    /// No scent to read: prey, social animals and the neutral overlay.
    pub(super) const NONE: Scent<'static> = Scent { block: &[], me: CreatureId(0), notice_min: 0.0, avoid: 0.0 };

    /// Foreign strength at cell index `i` (row-major), else 0.
    pub(super) fn foreign_at(&self, i: usize) -> f32 {
        match self.block.get(i) {
            Some(m) if m.holder != self.me && m.strength >= self.notice_min => m.strength,
            _ => 0.0,
        }
    }

    /// Foreign strength at `(x, y)`.
    pub(super) fn foreign(&self, world: &World, x: usize, y: usize) -> f32 {
        self.foreign_at(y * world.width + x)
    }

    /// The patrol-score multiplier for foreign strength `f`: `1 − min(1, avoid × f)`.
    pub(super) fn patrol_factor(&self, f: f32) -> f32 {
        1.0 - (self.avoid * f).min(1.0)
    }

    /// The hunt-score divisor for foreign strength `f`: `1 + avoid × f`.
    pub(super) fn hunt_divisor(&self, f: f32) -> f32 {
        1.0 + self.avoid * f
    }
}

/// Build the reader's scent view: a solitary predator that still cares reads
/// its species' block; everyone else reads nothing.
pub(super) fn scent_view<'a>(c: &Creature, world: &'a World, tp: &TerritoryParams, sp: &SocialParams) -> Scent<'a> {
    if !solitary(c, sp) {
        return Scent::NONE;
    }
    let avoid = tp.avoid(&c.genome, c.hunger);
    if avoid <= 0.0 {
        return Scent::NONE;
    }
    Scent { block: world.scent_block(c.species), me: c.id, notice_min: tp.notice_min, avoid }
}

/// Is `c` resident on `(x, y)`: the holder there, with the mark at `hold_min` or above?
fn holds(c: &Creature, world: &World, x: usize, y: usize, tp: &TerritoryParams) -> bool {
    let m = world.mark(c.species, x, y);
    m.holder == c.id && m.strength >= tp.hold_min
}

/// The nearest same-species adult standing on ground `c` holds, when `c` is a
/// solitary adult resident of its own cell and off cooldown. `peers` is the
/// perception's neighbour list, ids ascending, so ties go to the lower id.
pub(super) fn pick_intruder(c: &Creature, peers: &[CreatureId], view: &TickView, world: &World, time: &Time, tp: &TerritoryParams, sp: &SocialParams) -> Option<(CreatureId, (usize, usize))> {
    if !c.adult || !solitary(c, sp) || time.tick < c.contest_cooldown_until || !holds(c, world, c.x, c.y, tp) {
        return None;
    }
    let mut best: Option<(CreatureId, (usize, usize), f32)> = None;
    for &id in peers {
        if id == c.id || c.mate_id == Some(id) {
            continue;
        }
        let Some(peer) = view.get(id) else { continue };
        if peer.species != c.species || !peer.adult || !holds(c, world, peer.x, peer.y, tp) {
            continue;
        }
        let d = geom::dist(c.x, c.y, peer.x, peer.y);
        if best.is_none_or(|b| d < b.2) {
            best = Some((id, (peer.x, peer.y), d));
        }
    }
    best.map(|(id, pos, _)| (id, pos))
}

/// Keep walking at the intruder while it is alive, in sense range and still on
/// held ground; otherwise the challenge ends and the resident replans.
pub(super) fn update_challenge(c: &mut Creature, view: &TickView, world: &World, time: &Time, tp: &TerritoryParams) {
    let Some(id) = c.challenge_target else {
        end_challenge(c, time);
        return;
    };
    match view.get(id) {
        Some(p) if geom::dist(c.x, c.y, p.x, p.y) <= f32::from(c.sense_cells()) && holds(c, world, p.x, p.y, tp) => {
            c.target = Some((p.x, p.y));
        }
        _ => end_challenge(c, time),
    }
}

const fn end_challenge(c: &mut Creature, time: &Time) {
    c.challenge_target = None;
    c.challenge_until = 0;
    c.goal = Goal::Patrol;
    c.target = None;
    c.replan_at = time.tick;
}

/// The predator sibling of `preempt_prey`: an evicted predator keeps fleeing
/// the winner for `evict_ticks`; when the timer lapses, or the flee ended
/// early on distance, the eviction is over and it replans.
pub(super) fn preempt_predator(c: &mut Creature, world: &World, time: &Time, pp: &PredationParams) {
    if c.goal == Goal::Flee {
        if c.threatened_by.is_none() || time.tick >= c.flee_until {
            c.threatened_by = None;
            c.flee_until = 0;
            c.goal = Goal::Patrol;
            c.target = None;
            c.replan_at = time.tick;
        } else {
            c.target = flee_target(c, world, pp);
            c.replan_at = time.tick + 1;
        }
    } else if c.threatened_by.is_some() {
        c.threatened_by = None;
        c.flee_until = 0;
    }
}

/// The pair as it stands when a challenger is adjacent to its target.
struct Meeting {
    resident: CreatureId,
    intruder: CreatureId,
    p_resident: f32,
    species: SpeciesId,
}

fn meeting(store: &CreatureStore, cid: CreatureId, tid: CreatureId, tp: &TerritoryParams) -> Result<Option<Meeting>, ()> {
    let (Some(c), Some(t)) = (store.get(cid), store.get(tid)) else { return Err(()) };
    if !(c.alive && t.alive && c.goal == Goal::Challenge && c.challenge_target == Some(tid) && t.species == c.species) {
        return Err(());
    }
    if geom::cheb(c.x, c.y, t.x, t.y) > 1 {
        return Ok(None);
    }
    Ok(Some(Meeting { resident: cid, intruder: tid, p_resident: tp.resident_wins(&c.genome, &t.genome), species: c.species }))
}

/// Post-loop pass: one contest roll per challenger adjacent to its target, in
/// ascending challenger id. Both pay energy; the loser is injured (never below
/// `hp_floor`) and evicted; the winner over-marks the cell.
pub(super) fn contest_contacts(store: &mut CreatureStore, world: &mut World, events: &mut EventRing, time: &Time, roster: &Roster, pp: &PredationParams, tp: &TerritoryParams, rng: &mut Rng, tallies: &mut DeathTallies) {
    let mut challengers: Vec<(CreatureId, CreatureId)> = store
        .living()
        .filter(|c| c.goal == Goal::Challenge)
        .filter_map(|c| c.challenge_target.map(|t| (c.id, t)))
        .collect();
    challengers.sort_unstable();
    for (cid, tid) in challengers {
        let m = match meeting(store, cid, tid, tp) {
            Ok(Some(m)) => m,
            Ok(None) => continue,
            Err(()) => {
                if let Some(c) = store.get_mut(cid) {
                    if c.goal == Goal::Challenge {
                        end_challenge(c, time);
                    }
                }
                continue;
            }
        };
        let resident_wins = rng.chance(m.p_resident);
        let (winner, loser) = if resident_wins { (m.resident, m.intruder) } else { (m.intruder, m.resident) };
        tallies.contests[m.species.index()] += 1;
        resolve_contest(store, world, events, time, roster, pp, tp, m.species, winner, loser);
    }
}

/// Apply one decided contest.
fn resolve_contest(store: &mut CreatureStore, world: &mut World, events: &mut EventRing, time: &Time, roster: &Roster, pp: &PredationParams, tp: &TerritoryParams, species: SpeciesId, winner: CreatureId, loser: CreatureId) {
    let cooldown = time.tick + u64::from(tp.contest_cooldown_days) * u64::from(time.ticks_per_day);
    let Some(w) = store.get_mut(winner) else { return };
    let winner_at = (w.x, w.y, w.species);
    let winner_size = w.genome.size();
    let winner_label = w.label(roster);
    w.energy -= tp.contest_energy;
    w.contests_won = w.contests_won.saturating_add(1);
    w.contest_cooldown_until = cooldown;
    if w.goal == Goal::Challenge {
        end_challenge(w, time);
        w.replan_at = time.tick + 1;
    }
    let Some(l) = store.get_mut(loser) else { return };
    let loser_at = (l.x, l.y);
    let loser_label = l.label(roster);
    l.energy -= tp.contest_energy;
    if l.hp > tp.hp_floor {
        l.hp = (l.hp - tp.contest_injury * winner_size).max(tp.hp_floor);
    }
    l.contests_lost = l.contests_lost.saturating_add(1);
    l.contest_cooldown_until = cooldown;
    l.challenge_target = None;
    l.challenge_until = 0;
    l.hunt_phase = HuntPhase::Stalk;
    l.hunt_target = None;
    l.chase_start_tick = None;
    l.eat_until = None;
    l.scavenge_target = None;
    l.threatened_by = Some(winner_at);
    l.flee_until = time.tick + u64::from(tp.evict_ticks);
    l.goal = Goal::Flee;
    l.rest_reason = None;
    l.target = flee_target(l, world, pp);
    l.replan_at = time.tick + 1;
    if let Some(m) = world.mark_mut(species, loser_at.0, loser_at.1) {
        m.strength = 1.0;
        m.holder = winner;
    }
    let region = world.region_index(loser_at.0, loser_at.1);
    events.push(Event {
        year: time.year(),
        day: time.day_of_year(),
        hour: time.hour(),
        kind: EventKind::Contest,
        species: Some(species),
        subject: Some(winner),
        text: format!("{winner_label} drove {loser_label} off {}", world.place_name(loser_at.0, loser_at.1)),
        pos: Some(loser_at),
        detail: format!("{}:{}:{region}", winner.0, loser.0),
    });
}
