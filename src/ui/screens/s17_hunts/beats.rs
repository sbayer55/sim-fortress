//! The play-by-play, narrated from the trace's samples (spec item 16). No
//! event kind is read or written: every line is a delta between two samples,
//! the phase flip, the prey's goal turning to Flee or Wary, or the outcome.

use ratatui::style::Color;

use crate::sim::creatures::{Goal, HuntPhase};
use crate::sim::hunt_watch::{HuntOutcome, HuntSample};
use crate::sim::params::PredationParams;
use crate::theme;

use super::model::{terrain_word, HuntView, Status};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Calm,
    Tense,
    Hot,
    Kill,
    Escape,
    End,
}

impl Kind {
    pub(super) const fn color(self) -> Color {
        match self {
            Self::Calm => theme::TEXT,
            Self::Tense => theme::WARN,
            Self::Hot => theme::BAD,
            Self::Kill => theme::TEXT_BRIGHT,
            Self::Escape => theme::GOOD,
            Self::End => theme::DIM,
        }
    }

    pub(super) const fn bold(self) -> bool {
        matches!(self, Self::Hot | Self::Kill | Self::Escape)
    }
}

#[derive(Clone, Debug)]
pub(super) struct Beat {
    pub tick: u64,
    pub text: String,
    pub kind: Kind,
}

/// The names and params every beat needs.
struct Ctx<'a> {
    hunter: String,
    prey: String,
    region: &'a str,
    pp: &'a PredationParams,
    /// The tick the chase clock started, when it has.
    clock_from: Option<u64>,
}

impl Ctx<'_> {
    /// Ticks on the clock at `s`, once the clock runs.
    fn clock_at(&self, s: &HuntSample) -> Option<u64> {
        self.clock_from.filter(|&c| s.tick >= c).map(|c| s.tick - c)
    }
}

const fn noticed(g: Goal) -> bool {
    matches!(g, Goal::Flee | Goal::Wary)
}

/// The beats of the lane's current or last hunt up to now, oldest first.
pub(super) fn beats(v: &HuntView<'_>) -> Vec<Beat> {
    let roster = v.sim.roster();
    let ctx = Ctx {
        hunter: v.hunter.name_str(roster).to_string(),
        prey: v.prey.map_or_else(|| "the prey".to_string(), |q| q.name_str(roster).to_string()),
        region: v.region(),
        pp: &v.sim.params.predation,
        clock_from: v.trace.chase().map(|c| c.tick),
    };
    let mut out = Vec::new();
    let mut prev: Option<HuntSample> = None;
    for s in v.trace.samples() {
        match prev {
            None => out.push(opening(&ctx, s)),
            Some(p) => sample_beats(&ctx, &p, s, &mut out),
        }
        prev = Some(*s);
    }
    outcome_beats(v, &ctx, &mut out);
    if let Some(text) = status_beat(v) {
        out.push(Beat { tick: v.sim.time.tick, text, kind: Kind::End });
    }
    out
}

fn opening(ctx: &Ctx<'_>, s: &HuntSample) -> Beat {
    let (h, p) = (&ctx.hunter, &ctx.prey);
    if let Some(c) = ctx.clock_at(s) {
        return Beat { tick: s.tick, text: format!("{h} is already on {p}: {} cells, {c} ticks on the clock", s.gap), kind: Kind::Tense };
    }
    let tail = if noticed(s.prey_goal) { "" } else { "; it has not noticed" };
    Beat { tick: s.tick, text: format!("{h} picks out {p} at {} cells in {}{tail}", s.gap, ctx.region), kind: Kind::Calm }
}

/// What changed between two consecutive samples.
fn sample_beats(ctx: &Ctx<'_>, prev: &HuntSample, s: &HuntSample, out: &mut Vec<Beat>) {
    let (h, p) = (&ctx.hunter, &ctx.prey);
    let mut push = |text: String, kind: Kind| out.push(Beat { tick: s.tick, text, kind });
    if !noticed(prev.prey_goal) && noticed(s.prey_goal) {
        let text = if s.prey_goal == Goal::Flee { format!("{p} sees {h} and bolts") } else { format!("{p} catches {h}'s scent and grows wary") };
        push(text, Kind::Tense);
    }
    if prev.phase != HuntPhase::Chase && s.phase == HuntPhase::Chase {
        push(format!("chase on: {} cells, {} ticks on the clock", s.gap, ctx.pp.chase_max_ticks), Kind::Hot);
    }
    if s.gap < prev.gap {
        let lunge = usize::from(s.gap) <= ctx.pp.catch_distance_cheb + 1;
        let tail = if lunge { ": a lunge away" } else { "" };
        push(format!("{h} closes to {}{tail}", s.gap), if lunge { Kind::Hot } else { Kind::Tense });
    } else if s.gap > prev.gap {
        push(format!("{p} opens the gap to {}", s.gap), Kind::Calm);
    } else if s.tick % 3 == 0 {
        push(format!("{p} keeps to the {}", terrain_word(s.prey_terrain)), Kind::Calm);
    }
    if let Some(c) = ctx.clock_at(s) {
        let left = u64::from(ctx.pp.chase_max_ticks).saturating_sub(c);
        if matches!(left, 10 | 5 | 2) {
            push(format!("clock: {left} ticks left for {h}"), Kind::Tense);
        }
    }
}

/// The resolution and, a tick later, its aftermath.
fn outcome_beats(v: &HuntView<'_>, ctx: &Ctx<'_>, out: &mut Vec<Beat>) {
    let Some(end) = v.trace.end() else { return };
    let (h, p) = (&ctx.hunter, &ctx.prey);
    let terrain = v.trace.last_sample().map_or("cover", |s| terrain_word(s.prey_terrain));
    let (text, kind) = match end.outcome {
        HuntOutcome::Kill { .. } => (format!("CONTACT: {h} takes {p}"), Kind::Kill),
        HuntOutcome::Miss => (format!("CONTACT: {h} lunges and misses; {p} breaks free"), Kind::Escape),
        HuntOutcome::Timeout => (format!("the clock runs out: {p} outlasts {h}"), Kind::Escape),
        HuntOutcome::Lost => (format!("{p} slips beyond {h}'s senses: lost in the {terrain}"), Kind::Escape),
        HuntOutcome::Dropped => (format!("{h} gives up the chase"), Kind::End),
    };
    out.push(Beat { tick: end.tick, text, kind });
    if v.sim.time.tick <= end.tick {
        return;
    }
    let after = match end.outcome {
        HuntOutcome::Kill { .. } => {
            let share = if v.packmates.is_empty() { "" } else { "; the pack shares the kill" };
            Some(format!("{h} feeds{share}"))
        }
        HuntOutcome::Miss => Some(format!("{p} flees {} cells; {h} gives up for {} hours", ctx.pp.flee_distance.round(), ctx.pp.hunt_cooldown_hours)),
        _ => None,
    };
    if let Some(text) = after {
        out.push(Beat { tick: end.tick + 1, text, kind: Kind::End });
    }
}

fn status_beat(v: &HuntView<'_>) -> Option<String> {
    let text = match v.status {
        Status::Hunting | Status::Dead => return None,
        _ => v.idle_sentence(),
    };
    (!text.is_empty()).then_some(text)
}
