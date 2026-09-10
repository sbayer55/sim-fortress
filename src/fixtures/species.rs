//! Species table and per-species trait statistics.

use ratatui::style::Color;

use super::creatures::{Creature, Genome};
use crate::{glyphs, theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpeciesId {
    Vole,
    Hare,
    Deer,
    Fox,
    Wolf,
    Lynx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Prey,
    Predator,
}

impl SpeciesId {
    pub const ALL: [SpeciesId; 6] = [
        SpeciesId::Vole,
        SpeciesId::Hare,
        SpeciesId::Deer,
        SpeciesId::Fox,
        SpeciesId::Wolf,
        SpeciesId::Lynx,
    ];
    pub fn name(self) -> &'static str {
        match self {
            SpeciesId::Vole => "Vole",
            SpeciesId::Hare => "Hare",
            SpeciesId::Deer => "Deer",
            SpeciesId::Fox => "Fox",
            SpeciesId::Wolf => "Wolf",
            SpeciesId::Lynx => "Lynx",
        }
    }
    pub fn plural(self) -> &'static str {
        match self {
            SpeciesId::Vole => "Voles",
            SpeciesId::Hare => "Hares",
            SpeciesId::Deer => "Deer",
            SpeciesId::Fox => "Foxes",
            SpeciesId::Wolf => "Wolves",
            SpeciesId::Lynx => "Lynxes",
        }
    }
    pub fn glyph(self) -> char {
        match self {
            SpeciesId::Vole => glyphs::VOLE,
            SpeciesId::Hare => glyphs::HARE,
            SpeciesId::Deer => glyphs::DEER,
            SpeciesId::Fox => glyphs::FOX,
            SpeciesId::Wolf => glyphs::WOLF,
            SpeciesId::Lynx => glyphs::LYNX,
        }
    }
    pub fn color(self) -> Color {
        match self {
            SpeciesId::Vole => theme::VOLE,
            SpeciesId::Hare => theme::HARE,
            SpeciesId::Deer => theme::DEER,
            SpeciesId::Fox => theme::FOX,
            SpeciesId::Wolf => theme::WOLF,
            SpeciesId::Lynx => theme::LYNX,
        }
    }
    pub fn kind(self) -> Kind {
        match self {
            SpeciesId::Vole | SpeciesId::Hare | SpeciesId::Deer => Kind::Prey,
            _ => Kind::Predator,
        }
    }
    pub fn diet(self) -> &'static str {
        match self {
            SpeciesId::Vole => "seeds, roots",
            SpeciesId::Hare => "grass, bark",
            SpeciesId::Deer => "grass, leaves",
            SpeciesId::Fox => "voles, hares",
            SpeciesId::Wolf => "deer, hares",
            SpeciesId::Lynx => "hares, voles",
        }
    }
    /// Baseline genome around which individuals vary.
    pub fn base_genome(self) -> Genome {
        // speed, size, sense, metabolism, aggression, camouflage, fertility, longevity
        match self {
            SpeciesId::Vole => Genome([0.45, 0.10, 0.40, 0.75, 0.05, 0.60, 0.90, 0.20]),
            SpeciesId::Hare => Genome([0.80, 0.25, 0.65, 0.60, 0.10, 0.55, 0.75, 0.35]),
            SpeciesId::Deer => Genome([0.65, 0.80, 0.55, 0.40, 0.20, 0.35, 0.35, 0.70]),
            SpeciesId::Fox => Genome([0.70, 0.35, 0.80, 0.55, 0.60, 0.50, 0.50, 0.45]),
            SpeciesId::Wolf => Genome([0.75, 0.70, 0.70, 0.50, 0.85, 0.25, 0.40, 0.60]),
            SpeciesId::Lynx => Genome([0.72, 0.50, 0.90, 0.45, 0.75, 0.70, 0.30, 0.55]),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Species {
    pub id: SpeciesId,
    pub count: u32,
    pub adults: u32,
    pub juveniles: u32,
    pub births_today: u32,
    pub deaths_today: u32,
    pub peak: u32,
    pub generation: u32,
    /// 30-day population trend for sparklines.
    pub trend: Vec<u16>,
    pub mean: Genome,
    pub min: Genome,
    pub max: Genome,
    /// Per-trait histogram, 12 buckets each, values are creature counts.
    pub hist: [[u16; 12]; 8],
    /// Mean of each trait at generation 1, 5, 10, ... for drift charts.
    pub drift: Vec<Genome>,
}

pub fn generate(creatures: &[Creature]) -> Vec<Species> {
    let mut rng = super::rng::Rng::new(0x5EED_5EED);
    SpeciesId::ALL
        .iter()
        .map(|&id| {
            let members: Vec<&Creature> = creatures.iter().filter(|c| c.species == id && c.alive).collect();
            let sampled = members.len() as u32;
            // Scale sample counts up to a plausible world population.
            let scale = match id.kind() {
                Kind::Prey => 6,
                Kind::Predator => 3,
            };
            let count = sampled * scale + rng.below(scale as usize) as u32;
            let adults = members.iter().filter(|c| c.adult).count() as u32 * scale;
            let juveniles = count.saturating_sub(adults);
            let mut mean = Genome([0.0; 8]);
            let mut min = Genome([1.0; 8]);
            let mut max = Genome([0.0; 8]);
            let mut hist = [[0u16; 12]; 8];
            for c in &members {
                for t in 0..8 {
                    let v = c.genome.0[t];
                    mean.0[t] += v;
                    min.0[t] = min.0[t].min(v);
                    max.0[t] = max.0[t].max(v);
                    let b = ((v * 12.0) as usize).min(11);
                    hist[t][b] += scale as u16 + rng.below(3) as u16;
                }
            }
            if !members.is_empty() {
                for t in 0..8 {
                    mean.0[t] /= members.len() as f32;
                }
            }
            let base = id.base_genome();
            let generation = match id.kind() {
                Kind::Prey => 40 + rng.below(30) as u32,
                Kind::Predator => 18 + rng.below(12) as u32,
            };
            let drift = (0..12)
                .map(|g| {
                    let f = g as f32 / 11.0;
                    let mut gn = Genome([0.0; 8]);
                    for t in 0..8 {
                        let wobble = (g as f32 * 1.7 + t as f32).sin() * 0.03;
                        gn.0[t] = (base.0[t] + (mean.0[t] - base.0[t]) * f + wobble).clamp(0.0, 1.0);
                    }
                    gn
                })
                .collect();
            let mut trend = Vec::with_capacity(30);
            let mut v = count as f32 * (0.8 + rng.f32() * 0.4);
            for i in 0..30 {
                let phase = (i as f32 / 30.0 * 6.28 + id as usize as f32).sin();
                v = (v * 0.92 + count as f32 * 0.08 + phase * count as f32 * 0.08 + rng.gauss(0.0, 2.0)).max(0.0);
                trend.push(v as u16);
            }
            *trend.last_mut().unwrap() = count as u16;
            Species {
                id,
                count,
                adults,
                juveniles,
                births_today: rng.below(match id.kind() { Kind::Prey => 9, Kind::Predator => 3 }) as u32,
                deaths_today: rng.below(match id.kind() { Kind::Prey => 7, Kind::Predator => 2 }) as u32,
                peak: (count as f32 * (1.2 + rng.f32() * 0.8)) as u32,
                generation,
                trend,
                mean,
                min,
                max,
                hist,
                drift,
            }
        })
        .collect()
}

impl Species {
    pub fn trend_arrow(&self) -> char {
        let n = self.trend.len();
        let a = self.trend[n - 8] as i32;
        let b = self.trend[n - 1] as i32;
        if b > a + 2 {
            glyphs::UP
        } else if b + 2 < a {
            glyphs::DOWN
        } else {
            glyphs::FLAT
        }
    }
}
