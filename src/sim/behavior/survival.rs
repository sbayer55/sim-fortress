//! S16 survival tallies: the hardships an animal came through. Pure counters,
//! bumped after the state that defines them has flipped; no RNG, no events.

use crate::sim::creatures::CreatureStore;
use crate::sim::world::World;

/// A drought that eased today was survived by everyone standing in its region.
///
/// `before` and `after` are `Sim::drought` around the ecology update; regions
/// past the eighth fold into the last slot, as elsewhere.
pub fn droughts_eased(store: &mut CreatureStore, world: &World, before: [bool; 8], after: [bool; 8]) {
    if before == after {
        return;
    }
    let eased: [bool; 8] = std::array::from_fn(|i| before[i] && !after[i]);
    if !eased.iter().any(|&e| e) {
        return;
    }
    for c in store.living_mut() {
        let region = world.region_index(c.x, c.y).min(7);
        if eased.get(region).copied().unwrap_or(false) {
            c.droughts_survived = c.droughts_survived.saturating_add(1);
        }
    }
}

/// Spring returned: every living animal survived the winter. Every winter
/// counts; the sim has no winter severity.
pub fn winter_survived(store: &mut CreatureStore) {
    for c in store.living_mut() {
        c.winters_survived = c.winters_survived.saturating_add(1);
    }
}
