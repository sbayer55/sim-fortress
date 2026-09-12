//! Truecolor palette for Sim Fortress. Every color is an explicit `Color::Rgb`.

use ratatui::style::{Color, Modifier, Style};

// ---------------------------------------------------------------- UI chrome
pub const BG: Color = Color::Rgb(12, 12, 16);
pub const PANEL_BG: Color = Color::Rgb(18, 18, 24);
pub const BORDER: Color = Color::Rgb(96, 96, 118);
pub const BORDER_FOCUS: Color = Color::Rgb(214, 178, 84);
pub const TITLE: Color = Color::Rgb(236, 206, 118);
pub const TEXT: Color = Color::Rgb(206, 206, 206);
pub const TEXT_BRIGHT: Color = Color::Rgb(245, 245, 240);
pub const DIM: Color = Color::Rgb(112, 112, 126);
pub const ACCENT: Color = Color::Rgb(242, 184, 62);
pub const KEY: Color = Color::Rgb(255, 230, 120);
pub const GOOD: Color = Color::Rgb(96, 200, 96);
pub const WARN: Color = Color::Rgb(236, 160, 48);
pub const BAD: Color = Color::Rgb(226, 72, 64);
pub const INFO: Color = Color::Rgb(96, 168, 236);
pub const MAGENTA: Color = Color::Rgb(220, 96, 220);
pub const HEADER_BG: Color = Color::Rgb(40, 40, 60);
pub const HEADER_FG: Color = Color::Rgb(255, 240, 200);
pub const STATUS_BG: Color = Color::Rgb(28, 28, 40);
pub const SELECT_BG: Color = Color::Rgb(70, 60, 30);
pub const CURSOR_BG: Color = Color::Rgb(255, 255, 255);
pub const CURSOR_FG: Color = Color::Rgb(0, 0, 0);

// ---------------------------------------------------------------- terrain
pub const DEEP_WATER_FG: Color = Color::Rgb(58, 96, 178);
pub const DEEP_WATER_BG: Color = Color::Rgb(14, 30, 78);
pub const SHALLOW_FG: Color = Color::Rgb(96, 150, 210);
pub const SHALLOW_BG: Color = Color::Rgb(24, 58, 120);
pub const SAND_FG: Color = Color::Rgb(206, 186, 126);
pub const SAND_BG: Color = Color::Rgb(72, 64, 40);
pub const DIRT_FG: Color = Color::Rgb(150, 118, 78);
pub const DIRT_BG: Color = Color::Rgb(48, 38, 26);
pub const GRASS_SPARSE_FG: Color = Color::Rgb(112, 150, 66);
pub const GRASS_FG: Color = Color::Rgb(92, 168, 68);
pub const GRASS_DENSE_FG: Color = Color::Rgb(66, 190, 80);
pub const GRASS_BG: Color = Color::Rgb(22, 40, 20);
pub const FOREST_FG: Color = Color::Rgb(36, 118, 52);
pub const FOREST_BG: Color = Color::Rgb(12, 34, 18);
pub const ROCK_FG: Color = Color::Rgb(156, 156, 160);
pub const ROCK_BG: Color = Color::Rgb(54, 54, 58);
pub const SNOW_FG: Color = Color::Rgb(240, 244, 250);
pub const SNOW_BG: Color = Color::Rgb(118, 124, 140);

// ---------------------------------------------------------------- resources
pub const CARCASS: Color = Color::Rgb(178, 74, 58);
pub const DEN: Color = Color::Rgb(196, 150, 96);
pub const SEED: Color = Color::Rgb(180, 220, 120);
pub const TRAIL: Color = Color::Rgb(255, 210, 90);

// ---------------------------------------------------------------- species
pub const VOLE: Color = Color::Rgb(188, 156, 116);
pub const HARE: Color = Color::Rgb(228, 220, 196);
pub const DEER: Color = Color::Rgb(214, 160, 92);
pub const FOX: Color = Color::Rgb(246, 128, 42);
pub const WOLF: Color = Color::Rgb(224, 66, 66);
pub const LYNX: Color = Color::Rgb(236, 110, 150);
pub const VEGETATION: Color = Color::Rgb(96, 196, 96);
/// Disease / infection colour (C7): a sickly yellow-green.
pub const SICK: Color = Color::Rgb(150, 205, 70);
/// Immunity marker colour (C7).
pub const IMMUNE: Color = INFO;

/// Linear blend between two RGB colors. `t` is clamped to 0..=1.
pub fn lerp(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    match (a, b) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
            Color::Rgb(f(r1, r2), f(g1, g2), f(b1, b2))
        }
        _ => b,
    }
}

/// Multi-stop gradient. `stops` must have at least one entry.
pub fn ramp(stops: &[Color], t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    if stops.len() == 1 {
        return stops[0];
    }
    let scaled = t * (stops.len() - 1) as f32;
    let i = (scaled.floor() as usize).min(stops.len() - 2);
    lerp(stops[i], stops[i + 1], scaled - i as f32)
}

/// Cold-to-hot ramp used for pressure/density overlays.
pub fn heat(t: f32) -> Color {
    ramp(
        &[
            Color::Rgb(20, 30, 90),
            Color::Rgb(40, 110, 200),
            Color::Rgb(90, 200, 120),
            Color::Rgb(240, 210, 60),
            Color::Rgb(240, 110, 40),
            Color::Rgb(220, 40, 40),
        ],
        t,
    )
}

/// Vegetation density ramp (bare soil to lush).
pub fn veg(t: f32) -> Color {
    ramp(
        &[
            Color::Rgb(70, 52, 34),
            Color::Rgb(110, 120, 50),
            Color::Rgb(80, 170, 70),
            Color::Rgb(40, 220, 90),
        ],
        t,
    )
}

/// Water/moisture ramp (dry tan to saturated blue).
pub fn water(t: f32) -> Color {
    ramp(
        &[
            Color::Rgb(90, 70, 40),
            Color::Rgb(120, 140, 120),
            Color::Rgb(60, 130, 200),
            Color::Rgb(30, 70, 220),
        ],
        t,
    )
}

/// Parasite-load ramp (S02i): dim olive (clean) through `WARN` to `BAD` (fouled).
pub fn parasite(t: f32) -> Color {
    ramp(&[Color::Rgb(70, 74, 34), WARN, BAD], t)
}

/// Species-density ramp (S02f): near-black through the species' own colour to
/// a bright tint of it, so the hue names the species.
pub fn species_ramp(species: Color, t: f32) -> Color {
    ramp(&[dim(species, 0.85), dim(species, 0.45), species, lerp(species, TEXT_BRIGHT, 0.45)], t)
}

/// Dim a color toward the background (used for "night" and for dimming a
/// base screen underneath a modal).
pub fn dim(c: Color, amount: f32) -> Color {
    lerp(c, BG, amount)
}

/// Blue-shift a color for the night palette.
pub fn night(c: Color) -> Color {
    match c {
        Color::Rgb(r, g, b) => {
            let r = (r as f32 * 0.55) as u8;
            let g = (g as f32 * 0.62) as u8;
            let b = (b as f32 * 0.88 + 14.0).min(255.0) as u8;
            Color::Rgb(r, g, b)
        }
        other => other,
    }
}

// ---------------------------------------------------------------- regions
/// Categorical palette for the region overlay: eight muted, mutually
/// distinguishable hues (one per fixed region). Indexed modulo the length so
/// any region count is safe.
pub const REGION: [Color; 8] = [
    Color::Rgb(210, 90, 80),   // brick red
    Color::Rgb(230, 170, 50),  // amber
    Color::Rgb(120, 200, 90),  // leaf green
    Color::Rgb(70, 190, 190),  // teal
    Color::Rgb(90, 130, 230),  // cornflower blue
    Color::Rgb(170, 110, 230), // violet
    Color::Rgb(230, 110, 180), // rose
    Color::Rgb(190, 190, 120), // khaki
];

/// The tint colour for region `i`.
pub fn region(i: usize) -> Color {
    REGION[i % REGION.len()]
}

// ---------------------------------------------------------------- styles
pub fn text() -> Style {
    Style::default().fg(TEXT).bg(PANEL_BG)
}
pub fn dim_text() -> Style {
    Style::default().fg(DIM).bg(PANEL_BG)
}
pub fn title() -> Style {
    Style::default().fg(TITLE).bg(PANEL_BG).add_modifier(Modifier::BOLD)
}
pub fn key() -> Style {
    Style::default().fg(KEY).bg(PANEL_BG).add_modifier(Modifier::BOLD)
}
pub fn border() -> Style {
    Style::default().fg(BORDER).bg(PANEL_BG)
}
pub fn border_focus() -> Style {
    Style::default().fg(BORDER_FOCUS).bg(PANEL_BG)
}
pub fn label() -> Style {
    Style::default().fg(ACCENT).bg(PANEL_BG)
}
pub fn selected() -> Style {
    Style::default().fg(TEXT_BRIGHT).bg(SELECT_BG).add_modifier(Modifier::BOLD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_palette_distinct() {
        for i in 0..REGION.len() {
            for j in (i + 1)..REGION.len() {
                assert_ne!(REGION[i], REGION[j], "regions {i} and {j} share a colour");
            }
        }
        assert_eq!(region(8), region(0));
    }
}
