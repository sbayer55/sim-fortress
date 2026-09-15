//! UI styling as extension traits on the pure `sim` data types, so `src/sim`
//! never imports ratatui.

use ratatui::style::Color;

use crate::sim::creatures::{Creature, CreatureId};
use crate::sim::world::World;
use crate::sim::{EventKind, Season, Sim, SpeciesId};
use crate::widgets::map::{MapCreature, MapSource};
use crate::{glyphs, theme};

pub trait SeasonStyle {
    fn glyph(&self) -> char;
    fn color(&self) -> Color;
}

impl SeasonStyle for Season {
    fn glyph(&self) -> char {
        match self {
            Self::Spring => glyphs::SPRING,
            Self::Summer => glyphs::SUMMER,
            Self::Autumn => glyphs::AUTUMN,
            Self::Winter => glyphs::WINTER,
        }
    }

    fn color(&self) -> Color {
        match self {
            Self::Spring => Color::Rgb(120, 220, 120),
            Self::Summer => Color::Rgb(250, 210, 70),
            Self::Autumn => Color::Rgb(240, 140, 50),
            Self::Winter => Color::Rgb(160, 210, 255),
        }
    }
}

pub trait EventKindStyle {
    fn glyph(&self) -> char;
    fn color(&self) -> Color;
}

impl EventKindStyle for EventKind {
    fn glyph(&self) -> char {
        match self {
            Self::Birth => glyphs::BIRTH,
            Self::DeathStarved | Self::DeathThirst | Self::DeathPredation | Self::DeathAge => glyphs::DEATH,
            Self::DeathDisease | Self::Outbreak | Self::Spillover | Self::Epidemic | Self::EpidemicOver => glyphs::DISEASE,
            Self::Recovery => glyphs::IMMUNE,
            Self::Mutation => glyphs::MUTATION,
            Self::Migration => glyphs::MIGRATION,
            Self::Extinction => glyphs::EXTINCTION,
            Self::Drought | Self::DroughtEased => glyphs::DROUGHT,
            Self::Season => glyphs::SUMMER,
            Self::Wary => glyphs::ALERT,
            Self::Note => glyphs::NOTE,
        }
    }

    fn color(&self) -> Color {
        match self {
            Self::Birth | Self::Recovery => theme::GOOD,
            Self::DeathStarved | Self::DeathThirst | Self::Drought | Self::Wary => theme::WARN,
            Self::DeathPredation => theme::BAD,
            Self::DeathAge | Self::DroughtEased | Self::EpidemicOver => theme::DIM,
            Self::Mutation => theme::INFO,
            Self::Migration => theme::ACCENT,
            Self::Extinction | Self::Spillover => theme::MAGENTA,
            Self::Season => theme::TITLE,
            Self::Note => theme::TEXT,
            Self::DeathDisease | Self::Outbreak | Self::Epidemic => theme::SICK,
        }
    }
}

pub trait SpeciesStyle {
    fn glyph(&self) -> char;
    fn color(&self) -> Color;
}

impl SpeciesStyle for SpeciesId {
    fn glyph(&self) -> char {
        match self {
            Self::Vole => glyphs::VOLE,
            Self::Hare => glyphs::HARE,
            Self::Deer => glyphs::DEER,
            Self::Fox => glyphs::FOX,
            Self::Wolf => glyphs::WOLF,
            Self::Lynx => glyphs::LYNX,
        }
    }

    fn color(&self) -> Color {
        match self {
            Self::Vole => theme::VOLE,
            Self::Hare => theme::HARE,
            Self::Deer => theme::DEER,
            Self::Fox => theme::FOX,
            Self::Wolf => theme::WOLF,
            Self::Lynx => theme::LYNX,
        }
    }
}

/// The four vitals as the health overlay (S02g) reads them: each in 0..=1
/// where 1 is best, in the order the vital bars use.
pub const VITALS: [&str; 4] = ["health", "hunger", "thirst", "energy"];

/// A creature's condition: its weakest vital in 0..=1 and the index into
/// `VITALS` of that vital (ties go to the earlier one). Hunger and thirst are
///
/// inverted so that, like health and energy, higher is better.
pub fn condition(c: &Creature) -> (f32, usize) {
    let vitals = [c.hp, 1.0 - c.hunger, 1.0 - c.thirst, c.energy];
    let mut worst = (vitals[0].clamp(0.0, 1.0), 0);
    for (i, v) in vitals.iter().enumerate().skip(1) {
        let v = v.clamp(0.0, 1.0);
        if v < worst.0 {
            worst = (v, i);
        }
    }
    worst
}

/// Build the map renderer's lightweight creature view from a sim creature.
pub fn map_creature(c: &Creature) -> MapCreature<'_> {
    MapCreature {
        id: c.id,
        x: c.x,
        y: c.y,
        alive: c.alive,
        adult: c.adult,
        species: c.species,
        glyph: if c.adult { c.species.glyph().to_ascii_uppercase() } else { c.species.glyph() },
        color: c.species.color(),
        sense_cells: c.genome.sense_cells(),
        condition: condition(c).0,
        trail: &c.trail,
        target: c.target,
    }
}

impl MapSource for Sim {
    fn world(&self) -> &World {
        &self.world
    }

    fn living_creatures(&self) -> Vec<MapCreature<'_>> {
        self.creatures.living().map(map_creature).collect()
    }

    fn creature(&self, id: CreatureId) -> Option<MapCreature<'_>> {
        self.creatures.get(id).map(map_creature)
    }
}
