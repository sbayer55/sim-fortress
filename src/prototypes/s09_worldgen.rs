//! S09: world generation form with a live preview.

#[allow(unused_imports)]
use crate::fixtures::{EventKindStyle as _, SeasonStyle as _, SpeciesStyle as _};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use super::Prototype;
use crate::fixtures::{self, Fixtures, SpeciesId, Terrain};
use crate::widgets::map;
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

pub struct WorldGen;

pub fn all() -> Vec<Box<dyn Prototype>> {
    vec![Box::new(WorldGen)]
}

const FORM_W: u16 = 66;
const PREVIEW_W: u16 = 75;
const PREVIEW_H: u16 = 20;

fn sp(s: impl Into<String>, st: Style) -> Span<'static> {
    Span::styled(s.into(), st)
}

/// A form field: label, value, whether it is an adjustable (< >) field.
struct Field {
    label: &'static str,
    value: String,
    adjustable: bool,
    hint: &'static str,
}

fn field(label: &'static str, value: impl Into<String>, adjustable: bool, hint: &'static str) -> Field {
    Field { label, value: value.into(), adjustable, hint }
}

/// Draw one field row. `focused` uses the selected style plus `< >` arrows.
fn draw_field(f: &mut Frame, area: Rect, row: u16, fld: &Field, focused: bool) {
    if row >= area.height {
        return;
    }
    let y = area.y + row;
    let buf = f.buffer_mut();
    let label_style = if focused { theme::label().add_modifier(Modifier::BOLD) } else { theme::text() };
    buf.set_stringn(area.x + 1, y, format!("{:<20}", fld.label), 20, label_style);
    let box_w = 26u16;
    let bx = area.x + 21;
    let (l, r) = if fld.adjustable { (glyphs::REWIND, glyphs::PLAY) } else { ('[', ']') };
    let arrow_style = if focused {
        Style::default().fg(theme::KEY).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)
    } else {
        theme::dim_text()
    };
    buf.set_stringn(bx, y, l.to_string(), 1, arrow_style);
    let val = format!(" {:<w$}", fld.value, w = box_w as usize - 3);
    let val_style = if focused { theme::selected() } else { Style::default().fg(theme::TEXT_BRIGHT).bg(theme::BG) };
    buf.set_stringn(bx + 1, y, &val, box_w as usize - 2, val_style);
    buf.set_stringn(bx + box_w - 1, y, r.to_string(), 1, arrow_style);
    buf.set_stringn(bx + box_w + 1, y, fld.hint, (area.width - (box_w + 22)) as usize, theme::dim_text());
}

impl Prototype for WorldGen {
    fn id(&self) -> &'static str {
        "S09a"
    }
    fn name(&self) -> &'static str {
        "World Generation"
    }
    fn variant(&self) -> &'static str {
        "new world form"
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        let fx = fixtures::get();
        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let form_area = Rect::new(area.x, area.y, FORM_W, body_h);
        let preview_area = Rect::new(area.x + FORM_W, area.y, area.width - FORM_W, body_h);
        form(f, form_area, fx);
        preview(f, preview_area, fx);
        status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("Tab", "next field"), ("←→", "adjust"), ("Enter", "generate"), ("Esc", "back")],
            "seed 0xC0FFEE  preview is live",
        );
    }
}

fn form(f: &mut Frame, area: Rect, fx: &Fixtures) {
    let inner = panel::draw_with_hint(f, area, "New World", "field 3 of 17", panel::Kind::Outer);
    let mut row = 0u16;
    let (water, forest, rock) = terrain_mix(fx);

    panel::section(f, inner, row, "World");
    row += 1;
    let world_fields = [
        field("World name", "The Valley of Sunfall", false, "text"),
        field("Seed", "0xC0FFEE", false, "hex/decimal"),
        field("Size", "150 x 40", true, "w x h cells"),
        field("Water %", format!("{}", water), true, "lakes + rivers"),
        field("Forest %", format!("{}", forest), true, "predator cover"),
        field("Rock %", format!("{}", rock), true, "impassable"),
        field("Rainfall", "normal", true, "dry/normal/wet"),
        field("Season length", "90 days", true, "30 - 180 days"),
    ];
    for (i, fld) in world_fields.iter().enumerate() {
        draw_field(f, inner, row, fld, i == 2);
        row += 1;
    }
    row += 1;

    panel::section(f, inner, row, "Initial species");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp("   species    kind       count   ", theme::dim_text()),
        sp("share of starting population", theme::dim_text()),
    ]));
    row += 1;
    let counts = [
        (SpeciesId::Vole, 240u32),
        (SpeciesId::Hare, 180),
        (SpeciesId::Deer, 90),
        (SpeciesId::Fox, 30),
        (SpeciesId::Wolf, 24),
        (SpeciesId::Lynx, 12),
    ];
    let total: u32 = counts.iter().map(|c| c.1).sum();
    for (i, (id, n)) in counts.iter().enumerate() {
        let focused = i == 1;
        let y = inner.y + row;
        let buf = f.buffer_mut();
        let base = if focused { theme::selected() } else { theme::text() };
        let bg = if focused { theme::SELECT_BG } else { theme::PANEL_BG };
        if focused {
            for x in inner.x..inner.right() {
                if let Some(c) = buf.cell_mut((x, y)) {
                    c.set_bg(bg);
                }
            }
        }
        buf.set_stringn(inner.x + 1, y, format!("{} ", id.glyph().to_ascii_uppercase()), 2, Style::default().fg(id.color()).bg(bg).add_modifier(Modifier::BOLD));
        buf.set_stringn(inner.x + 3, y, format!("{:<10}", id.plural()), 10, base);
        let kind = match id.kind() {
            fixtures::Kind::Prey => "prey",
            fixtures::Kind::Predator => "predator",
        };
        buf.set_stringn(inner.x + 13, y, format!("{:<9}", kind), 9, Style::default().fg(theme::DIM).bg(bg));
        let arrows = if focused {
            Style::default().fg(theme::KEY).bg(bg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::DIM).bg(bg)
        };
        buf.set_stringn(inner.x + 22, y, glyphs::REWIND.to_string(), 1, arrows);
        buf.set_stringn(inner.x + 23, y, format!("{:>5} ", n), 6, base);
        buf.set_stringn(inner.x + 29, y, glyphs::PLAY.to_string(), 1, arrows);
        let share = *n as f32 / total as f32;
        bars::bar(buf, inner.x + 33, y, 22, share, id.color());
        buf.set_stringn(inner.x + 56, y, format!("{:>3}%", (share * 100.0).round() as u32), 4, Style::default().fg(theme::TEXT).bg(bg));
        row += 1;
    }
    util::line(f, inner, row, Line::from(vec![
        sp(format!("   total {}   prey {}   predators {}   ratio {:.1}:1", total, 510, 66, 510.0 / 66.0), theme::dim_text()),
    ]));
    row += 2;

    panel::section(f, inner, row, "Evolution");
    row += 1;
    let evo_fields = [
        field("Mutation rate", "0.04", true, "per trait/birth"),
        field("Mutation strength", "0.06", true, "mutation sd"),
        field("Predation difficulty", "normal", true, "easy/norm/hard"),
        field("Regrowth rate", "1.0", true, "veg multiplier"),
    ];
    for fld in &evo_fields {
        draw_field(f, inner, row, fld, false);
        row += 1;
    }
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {} ", glyphs::NOTE), theme::label()),
        sp("Higher mutation strength speeds adaptation but", theme::dim_text()),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(sp("   raises the chance of unviable offspring.", theme::dim_text())));
    row += 2;

    panel::section(f, inner, row, "Presets");
    row += 1;
    let presets: [(&str, &str, bool); 5] = [
        ("Balanced", "default values, gentle seasons", true),
        ("Harsh winter", "180-day seasons, regrowth 0.6", false),
        ("Lush", "forest 30%, regrowth 1.4, predation hard", false),
        ("Archipelago", "water 55%, islands isolate lineages", false),
        ("Fast evolution", "mutation rate 0.10, strength 0.12", false),
    ];
    for (name, desc, on) in presets {
        util::line(f, inner, row, Line::from(vec![
            sp(format!(" {} ", if on { glyphs::DIAMOND } else { glyphs::DOT }), if on { theme::label() } else { theme::dim_text() }),
            sp(format!("{:<16}", name), if on { theme::title() } else { theme::text() }),
            sp(desc, theme::dim_text()),
        ]));
        row += 1;
    }
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {} ", glyphs::PLAY), theme::key()),
        sp("Generate builds the world from the seed above; the", theme::dim_text()),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(sp("   preview updates as seed or terrain sliders change.", theme::dim_text())));

    // Buttons pinned to the bottom of the form panel.
    let brow = inner.height - 1;
    let y = inner.y + brow;
    let buf = f.buffer_mut();
    let buttons: [(&str, bool); 3] = [("[ Generate ]", true), ("[ Randomize seed ]", false), ("[ Back ]", false)];
    let mut x = inner.x + 4;
    for (label, primary) in buttons {
        let st = if primary {
            Style::default().fg(theme::CURSOR_FG).bg(theme::ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::TEXT_BRIGHT).bg(theme::HEADER_BG)
        };
        buf.set_stringn(x, y, label, label.len(), st);
        x += label.len() as u16 + 3;
    }
}

/// Fractions of the world that are water, forest and rock (percent).
fn terrain_mix(fx: &Fixtures) -> (u32, u32, u32) {
    let counts = terrain_counts(fx);
    let total = fx.world.cells.len() as f32;
    let pct = |n: usize| (n as f32 / total * 100.0).round() as u32;
    (pct(counts.0 + counts.1), pct(counts.7), pct(counts.8))
}

/// Per-terrain cell counts in `Terrain` declaration order.
fn terrain_counts(fx: &Fixtures) -> (usize, usize, usize, usize, usize, usize, usize, usize, usize) {
    let mut c = [0usize; 9];
    for cell in &fx.world.cells {
        let i = match cell.terrain {
            Terrain::DeepWater => 0,
            Terrain::ShallowWater => 1,
            Terrain::Sand => 2,
            Terrain::Dirt => 3,
            Terrain::GrassSparse => 4,
            Terrain::Grass => 5,
            Terrain::GrassDense => 6,
            Terrain::Forest => 7,
            Terrain::Rock => 8,
        };
        c[i] += 1;
    }
    (c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7], c[8])
}

fn preview(f: &mut Frame, area: Rect, fx: &Fixtures) {
    let inner = panel::draw_with_hint(f, area, "Preview", "seed 0xC0FFEE, 150x40 at 1:2", panel::Kind::Outer);
    let world = &fx.world;
    // 2:1 downsampled terrain: every second column and row.
    let px = inner.x + (inner.width - PREVIEW_W) / 2;
    let py = inner.y + 1;
    {
        let buf = f.buffer_mut();
        for sy in 0..PREVIEW_H {
            for sx in 0..PREVIEW_W {
                let (wx, wy) = (sx as usize * 2, sy as usize * 2);
                if wx >= world.width() || wy >= world.height() {
                    continue;
                }
                let cell = world.cell(wx, wy);
                let (g, fg, bg) = map::terrain_cell(cell, false);
                if let Some(c) = buf.cell_mut((px + sx, py + sy)) {
                    c.set_char(g);
                    c.set_style(Style::default().fg(fg).bg(bg));
                }
            }
        }
        // Region labels overlaid on the preview.
        for r in &world.regions {
            let (name, x0, y0, x1, y1) = (&r.0, r.1, r.2, r.3, r.4);
            let cx = px + ((x0 + x1) / 4) as u16;
            let cy = py + ((y0 + y1) / 4) as u16;
            let w = name.chars().count() as u16;
            let x = cx.saturating_sub(w / 2).max(px).min(px + PREVIEW_W - w);
            buf.set_stringn(x, cy, name.as_str(), w as usize, Style::default().fg(theme::TEXT_BRIGHT).bg(theme::dim(theme::BG, 0.0)).add_modifier(Modifier::BOLD));
        }
        // Frame corners around the preview.
        let frame_style = theme::border();
        buf.set_stringn(px - 1, py - 1, "┌", 1, frame_style);
        buf.set_stringn(px + PREVIEW_W, py - 1, "┐", 1, frame_style);
        buf.set_stringn(px - 1, py + PREVIEW_H, "└", 1, frame_style);
        buf.set_stringn(px + PREVIEW_W, py + PREVIEW_H, "┘", 1, frame_style);
        let hline: String = std::iter::repeat_n(glyphs::H_LINE, PREVIEW_W as usize).collect();
        buf.set_stringn(px, py - 1, &hline, PREVIEW_W as usize, frame_style);
        buf.set_stringn(px, py + PREVIEW_H, &hline, PREVIEW_W as usize, frame_style);
        for y in py..py + PREVIEW_H {
            buf.set_stringn(px - 1, y, glyphs::V_LINE.to_string(), 1, frame_style);
            buf.set_stringn(px + PREVIEW_W, y, glyphs::V_LINE.to_string(), 1, frame_style);
        }
    }
    let mut row = PREVIEW_H + 3;

    panel::section(f, inner, row, "Terrain summary");
    row += 1;
    let c = terrain_counts(fx);
    let total = world.cells.len() as f32;
    let groups: Vec<(&str, char, ratatui::style::Color, usize)> = vec![
        ("water", glyphs::DEEP_WATER, theme::SHALLOW_FG, c.0 + c.1),
        ("sand / dirt", glyphs::SAND, theme::SAND_FG, c.2 + c.3),
        ("grassland", glyphs::GRASS, theme::GRASS_FG, c.4 + c.5),
        ("meadow", glyphs::GRASS_DENSE, theme::GRASS_DENSE_FG, c.6),
        ("forest", glyphs::FOREST, theme::FOREST_FG, c.7),
        ("rock", glyphs::ROCK, theme::ROCK_FG, c.8),
    ];
    let half = inner.width / 2;
    for (i, (name, g, color, n)) in groups.iter().enumerate() {
        let col = (i % 2) as u16;
        let r = row + (i / 2) as u16;
        let x = inner.x + 1 + col * half;
        let y = inner.y + r;
        let buf = f.buffer_mut();
        let frac = *n as f32 / total;
        buf.set_stringn(x, y, format!("{} ", g), 2, Style::default().fg(*color).bg(theme::PANEL_BG));
        buf.set_stringn(x + 2, y, format!("{:<12}", name), 12, theme::text());
        bars::bar(buf, x + 14, y, 14, frac, *color);
        buf.set_stringn(x + 29, y, format!("{:>3}% {:>4}", (frac * 100.0).round() as u32, n), 9, theme::dim_text());
    }
    row += 3;
    let land = total as usize - c.0 - c.1;
    let walkable = land - c.8;
    let veg: f32 = world.cells.iter().map(|c| c.vegetation).sum::<f32>() / total;
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" land {}%  walkable {} cells  mean vegetation {:.2}  regions {}  dens {}  seeds {}", (land as f32 / total * 100.0).round() as u32, walkable, veg, world.regions.len(), world.dens.len(), world.seeds.len()), theme::dim_text()),
    ]));
    row += 2;

    panel::section(f, inner, row, "Carrying capacity estimate");
    row += 1;
    let veg_cells = c.4 + c.5 + c.6 + c.7;
    let prey_cap = (veg_cells as f32 * 0.35) as u32;
    let pred_cap = prey_cap / 8;
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" forage cells {}  ", veg_cells), theme::text()),
        sp(format!("{} supports about {} prey and {} predators at normal rainfall", glyphs::RIGHT, prey_cap, pred_cap), theme::dim_text()),
    ]));
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        sp(" starting 510 prey / 66 predators: ", theme::text()),
        sp("within capacity", Style::default().fg(theme::GOOD).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp(format!("  ({}% / {}% of capacity)", 510 * 100 / prey_cap.max(1), 66 * 100 / pred_cap.max(1)), theme::dim_text()),
    ]));
    row += 2;
    util::line(f, inner, row, Line::from(vec![
        sp(format!(" {} ", glyphs::ALERT), Style::default().fg(theme::WARN).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)),
        sp("Sunfall Coast has little forage; lynx placed there tend to starve early.", theme::dim_text()),
    ]));
}
