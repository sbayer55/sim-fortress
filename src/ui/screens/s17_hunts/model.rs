//! The facts one lane shows, read from the sim once per render: the hunter,
//! its prey, its trace, and the derived state, odds and verdict words. The
//! tug value and the thresholds are UI estimates; the spec names them so.

use ratatui::style::Color;

use crate::sim::creatures::{Creature, CreatureId, Goal, HuntPhase};
use crate::sim::hunt_watch::{HuntOutcome, HuntTrace};
use crate::sim::params::{CreaturesParams, PredationParams};
use crate::sim::predation::{self, OddsParts};
use crate::sim::world::Terrain;
use crate::sim::{Kind, Sim};
use crate::theme;
use crate::ui::screens::s16_dynasties::Pin;

use super::{DEAD_LINGER, FED_LINGER};

/// What the hunter is doing, derived from its creature and its trace (spec item 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Status {
    Hunting,
    Eating,
    Fed,
    /// Fed and past the linger; shown only while pinned.
    Sated,
    Cooldown,
    Resting,
    Patrol,
    Dead,
}

/// One lane's facts.
pub(super) struct HuntView<'a> {
    pub sim: &'a Sim,
    pub hunter: &'a Creature,
    pub prey: Option<&'a Creature>,
    pub trace: &'a HuntTrace,
    pub status: Status,
    /// Hours left while eating, fed or cooling down; ticks left on the clock while chasing.
    pub left: u64,
    pub pinned: bool,
    pub odds: Option<OddsParts>,
    pub packmates: Vec<CreatureId>,
}

impl<'a> HuntView<'a> {
    pub(super) fn build(sim: &'a Sim, trace: &'a HuntTrace, pins: &[Pin]) -> Option<Self> {
        let now = sim.time.tick;
        let hunter = sim.creatures.get(trace.hunter())?;
        let prey = sim.creatures.get(trace.prey());
        let (status, left) = status_of(hunter, trace, now, &sim.params.predation);
        let hunting = status == Status::Hunting;
        let odds = hunting.then(|| predation::kill_odds(sim, hunter.id, trace.prey())).flatten();
        let packmates = match (hunting, prey) {
            (true, Some(q)) => {
                let pair = predation::HuntPair { pred: hunter.id, pred_species: hunter.species, prey: q.id, prey_at: (q.x, q.y) };
                predation::pack_participants(&sim.creatures, pair, &sim.params.social)
            }
            _ => Vec::new(),
        };
        Some(Self { sim, hunter, prey, trace, status, left, pinned: is_pinned(pins, hunter.id), odds, packmates })
    }

    pub(super) const fn id(&self) -> CreatureId {
        self.hunter.id
    }

    /// Whether the lane is shown: dead or sated hunters stay only while pinned (spec item 1).
    pub(super) fn on_board(&self, now: u64) -> bool {
        match self.status {
            Status::Sated => self.pinned,
            Status::Dead => self.pinned || self.trace.died().is_none_or(|d| now.saturating_sub(d.tick) < DEAD_LINGER),
            _ => true,
        }
    }

    pub(super) const fn hunting(&self) -> bool {
        matches!(self.status, Status::Hunting)
    }

    /// The gap this tick: the live Chebyshev distance while hunting, else the last sample's.
    pub(super) fn gap(&self) -> Option<u8> {
        match (self.hunting(), self.prey) {
            (true, Some(q)) => Some(crate::cast!(crate::sim::cheb(self.hunter.x, self.hunter.y, q.x, q.y).min(255) => u8)),
            _ => self.trace.last_sample().map(|s| s.gap),
        }
    }

    /// Ticks on the chase clock so far, while chasing.
    pub(super) fn chase_ticks(&self) -> Option<u64> {
        (self.hunting() && self.hunter.hunt_phase == HuntPhase::Chase).then(|| self.trace.chase().map(|c| self.sim.time.tick.saturating_sub(c.tick))).flatten()
    }

    pub(super) fn region(&self) -> &str {
        self.sim.world.region_name(self.hunter.x, self.hunter.y)
    }

    /// STALK / CHASE while hunting, the outcome word on the resolving tick, else the status word.
    pub(super) fn state_word(&self) -> (String, Color) {
        let now = self.sim.time.tick;
        if let Some(end) = self.trace.end().filter(|e| e.tick == now) {
            return outcome_word(end.outcome);
        }
        match self.status {
            Status::Hunting => match self.hunter.hunt_phase {
                HuntPhase::Chase => ("CHASE".into(), theme::WARN),
                _ => ("STALK".into(), theme::INFO),
            },
            Status::Eating => ("EATING".into(), theme::BAD),
            Status::Fed => (format!("FED {}h", self.left), theme::GOOD),
            Status::Sated => ("SATED".into(), theme::GOOD),
            Status::Cooldown => (format!("COOLDOWN {}h", self.left), theme::DIM),
            Status::Resting => ("RESTING".into(), theme::INFO),
            Status::Patrol => ("PATROL".into(), theme::TEXT),
            Status::Dead => {
                let cause = self.trace.died().map_or(crate::sim::Cause::Starved, |d| d.cause);
                (cause.label().to_uppercase(), theme::BAD)
            }
        }
    }

    /// The sentence written across a remembered ribbon (spec item 11).
    pub(super) fn idle_sentence(&self) -> String {
        let name = self.hunter.name_str(self.sim.roster());
        let prey = self.prey.map_or_else(|| "its prey".to_string(), |q| q.label(self.sim.roster()));
        match self.status {
            Status::Hunting => String::new(),
            Status::Eating => format!("feeding on {prey} for {} more hours", self.left),
            Status::Fed => format!("fed on {}; leaves the board in {}", self.prey.map_or("its prey", |q| q.name_str(self.sim.roster())), self.left),
            Status::Sated => format!("sated, hunger {}% and rising", pct(self.hunter.hunger)),
            Status::Cooldown => format!("hunt cooldown: {} h before {name} can hunt again", self.left),
            Status::Resting => format!("resting: energy {}% and rising, hunger {}% and rising", pct(self.hunter.energy), pct(self.hunter.hunger)),
            Status::Patrol => format!("patrolling {} for prey", self.region()),
            Status::Dead => {
                let cause = self.trace.died().map_or("died", |d| d.cause.label());
                format!("{name} {cause} {}", self.trace.died().map_or_else(String::new, |d| hour_label(self.sim, d.tick)))
            }
        }
    }

    /// The stalk and chase values the ribbon plots.
    pub(super) fn ribbon_values(&self) -> (Vec<f32>, Vec<f32>) {
        let pp = &self.sim.params.predation;
        let sense = self.hunter.genome.sense_cells();
        let chase = self.trace.chase();
        let (mut stalk, mut run) = (Vec::new(), Vec::new());
        for s in self.trace.samples() {
            let chase_tick = chase.filter(|c| s.tick >= c.tick).map(|c| s.tick - c.tick);
            let p = tug(s.gap, sense, chase_tick, pp.chase_max_ticks, s.hunter_energy, chase.map(|c| c.hunter_energy));
            if chase_tick.is_some() {
                run.push(p);
            } else {
                stalk.push(p);
            }
        }
        (stalk, run)
    }

    /// The three escape routes of the latest sample, each in 0..=1 (spec item 9).
    pub(super) fn escape_routes(&self) -> Option<[f32; 3]> {
        let s = self.trace.last_sample()?;
        let pp = &self.sim.params.predation;
        let chase = self.trace.chase();
        let chase_tick = chase.filter(|c| s.tick >= c.tick).map(|c| s.tick - c.tick);
        Some(routes(s.gap, self.hunter.genome.sense_cells(), chase_tick, pp.chase_max_ticks, s.hunter_energy, chase.map(|c| c.hunter_energy)))
    }
}

/// Every trace on the board, in trace order (by hunter id).
pub(super) fn collect<'a>(sim: &'a Sim, pins: &[Pin]) -> Vec<HuntView<'a>> {
    let now = sim.time.tick;
    sim.hunts.traces().filter_map(|t| HuntView::build(sim, t, pins)).filter(|v| v.on_board(now)).collect()
}

pub(super) fn is_pinned(pins: &[Pin], id: CreatureId) -> bool {
    pins.contains(&Pin::Member(id))
}

/// Status and its countdown (spec item 3, plan D10).
fn status_of(c: &Creature, trace: &HuntTrace, now: u64, pp: &PredationParams) -> (Status, u64) {
    if !c.alive || trace.died().is_some() {
        return (Status::Dead, 0);
    }
    if trace.is_open() {
        let left = trace.chase().map_or(0, |ch| u64::from(pp.chase_max_ticks).saturating_sub(now.saturating_sub(ch.tick)));
        return (Status::Hunting, left);
    }
    match trace.end().map(|e| e.outcome) {
        Some(HuntOutcome::Kill { eat_until }) => {
            if now < eat_until {
                (Status::Eating, eat_until - now)
            } else if now < eat_until + FED_LINGER {
                (Status::Fed, eat_until + FED_LINGER - now)
            } else {
                (Status::Sated, 0)
            }
        }
        _ if now < c.hunt_cooldown_until => (Status::Cooldown, c.hunt_cooldown_until - now),
        _ if c.goal == Goal::Rest => (Status::Resting, 0),
        _ => (Status::Patrol, 0),
    }
}

fn outcome_word(o: HuntOutcome) -> (String, Color) {
    match o {
        HuntOutcome::Kill { .. } => ("KILL".into(), theme::BAD),
        HuntOutcome::Miss => ("MISSED".into(), theme::GOOD),
        HuntOutcome::Timeout => ("TIMED OUT".into(), theme::GOOD),
        HuntOutcome::Lost => ("LOST".into(), theme::GOOD),
        HuntOutcome::Dropped => ("DROPPED".into(), theme::DIM),
    }
}

/// The three escape routes: the gap toward the sense edge, the clock, the hunter's legs.
fn routes(gap: u8, sense: u16, chase_tick: Option<u64>, chase_max: u32, energy: f32, energy0: Option<f32>) -> [f32; 3] {
    let span = f32::from(sense.max(2) - 1);
    let gap01 = ((f32::from(gap) - 1.0) / span).clamp(0.0, 1.0);
    let clock01 = chase_tick.map_or(0.0, |t| (crate::cast!(t => f32) / crate::cast!(chase_max.max(1) => f32)).clamp(0.0, 1.0));
    let legs01 = energy0.map_or(0.0, |e0| (1.0 - (energy - 0.05) / (e0 - 0.05).max(0.01)).clamp(0.0, 1.0));
    [gap01, clock01, legs01]
}

/// Who is winning, 0 = the prey is getting away, 1 = the kill is on (spec item 9).
pub(super) fn tug(gap: u8, sense: u16, chase_tick: Option<u64>, chase_max: u32, energy: f32, energy0: Option<f32>) -> f32 {
    let [gap01, clock01, legs01] = routes(gap, sense, chase_tick, chase_max, energy, energy0);
    let k = if gap <= 1 { 1.0 } else { 1.0 - gap01 };
    let e = gap01.max(clock01).max(legs01);
    (k - e).mul_add(0.5, 0.5).clamp(0.0, 1.0)
}

/// The hunter's need from its hunger (spec item 4).
pub(super) fn need_word(hunger: f32, hunt_hunger_min: f32) -> (&'static str, Color) {
    if hunger >= 0.8 {
        ("DESPERATE", theme::BAD)
    } else if hunger >= 0.6 {
        ("HUNGRY", theme::WARN)
    } else if hunger >= hunt_hunger_min {
        ("IN NEED", theme::TEXT)
    } else {
        ("NOT HUNGRY", theme::DIM)
    }
}

/// How much of a challenge the prey is, from the kill odds, read from the hunter's side (spec item 5).
pub(super) fn challenge_word(odds: f32) -> (&'static str, Color) {
    if odds >= 0.7 {
        ("EASY PREY", theme::GOOD)
    } else if odds >= 0.5 {
        ("FAIR CHASE", theme::TEXT)
    } else if odds >= 0.35 {
        ("CHALLENGE", theme::WARN)
    } else {
        ("LONG SHOT", theme::BAD)
    }
}

/// Energy a hunter spends per chase tick: a UI estimate from the movement costs.
pub(super) fn exertion(cp: &CreaturesParams) -> f32 {
    cp.move_cost_energy.mul_add(1.5, cp.energy_awake_per_hour).max(0.001)
}

/// Ticks of chase the hunter's energy still buys.
pub(super) fn reserves(energy: f32, cp: &CreaturesParams) -> u32 {
    crate::cast!(((energy - 0.05).max(0.0) / exertion(cp)).floor() => u32)
}

/// Ticks of flight the prey's energy still buys (a fleeing prey pays `flee_energy_factor`).
pub(super) fn stamina(energy: f32, cp: &CreaturesParams, pp: &PredationParams) -> u32 {
    crate::cast!(((energy - 0.05).max(0.0) / (exertion(cp) * pp.flee_energy_factor.max(1.0))).floor() => u32)
}

pub(super) fn pct(v: f32) -> u32 {
    crate::cast!((v.clamp(0.0, 1.0) * 100.0).round() => u32)
}

/// `.78` from 0.78; `1.0` at the top.
pub(super) fn frac(v: f32) -> String {
    if v >= 0.995 {
        "1.0".to_string()
    } else {
        format!(".{:02}", crate::cast!((v.clamp(0.0, 1.0) * 100.0).round() => u32) % 100)
    }
}

pub(super) const fn terrain_word(t: Terrain) -> &'static str {
    match t {
        Terrain::DeepWater | Terrain::ShallowWater => "water",
        Terrain::Sand => "sand",
        Terrain::Dirt => "dirt",
        Terrain::GrassSparse => "sparse grass",
        Terrain::Grass => "grass",
        Terrain::GrassDense => "dense grass",
        Terrain::Forest => "forest",
        Terrain::Rock => "rock",
        Terrain::Marsh => "marsh",
    }
}

/// `hh:00` of an earlier tick, from the current clock.
pub(super) fn hour_label(sim: &Sim, tick: u64) -> String {
    let per_day = i64::from(sim.time.ticks_per_day.max(1));
    let back = crate::cast!(sim.time.tick.saturating_sub(tick).min(u64::from(u32::MAX)) => i64);
    let hour = (i64::from(sim.time.hour()) - back).rem_euclid(per_day);
    format!("{hour:02}:00")
}

/// Living prey the hunter could detect within its sense range (spec item 13).
pub(super) fn other_prey_in_range(sim: &Sim, hunter: &Creature) -> usize {
    let roster = sim.roster();
    sim.spatial
        .within(hunter.x, hunter.y, hunter.genome.sense_cells())
        .into_iter()
        .filter_map(|id| sim.creatures.get(id))
        .filter(|q| q.alive && q.id != hunter.id && roster.kind(q.species) == Kind::Prey && predation::can_detect(hunter, q, &sim.world, &sim.params.predation))
        .count()
}
