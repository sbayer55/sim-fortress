//! Static fixture data shared by every prototype. Nothing here ticks; the
//! data is generated once from a fixed seed so every run looks identical.
//!
//! The plain data types now live in `crate::sim` and are re-exported here so the
//! prototype files keep working unchanged. UI styling (glyph/color) lives in
//! `crate::ui::style` and is re-exported alongside the data types.

pub mod creatures;
pub mod events;
pub mod lineage;
pub mod series;
pub mod species;
pub mod world;

use std::sync::OnceLock;

use crate::sim::creatures::CreatureId;
use crate::widgets::map::{MapCreature, MapSource};

pub use crate::sim::{Cell, Event, EventKind, Genome, Kind, Season, SpeciesId, Terrain, World, TRAIT_NAMES};
pub use crate::ui::style::{EventKindStyle, SeasonStyle, SpeciesStyle};
pub use creatures::{Creature, Sex};
pub use species::Species;

#[derive(Clone, Debug)]
pub struct Clock {
    pub year: u32,
    pub day: u32,
    pub season: Season,
    pub hour: u32,
    pub tick: u64,
    pub speed: u8,
    pub paused: bool,
}

impl Clock {
    pub fn label(&self) -> String {
        format!("Year {}, Day {} of {}  {:02}:00", self.year, self.day, self.season.name(), self.hour)
    }
}

pub struct Fixtures {
    pub world: World,
    pub creatures: Vec<Creature>,
    pub species: Vec<Species>,
    pub series: series::Series,
    pub events: Vec<Event>,
    pub lineage: lineage::Lineage,
    pub clock: Clock,
    /// Index into `creatures` of the "selected" hare used by inspector/follow screens.
    pub hero_prey: usize,
    /// Index into `creatures` of the "selected" wolf.
    pub hero_pred: usize,
    /// Index into `creatures` of a dead creature (corpse).
    pub corpse: usize,
}

impl Fixtures {
    fn view<'a>(c: &'a Creature) -> MapCreature<'a> {
        MapCreature {
            id: CreatureId(c.id),
            x: c.x,
            y: c.y,
            alive: c.alive,
            adult: c.adult,
            glyph: c.glyph(),
            color: c.species.color(),
            sense_cells: c.genome.sense_cells(),
            trail: &c.trail,
            target: c.target,
        }
    }
}

impl MapSource for Fixtures {
    fn world(&self) -> &World {
        &self.world
    }

    fn living_creatures(&self) -> Vec<MapCreature<'_>> {
        self.creatures.iter().filter(|c| c.alive).map(|c| Self::view(c)).collect()
    }

    fn creature(&self, id: CreatureId) -> Option<MapCreature<'_>> {
        self.creatures.iter().find(|c| c.id == id.0).map(|c| Self::view(c))
    }
}

static FIXTURES: OnceLock<Fixtures> = OnceLock::new();

pub fn get() -> &'static Fixtures {
    FIXTURES.get_or_init(build)
}

fn build() -> Fixtures {
    let mut world = world::generate(0xC0FFEE);
    let (creatures, hero_prey, hero_pred, corpse) = creatures::generate(&world, 0xBEEF);
    // The hero corpse renders as a carcass from `world.carcasses` (FR Scope).
    world.carcasses.push((creatures[corpse].x, creatures[corpse].y));
    let species = species::generate(&creatures);
    let series = series::generate();
    let events = events::generate(&creatures);
    let lineage = lineage::generate();
    let clock = Clock {
        year: 12,
        day: 4,
        season: Season::Autumn,
        hour: 14,
        tick: 1_064_772,
        speed: 2,
        paused: false,
    };
    Fixtures { world, creatures, species, series, events, lineage, clock, hero_prey, hero_pred, corpse }
}

/// Region name for a map position (usable before the fixture is fully built).
pub fn get_region(x: usize, y: usize) -> &'static str {
    match ((x / 50).min(2), y) {
        (0, 0..=13) => "Northmarch",
        (1, 0..=11) => "Ashen Ridge",
        (2, 0..=15) => "Sunfall Coast",
        (0, 14..=27) => "Reedwater Vale",
        (1, 12..=27) => "The Long Meadow",
        (2, _) => "Lakeshore",
        (0, _) => "Southern Thicket",
        _ => "Fenlands",
    }
}
