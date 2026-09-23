//! The built-in quirk catalogue: 44 quirks in six groups. Catalogue order is the
//! bit index of a creature's quirk mask, so append new quirks at the end.

use super::{QuirkDef, QuirkKinds, QuirkSpecial, QuirkTier};

use QuirkKinds::{Any, Predator, Prey};
use QuirkTier::{Common, Legendary, Rare};

/// One multiplier of a catalogue row.
#[derive(Clone, Copy)]
enum M {
    Speed(f32),
    Sense(f32),
    Kill(f32),
    Evade(f32),
    Camo(f32),
    Flee(f32),
    Litter(f32),
    Life(f32),
    Mature(f32),
    Sick(f32),
    Hunger(f32),
    Social(f32),
}

fn q(name: &str, tier: QuirkTier, kinds: QuirkKinds, mults: &[M], excludes: &[&str]) -> QuirkDef {
    let mut d = QuirkDef {
        name: name.into(),
        tier,
        kinds,
        inheritable: tier != Legendary,
        excludes: excludes.iter().map(|s| (*s).to_string()).collect(),
        ..QuirkDef::default()
    };
    for m in mults {
        match *m {
            M::Speed(v) => d.speed = v,
            M::Sense(v) => d.sense = v,
            M::Kill(v) => d.kill_odds = v,
            M::Evade(v) => d.evade_odds = v,
            M::Camo(v) => d.camouflage = v,
            M::Flee(v) => d.flee_ticks = v,
            M::Litter(v) => d.litter = v,
            M::Life(v) => d.lifespan = v,
            M::Mature(v) => d.maturity = v,
            M::Sick(v) => d.susceptibility = v,
            M::Hunger(v) => d.hunger = v,
            M::Social(v) => d.sociality = v,
        }
    }
    d
}

pub(super) fn default_catalog() -> Vec<QuirkDef> {
    use M::{Camo, Evade, Flee, Hunger, Kill, Life, Litter, Mature, Sense, Sick, Social, Speed};
    let mut v = vec![
        // ---- physical
        q("Giant", Common, Any, &[Hunger(1.3), Kill(1.15), Evade(1.1), Speed(0.9)], &["Dwarf", "Titan"]),
        q("Dwarf", Common, Any, &[Hunger(0.75), Camo(1.2), Speed(1.05), Kill(0.85)], &["Giant", "Titan"]),
        q("Swift", Common, Any, &[Speed(1.3), Hunger(1.1)], &["Sluggish"]),
        q("Sluggish", Common, Any, &[Speed(0.75), Hunger(0.9)], &["Swift", "Windrunner"]),
        q("Keen-eyed", Common, Any, &[Sense(1.4)], &["Myopic"]),
        q("Myopic", Common, Any, &[Sense(0.6)], &["Keen-eyed", "Oracle"]),
        q("Albino", Rare, Any, &[Camo(0.3), Sense(0.85)], &["Melanistic", "Ghost"]),
        q("Melanistic", Rare, Any, &[Camo(1.3)], &["Albino"]),
        q("Iron Gut", Common, Any, &[Hunger(0.8), Sick(0.9)], &["Glutton"]),
        q("Glutton", Common, Any, &[Hunger(1.35), Litter(1.1)], &["Iron Gut"]),
        q("Frail", Common, Any, &[Evade(0.8), Life(0.85), Sick(1.2)], &["Robust", "Hardy"]),
        q("Robust", Common, Any, &[Evade(1.2), Life(1.1)], &["Frail"]),
        q("Thick Hide", Rare, Any, &[Evade(1.35), Speed(0.95)], &[]),
        q("Lean", Common, Any, &[Hunger(0.85), Evade(0.9)], &[]),
        // ---- temperament
        q("Brave", Common, Any, &[Flee(0.5), Kill(1.1)], &["Cowardly", "Skittish"]),
        q("Cowardly", Common, Any, &[Flee(2.0), Evade(1.15), Kill(0.8)], &["Brave", "Bloodthirsty", "Reckless"]),
        q("Bloodthirsty", Common, Predator, &[Kill(1.25), Hunger(1.15)], &["Gentle", "Cowardly"]),
        q("Gentle", Common, Predator, &[Kill(0.8), Hunger(0.9)], &["Bloodthirsty"]),
        q("Loner", Common, Any, &[Social(0.3)], &["Gregarious", "Alpha"]),
        q("Gregarious", Common, Any, &[Social(1.8)], &["Loner"]),
        q("Skittish", Common, Prey, &[Flee(1.5), Sense(1.15)], &["Brave", "Stoic"]),
        q("Stoic", Common, Any, &[Flee(0.7), Sick(0.9)], &["Skittish"]),
        q("Reckless", Common, Any, &[Kill(1.15), Evade(0.8), Flee(0.6)], &["Cowardly"]),
        // ---- life history
        q("Fertile", Common, Any, &[Litter(1.4)], &["Poor Breeder"]),
        q("Poor Breeder", Common, Any, &[Litter(0.6)], &["Fertile", "Twin-bearer", "Brood Queen"]),
        q("Long-lived", Common, Any, &[Life(1.3)], &["Short-lived"]),
        q("Short-lived", Common, Any, &[Life(0.7)], &["Long-lived", "Undying"]),
        q("Precocious", Common, Any, &[Mature(0.7), Life(0.9)], &["Late Bloomer"]),
        q("Late Bloomer", Common, Any, &[Mature(1.4), Life(1.15)], &["Precocious"]),
        q("Twin-bearer", Rare, Any, &[Litter(1.8), Hunger(1.15)], &["Poor Breeder"]),
        // ---- health
        q("Hardy", Common, Any, &[Sick(0.6)], &["Sickly", "Frail"]),
        q("Sickly", Common, Any, &[Sick(1.6), Life(0.9)], &["Hardy", "Plague-proof"]),
        q("Plague-proof", Rare, Any, &[Sick(0.15)], &["Sickly"]),
        // ---- luck
        q("Lucky", Rare, Any, &[Evade(1.2), Kill(1.1)], &["Cursed"]),
        q("Cursed", Rare, Any, &[Evade(0.8), Kill(0.9), Sick(1.2)], &["Lucky", "Blessed"]),
        // ---- legendary
        q("Undying", Legendary, Any, &[Life(4.0)], &["Short-lived"]),
        q("Titan", Legendary, Any, &[Hunger(2.0), Kill(1.6), Evade(1.6), Speed(0.9)], &["Giant", "Dwarf"]),
        q("Ghost", Legendary, Any, &[Camo(3.0), Sense(1.3)], &["Albino"]),
        q("Windrunner", Legendary, Any, &[Speed(1.8), Flee(0.8)], &["Sluggish"]),
        q("Oracle", Legendary, Any, &[Sense(2.5)], &["Myopic"]),
        q("Blessed", Legendary, Any, &[Evade(1.3), Sick(0.4), Litter(1.4), Life(1.3)], &["Cursed"]),
        q("Brood Queen", Legendary, Any, &[Litter(2.2), Life(1.5)], &["Poor Breeder"]),
        q("Alpha", Legendary, Predator, &[Kill(1.4), Social(2.0), Litter(1.3)], &["Loner"]),
        q("Cannibal", Legendary, Predator, &[Hunger(0.9)], &[]),
    ];
    if let Some(c) = v.last_mut() {
        c.special = QuirkSpecial::Cannibal;
    }
    v
}
