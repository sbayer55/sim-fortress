//! S09 preview panel: the terrain image with region names, the terrain
//! summary, the carrying-capacity estimate and the biome shares.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use super::WorldGenForm;
use crate::sim::world::{AgeRegime, Biome};
use crate::sim::World;
use crate::widgets::map;
use crate::widgets::{bars, panel, util};
use crate::{glyphs, theme};

pub(super) fn preview_panel(f: &mut Frame<'_>, area: Rect, form: &WorldGenForm) {
    let world = &form.preview;
    // Smallest zoom-out (at least 1:2) at which the whole world fits 75x20.
    let scale = world.width().div_ceil(75).max(world.height().div_ceil(20)).max(2);
    let hint = format!(
        "seed {}, {}x{} at 1:{}, {} wind, {} land, {} events",
        form.seed_text,
        world.width(),
        world.height(),
        scale,
        world.wind.name(),
        AgeRegime::for_age(form.world.age).name,
        world.history.len()
    );
    let inner = panel::draw_with_hint(f, area, "Preview", &hint, panel::Kind::Outer);

    let iw = crate::cast!(world.width().div_ceil(scale) => u16);
    let ih = crate::cast!(world.height().div_ceil(scale) => u16);
    let px = inner.x + (inner.width.saturating_sub(iw)).div_euclid(2);
    let py = inner.y + 1;
    preview_image(f.buffer_mut(), world, scale, (px, py), (iw, ih));
    let mut row = ih + 3;

    panel::section(f, inner, row, "Terrain summary");
    row += 1;
    let c = terrain_counts(world);
    let total = crate::cast!(world.cells.len().max(1) => f32);
    let groups: Vec<(&str, char, ratatui::style::Color, usize)> = vec![
        ("water", glyphs::DEEP_WATER, theme::SHALLOW_FG, c[0] + c[1]),
        ("sand / dirt", glyphs::SAND, theme::SAND_FG, c[2] + c[3]),
        ("grassland", glyphs::GRASS, theme::GRASS_FG, c[4] + c[5]),
        ("meadow", glyphs::GRASS_DENSE, theme::GRASS_DENSE_FG, c[6]),
        ("forest", glyphs::FOREST, theme::FOREST_FG, c[7]),
        ("rock", glyphs::ROCK, theme::ROCK_FG, c[8]),
        ("marsh", glyphs::MARSH, theme::MARSH_FG, c[9]),
    ];
    let half = inner.width.div_euclid(2);
    for (i, (name, g, color, n)) in groups.iter().enumerate() {
        let col = crate::cast!((i % 2) => u16);
        let r = row + crate::cast!((i.div_euclid(2)) => u16);
        let x = inner.x + 1 + col * half;
        let y = inner.y + r;
        let buf = f.buffer_mut();
        let frac = crate::cast!(*n => f32) / total;
        buf.set_stringn(x, y, format!("{g} "), 2, Style::default().fg(*color).bg(theme::PANEL_BG));
        buf.set_stringn(x + 2, y, format!("{name:<12}"), 12, theme::text());
        bars::bar(buf, x + 14, y, 14, frac, *color);
        buf.set_stringn(x + 29, y, format!("{:>3}% {:>4}", crate::cast!((frac * 100.0).round() => u32), n), 9, theme::dim_text());
    }
    row += 5;

    let forage = c[4] + c[5] + c[6] + c[7] + c[9];
    let prey_cap = crate::cast!((crate::cast!(forage => f32) * 0.35) => u32);
    let pred_cap = prey_cap.div_euclid(8);
    util::line(f, inner, row, Line::from(vec![
        Span::styled(format!(" forage cells {forage}  "), theme::text()),
        Span::styled(format!("{} supports about {} prey and {} predators", glyphs::RIGHT, prey_cap, pred_cap), theme::dim_text()),
    ]));
    row += 1;

    // Biome shares: the five largest, as a share of all cells.
    let mut biomes: Vec<(Biome, usize)> = Biome::ALL.iter().map(|&b| (b, world.cells.iter().filter(|c| c.biome == b).count())).collect();
    biomes.sort_by_key(|&(b, n)| (std::cmp::Reverse(n), b));
    let mut spans = vec![Span::styled(" biomes  ", theme::text())];
    for (b, n) in biomes.iter().take(5).filter(|&&(_, n)| n > 0) {
        let pct = crate::cast!((crate::cast!(*n => f32) / total * 100.0).round() => u32);
        spans.push(Span::styled(format!("{} ", b.name()), Style::default().fg(theme::biome(crate::cast!(*b => u8))).bg(theme::PANEL_BG)));
        spans.push(Span::styled(format!("{pct}%  "), theme::dim_text()));
    }
    spans.push(Span::styled(format!(" regions {}", world.regions.len()), theme::dim_text()));
    util::line(f, inner, row, Line::from(spans));
    row += 1;

    super::chronicle::summary_and_chronicle(f, inner, row, world, form.world.age);
}

fn terrain_counts(world: &World) -> [usize; 10] {
    let mut c = [0usize; 10];
    for cell in &world.cells {
        c[crate::cast!(cell.terrain => usize)] += 1;
    }
    c
}

/// The terrain image at 1:`scale` with the region names over it.
fn preview_image(buf: &mut ratatui::buffer::Buffer, world: &World, scale: usize, (px, py): (u16, u16), (iw, ih): (u16, u16)) {
    for sy in 0..ih {
        for sx in 0..iw {
            let wx = (crate::cast!(sx => usize) * scale).min(world.width() - 1);
            let wy = (crate::cast!(sy => usize) * scale).min(world.height() - 1);
            let (g, fg, bg) = map::world_cell(world, wx, wy, false);
            if let Some(c) = buf.cell_mut((px + sx, py + sy)) {
                c.set_char(g);
                c.set_style(Style::default().fg(fg).bg(bg));
            }
        }
    }
    // Region names, bold and bright, centred on each region and kept
    // inside the image.
    for ri in 0..world.regions.len() {
        let (lx, ly) = map::region_label_origin(world, ri);
        let name = &world.regions[ri].0;
        let len = crate::cast!(name.chars().count() => u16);
        let sx = crate::cast!(lx.div_euclid(scale) => u16).min(iw.saturating_sub(len));
        let sy = crate::cast!(ly.div_euclid(scale) => u16).min(ih.saturating_sub(1));
        for (k, ch) in name.chars().enumerate() {
            if let Some(c) = buf.cell_mut((px + sx + crate::cast!(k => u16), py + sy)) {
                let bg = c.bg;
                c.set_char(ch);
                c.set_style(Style::default().fg(theme::TEXT_BRIGHT).bg(bg).add_modifier(Modifier::BOLD));
            }
        }
    }
}
