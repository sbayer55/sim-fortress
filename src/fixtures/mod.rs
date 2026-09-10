//! Static fixture data shared by every prototype. Nothing here ticks; the
//! data is generated once from a fixed seed so every run looks identical.

pub mod creatures;
pub mod events;
pub mod lineage;
pub mod rng;
pub mod series;
pub mod species;
pub mod world;

use std::sync::OnceLock;

pub use creatures::{Creature, Sex, TRAIT_NAMES};
pub use events::{Event, EventKind};
pub use species::{Kind, Species, SpeciesId};
pub use world::{Cell, Terrain, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub fn name(self) -> &'static str {
        match self {
            Season::Spring => "Spring",
            Season::Summer => "Summer",
            Season::Autumn => "Autumn",
            Season::Winter => "Winter",
        }
    }
    pub fn glyph(self) -> char {
        match self {
            Season::Spring => crate::glyphs::SPRING,
            Season::Summer => crate::glyphs::SUMMER,
            Season::Autumn => crate::glyphs::AUTUMN,
            Season::Winter => crate::glyphs::WINTER,
        }
    }
    pub fn color(self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Season::Spring => Color::Rgb(120, 220, 120),
            Season::Summer => Color::Rgb(250, 210, 70),
            Season::Autumn => Color::Rgb(240, 140, 50),
            Season::Winter => Color::Rgb(160, 210, 255),
        }
    }
}

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
        format!(
            "Year {}, Day {} of {}  {:02}:00",
            self.year, self.day, self.season.name(), self.hour
        )
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

static FIXTURES: OnceLock<Fixtures> = OnceLock::new();

pub fn get() -> &'static Fixtures {
    FIXTURES.get_or_init(build)
}

fn build() -> Fixtures {
    let world = world::generate(0xC0FFEE);
    let (creatures, hero_prey, hero_pred, corpse) = creatures::generate(&world, 0xBEEF);
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
    Fixtures {
        world,
        creatures,
        species,
        series,
        events,
        lineage,
        clock,
        hero_prey,
        hero_pred,
        corpse,
    }
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
