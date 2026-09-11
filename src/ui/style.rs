//! UI styling as extension traits on the pure `sim` data types, so `src/sim`
//! never imports ratatui.

use ratatui::style::Color;

use crate::sim::{EventKind, Season, SpeciesId};
use crate::{glyphs, theme};

pub trait SeasonStyle {
    fn glyph(&self) -> char;
    fn color(&self) -> Color;
}

impl SeasonStyle for Season {
    fn glyph(&self) -> char {
        match self {
            Season::Spring => glyphs::SPRING,
            Season::Summer => glyphs::SUMMER,
            Season::Autumn => glyphs::AUTUMN,
            Season::Winter => glyphs::WINTER,
        }
    }

    fn color(&self) -> Color {
        match self {
            Season::Spring => Color::Rgb(120, 220, 120),
            Season::Summer => Color::Rgb(250, 210, 70),
            Season::Autumn => Color::Rgb(240, 140, 50),
            Season::Winter => Color::Rgb(160, 210, 255),
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
            EventKind::Birth => glyphs::BIRTH,
            EventKind::DeathStarved | EventKind::DeathPredation | EventKind::DeathAge => glyphs::DEATH,
            EventKind::Mutation => glyphs::MUTATION,
            EventKind::Migration => glyphs::MIGRATION,
            EventKind::Extinction => glyphs::EXTINCTION,
            EventKind::Drought => glyphs::DROUGHT,
            EventKind::Season => glyphs::SUMMER,
            EventKind::Note => glyphs::NOTE,
        }
    }

    fn color(&self) -> Color {
        match self {
            EventKind::Birth => theme::GOOD,
            EventKind::DeathStarved => theme::WARN,
            EventKind::DeathPredation => theme::BAD,
            EventKind::DeathAge => theme::DIM,
            EventKind::Mutation => theme::INFO,
            EventKind::Migration => theme::ACCENT,
            EventKind::Extinction => theme::MAGENTA,
            EventKind::Drought => theme::WARN,
            EventKind::Season => theme::TITLE,
            EventKind::Note => theme::TEXT,
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
            SpeciesId::Vole => glyphs::VOLE,
            SpeciesId::Hare => glyphs::HARE,
            SpeciesId::Deer => glyphs::DEER,
            SpeciesId::Fox => glyphs::FOX,
            SpeciesId::Wolf => glyphs::WOLF,
            SpeciesId::Lynx => glyphs::LYNX,
        }
    }

    fn color(&self) -> Color {
        match self {
            SpeciesId::Vole => theme::VOLE,
            SpeciesId::Hare => theme::HARE,
            SpeciesId::Deer => theme::DEER,
            SpeciesId::Fox => theme::FOX,
            SpeciesId::Wolf => theme::WOLF,
            SpeciesId::Lynx => theme::LYNX,
        }
    }
}
