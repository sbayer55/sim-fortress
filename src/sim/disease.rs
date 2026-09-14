//! Disease and parasites (C7). Contagious pathogens spread by proximity and
//!
//! through carcasses; a continuous parasite load builds up from fouled ground
//! and water; the Resistance gene gates both. Outbreaks are recorded as named
//! events, and a pathogen can mutate into a predator's species when the predator
//! eats infected prey (spillover, `FR8b`).
//!
//!
//! Determinism: infectious creatures are visited in id order, queued infections
//! are applied in `(target, source)` order, and every roll comes from the
//! dedicated `disease_rng` stream.


pub use types::{DiseaseState, Infection, MAX_PATHOGENS, OUTBREAKS_MAX, Outbreak, Pathogen, PathogenId, PathogenStats, Stage};
pub use effects::{Effects, REST_ENERGY, effects, is_immune, is_infectious};
pub use contagion::{contagion_pass, on_eat, parasite_shed, parasite_uptake};
pub use spillover::at_birth;
pub use daily::{daily_update, decay_cells, mean_resistance, progress_daily};

mod types;
mod effects;
mod contagion;
mod spillover;
mod lifecycle;
mod daily;
mod outbreaks;
mod emergence;
#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
#[allow(clippy::float_cmp)]
mod tests;
