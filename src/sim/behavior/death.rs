//! Mortality: death checks, grazing pressure and the kill routine.

use crate::sim::creatures::{
    Cause, Creature, DeathTallies,
};
use crate::sim::events::EventRing;
use crate::sim::lineage::Lineage;
use crate::sim::params::{CreaturesParams, Roster, TerritoryParams};
use crate::sim::species::Kind;
use crate::sim::params::DiseaseParams;
use crate::sim::time::Time;
use crate::sim::world::World;
use super::kill;

pub(super) fn maybe_die(c: &mut Creature, world: &mut World, events: &mut EventRing, time: &Time, roster: &Roster, tallies: &mut DeathTallies, lineage: &mut Lineage, dp: &DiseaseParams) {
    if c.alive && c.hp <= 0.0 {
        let cause = if c.thirst >= 1.0 {
            Cause::Thirst
        } else if c.hunger >= 1.0 {
            Cause::Starved
        } else if dp.enabled && c.parasite_load > dp.parasite_hp_threshold {
            Cause::Disease
        } else {
            Cause::Injury
        };
        let label = if cause == Cause::Disease { Some("parasites") } else { None };
        kill(c, cause, world, events, time, roster, tallies, lineage, None, 0, label);
    }
}

pub(super) fn pressure(c: &Creature, world: &mut World, roster: &Roster, cp: &CreaturesParams, tp: &TerritoryParams) {
    match roster.kind(c.species) {
        Kind::Prey => {
            let cell = world.cell_mut(c.x, c.y);
            cell.prey_pressure = (cell.prey_pressure + cp.pressure_per_creature_tick).min(1.0);
        }
        Kind::Predator => {
            let cell = world.cell_mut(c.x, c.y);
            cell.pred_pressure = (cell.pred_pressure + cp.pressure_per_creature_tick).min(1.0);
            // C5 FR13: the scent mark rides the same write.
            super::territory::deposit(c, world, tp);
        }
    }
}
