//! C2 FR12: succession — the daily classification of every land cell on the
//! ladder and the flip roll once a cell is ripe.
//!
//! Runs as sub-step 8b of `daily_update`, after the regrowth sites and before
//! the series sample, so every draw of the day that existed before this step
//! keeps its position. Trampling itself lives in `step_vegetation`; this
//! module only reads the targets that step computed.

use crate::sim::events::{EventKind, EventRing};
use crate::sim::params::{EcologyParams, SuccessionParams};
use crate::sim::rng::Rng;
use crate::sim::time::Time;
use crate::sim::world::{Cell, Terrain, World};

use super::region_event;

/// What one day did to a cell on the ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Signal {
    /// Lush, lightly used and moist enough for the next rung.
    Thriving,
    /// Grazed bare and trodden.
    Worn,
    Neither,
}

/// Classify one on-ladder cell for today. `untrampled` is the target the
/// vegetation step computed before trampling; the trampled target is derived
/// from it here so the two steps agree to the bit.
pub(super) fn classify(cell: &Cell, untrampled: f32, sp: &SuccessionParams) -> Signal {
    let trampled = untrampled * sp.trample_factor(cell.prey_pressure);
    let moist_enough = cell.next_rung().is_some_and(|next| cell.moisture >= sp.climb_moisture.get(&next).copied().unwrap_or(1.0));
    if moist_enough && cell.vegetation >= sp.climb_veg * trampled && cell.prey_pressure < sp.trample_low {
        Signal::Thriving
    } else if cell.vegetation < sp.wear_veg * untrampled && cell.prey_pressure >= sp.trample_high {
        Signal::Worn
    } else {
        Signal::Neither
    }
}

/// Advance the two counters by today's signal.
pub(super) const fn count(cell: &mut Cell, signal: Signal, sp: &SuccessionParams) {
    match signal {
        Signal::Thriving => {
            cell.thrive_days = cell.thrive_days.saturating_add(1);
            cell.wear_days = 0;
        }
        Signal::Worn => {
            cell.wear_days = cell.wear_days.saturating_add(1);
            cell.thrive_days = 0;
        }
        Signal::Neither => {
            cell.thrive_days = cell.thrive_days.saturating_sub(sp.relax_per_day);
            cell.wear_days = cell.wear_days.saturating_sub(sp.relax_per_day);
        }
    }
}

/// The rung a ripe cell flips to today, if any: up when the thriving count
/// has reached the next rung's `climb_days` and the cell is moist enough for
/// it, down when the worn count has reached `wear_days_needed`. The two
/// counters reset each other, so a cell is never ripe both ways.
pub(super) fn ripe(cell: &Cell, sp: &SuccessionParams) -> Option<Terrain> {
    if let Some(next) = cell.next_rung() {
        let need = sp.climb_days.get(&next).copied().unwrap_or(u32::MAX);
        let moist = sp.climb_moisture.get(&next).copied().unwrap_or(1.0);
        if u32::from(cell.thrive_days) >= need && cell.moisture >= moist {
            return Some(next);
        }
    }
    if let Some(down) = cell.terrain.wear() {
        if u32::from(cell.wear_days) >= sp.wear_days_needed {
            return Some(down);
        }
    }
    None
}

/// Step 8b: count today's signal on every on-ladder cell in row-major order
/// and roll the flip for every ripe one; one `Note` per region per day per
/// direction. `targets` is the untrampled target per cell from step 3.
#[allow(clippy::too_many_arguments)]
pub(super) fn step_succession(world: &mut World, rng: &mut Rng, time: &Time, events: &mut EventRing, ecology: &EcologyParams, sp: &SuccessionParams, targets: &[f32], w: usize, h: usize) {
    let mut climbed = [false; 8];
    let mut worn = [false; 8];
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if !world.cells[idx].terrain.on_ladder() {
                continue;
            }
            let signal = classify(&world.cells[idx], targets[idx], sp);
            count(&mut world.cells[idx], signal, sp);
            if sp.flip_chance <= 0.0 {
                continue;
            }
            let Some(to) = ripe(&world.cells[idx], sp) else { continue };
            if !rng.chance(sp.flip_chance) {
                continue;
            }
            let up = crate::cast!(to => u8) > crate::cast!(world.cells[idx].terrain => u8);
            flip(&mut world.cells[idx], to, ecology);
            let ri = world.region_index(x, y).min(7);
            let (seen, text) = if up {
                (&mut climbed[ri], format!("Scrub is closing over {}", world.regions[ri].0))
            } else {
                (&mut worn[ri], format!("Grazing wears {} back", world.regions[ri].0))
            };
            if !*seen {
                *seen = true;
                events.push(region_event(time, EventKind::Note, world, ri, text));
            }
        }
    }
}

/// Move a cell to `to`: both counters reset and the vegetation is clamped to
/// the new rung's cap. Nothing else on the cell changes.
fn flip(cell: &mut Cell, to: Terrain, ecology: &EcologyParams) {
    cell.terrain = to;
    cell.thrive_days = 0;
    cell.wear_days = 0;
    let cap = ecology.max_vegetation.get(&to).copied().unwrap_or(0.0);
    cell.vegetation = cell.vegetation.min(cap);
}
