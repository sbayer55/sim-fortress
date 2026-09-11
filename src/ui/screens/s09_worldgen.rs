//! S09: the functional world-generation form with a live preview.

use std::cell::RefCell;
use std::collections::BTreeMap;

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::params::{CreaturesParams, Difficulty, EvolutionParams, Rainfall, TimeParams, WorldParams};
use crate::sim::{Params, Sim, SpeciesId, World};
use crate::ui::app::AppState;
use crate::ui::screens::s01_map::WorldMap;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::SpeciesStyle;
use crate::widgets::map;
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

const FIELD_COUNT: usize = 27;
const FORM_W: u16 = 66;

pub struct WorldGen {
    form: RefCell<WorldGenForm>,
}

struct WorldGenForm {
    name: String,
    seed_text: String,
    world: WorldParams,
    season_days: u32,
    counts: [u32; 6],
    evolution: EvolutionParams,
    focus: usize,
    preview: World,
    dirty: bool,
    text_edited: bool,
}

impl WorldGenForm {
    fn new(params: Params) -> Self {
        let world = params.world.clone();
        let preview = World::generate(parse_seed("0xC0FFEE").unwrap_or(0), &world);
        let mut counts = [0u32; 6];
        for (i, id) in SpeciesId::ALL.iter().enumerate() {
            counts[i] = params.creatures.initial_counts.get(id).copied().unwrap_or(0);
        }
        WorldGenForm {
            name: "The Valley of Sunfall".to_string(),
            seed_text: "0xC0FFEE".to_string(),
            world,
            season_days: params.time.season_days,
            counts,
            evolution: params.evolution.clone(),
            // Start on Size (Width), matching the prototype's default highlight and
            // so a fresh S09 quits on `q` (FR8: `q` quits when no text field is focused).
            focus: 2,
            preview,
            dirty: false,
            text_edited: false,
        }
    }

    fn seed(&self) -> Option<u64> {
        parse_seed(&self.seed_text)
    }

    fn build_params(&self) -> Params {
        Params {
            world: self.world.clone(),
            time: TimeParams { season_days: self.season_days, ..TimeParams::default() },
            creatures: CreaturesParams {
                initial_counts: SpeciesId::ALL
                    .iter()
                    .enumerate()
                    .map(|(i, id)| (*id, self.counts[i]))
                    .collect::<BTreeMap<_, _>>(),
            },
            evolution: self.evolution.clone(),
            ..Params::default()
        }
    }

    fn regenerate_if_needed(&mut self) {
        if self.dirty {
            let seed = self.seed().unwrap_or(0);
            self.preview = World::generate(seed, &self.world);
            self.dirty = false;
        }
    }
}

impl Default for WorldGen {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldGen {
    pub fn new() -> Self {
        Self::from_params(Params::default())
    }

    pub fn from_params(params: Params) -> Self {
        WorldGen { form: RefCell::new(WorldGenForm::new(params)) }
    }
}

fn generate(form: &WorldGenForm, app: &mut AppState) -> Action {
    let Some(seed) = form.seed() else {
        return Action::None; // invalid seed — refuse
    };
    let params = form.build_params();
    let name = form.name.clone();
    app.params = params.clone();
    app.sim = Some(Sim::new(seed, params));
    Action::Replace(Box::new(WorldMap::new(name)))
}

impl Screen for WorldGen {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        let mut form = self.form.borrow_mut();
        let code = key.code;

        if code == KeyCode::Tab || code == KeyCode::BackTab {
            let n = if code == KeyCode::BackTab { FIELD_COUNT - 1 } else { 1 };
            form.focus = (form.focus + n) % FIELD_COUNT;
            form.text_edited = false;
            return Action::None;
        }

        if code == KeyCode::Esc {
            return if app.sim.is_some() { Action::Pop } else { Action::Quit };
        }

        let focus = form.focus;

        // Text fields (0 = name, 1 = seed) consume printable keys and caret keys.
        if focus == 0 || focus == 1 {
            let is_name = focus == 0;
            match code {
                KeyCode::Char(c) if !c.is_control() => {
                    let max = if is_name { 23 } else { 20 };
                    if !form.text_edited {
                        form.text_edited = true;
                        if is_name {
                            form.name.clear();
                        } else {
                            form.seed_text.clear();
                        }
                    }
                    let full = if is_name { form.name.chars().count() >= max } else { form.seed_text.chars().count() >= max };
                    if !full {
                        if is_name {
                            form.name.push(c);
                        } else {
                            form.seed_text.push(c);
                        }
                    }
                    if !is_name {
                        form.dirty = true;
                    }
                    return Action::None;
                }
                KeyCode::Backspace => {
                    if is_name {
                        form.name.pop();
                    } else {
                        form.seed_text.pop();
                    }
                    form.text_edited = true;
                    if !is_name {
                        form.dirty = true;
                    }
                    return Action::None;
                }
                _ => return Action::None,
            }
        }

        // Buttons.
        if focus == 24 || focus == 25 || focus == 26 {
            return match (focus, code) {
                (24, KeyCode::Enter) => generate(&form, app),
                (25, KeyCode::Enter) => {
                    let s = rand_seed();
                    form.seed_text = format!("{s}");
                    form.text_edited = true;
                    form.dirty = true;
                    Action::None
                }
                (26, KeyCode::Enter) => {
                    if app.sim.is_some() {
                        Action::Pop
                    } else {
                        Action::Quit
                    }
                }
                _ => Action::None,
            };
        }

        // Presets (19..24) are inert until C6.
        if (19..24).contains(&focus) {
            return Action::None;
        }

        // 'q' with no text focus quits.
        if code == KeyCode::Char('q') {
            return Action::Quit;
        }

        // Adjustable fields respond to Left/Right; Enter generates.
        match code {
            KeyCode::Left | KeyCode::Right => {
                let dir: i32 = if code == KeyCode::Right { 1 } else { -1 };
                adjust(&mut form, focus, dir);
                form.dirty = true;
                Action::None
            }
            KeyCode::Enter => generate(&form, app),
            _ => Action::None,
        }
    }

    fn render(&self, _app: &AppState, f: &mut Frame, area: Rect) {
        let mut form = self.form.borrow_mut();
        form.regenerate_if_needed();

        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let form_area = Rect::new(area.x, area.y, FORM_W, body_h);
        let preview_area = Rect::new(area.x + FORM_W, area.y, area.width.saturating_sub(FORM_W), body_h);

        form_panel(f, form_area, &form);
        preview_panel(f, preview_area, &form);

        let seed_hint = if form.seed().is_some() { format!("seed {}  preview is live", form.seed_text) } else { "invalid seed".to_string() };
        status::render(
            f,
            Rect::new(area.x, status_row, area.width, 1),
            &[("Tab", "next field"), ("←→", "adjust"), ("Enter", "generate"), ("Esc", "back")],
            &seed_hint,
        );
    }
}

fn adjust(form: &mut WorldGenForm, focus: usize, dir: i32) {
    let d = dir as i64;
    let w = &mut form.world;
    match focus {
        2 => w.width = clamp_i64(w.width as i64 + d * 10, 100, 200) as usize,
        3 => w.height = clamp_i64(w.height as i64 + d * 5, 30, 60) as usize,
        4 => w.water_pct = clamp_i64(w.water_pct as i64 + d, 0, 60) as u8,
        5 => w.forest_pct = clamp_i64(w.forest_pct as i64 + d, 0, 50) as u8,
        6 => w.rock_pct = clamp_i64(w.rock_pct as i64 + d, 0, 30) as u8,
        7 => cycle_rainfall(&mut w.rainfall, dir),
        8 => form.season_days = clamp_i64(form.season_days as i64 + d * 10, 30, 180) as u32,
        9..=14 => {
            let i = focus - 9;
            form.counts[i] = clamp_i64(form.counts[i] as i64 + d * 10, 0, 999) as u32;
        }
        15 => form.evolution.mutation_rate = clamp_f32(form.evolution.mutation_rate + dir as f32 * 0.01, 0.0, 0.2),
        16 => form.evolution.mutation_strength = clamp_f32(form.evolution.mutation_strength + dir as f32 * 0.01, 0.0, 0.2),
        17 => cycle_difficulty(&mut form.evolution.predation_difficulty, dir),
        18 => form.evolution.regrowth_rate = clamp_f32(form.evolution.regrowth_rate + dir as f32 * 0.1, 0.2, 2.0),
        _ => {}
    }
}

fn clamp_i64(v: i64, lo: i64, hi: i64) -> i64 {
    v.clamp(lo, hi)
}

fn clamp_f32(v: f32, lo: f32, hi: f32) -> f32 {
    v.clamp(lo, hi)
}

fn cycle_rainfall(r: &mut Rainfall, dir: i32) {
    let all = [Rainfall::Dry, Rainfall::Normal, Rainfall::Wet];
    let i = all.iter().position(|x| x == r).unwrap_or(1) as i32;
    *r = all[((i + dir).rem_euclid(3)) as usize];
}

fn cycle_difficulty(d: &mut Difficulty, dir: i32) {
    let all = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];
    let i = all.iter().position(|x| x == d).unwrap_or(1) as i32;
    *d = all[((i + dir).rem_euclid(3)) as usize];
}

fn parse_seed(s: &str) -> Option<u64> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        t.parse::<u64>().ok()
    }
}

fn rand_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0xC0FFEE) | 1
}

// ------------------------------------------------------------------ form

fn field(label: &'static str, value: impl Into<String>, adjustable: bool, hint: &'static str) -> (String, String, bool, &'static str) {
    (label.to_string(), value.into(), adjustable, hint)
}

fn draw_field(f: &mut Frame, area: Rect, row: u16, label: &str, value: &str, adjustable: bool, hint: &str, focused: bool) {
    if row >= area.height {
        return;
    }
    let y = area.y + row;
    let buf = f.buffer_mut();
    let label_style = if focused { theme::label().add_modifier(Modifier::BOLD) } else { theme::text() };
    buf.set_stringn(area.x + 1, y, format!("{:<20}", label), 20, label_style);
    let box_w = 26u16;
    let bx = area.x + 21;
    let (l, r) = if adjustable { (glyphs::REWIND, glyphs::PLAY) } else { ('[', ']') };
    let arrow_style = if focused {
        Style::default().fg(theme::KEY).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)
    } else {
        theme::dim_text()
    };
    buf.set_stringn(bx, y, l.to_string(), 1, arrow_style);
    let val = format!(" {:<w$}", value, w = box_w as usize - 3);
    let val_style = if focused { theme::selected() } else { Style::default().fg(theme::TEXT_BRIGHT).bg(theme::BG) };
    buf.set_stringn(bx + 1, y, &val, box_w as usize - 2, val_style);
    buf.set_stringn(bx + box_w - 1, y, r.to_string(), 1, arrow_style);
    buf.set_stringn(bx + box_w + 1, y, hint, (area.width.saturating_sub(box_w + 22)) as usize, theme::dim_text());
}

fn form_panel(f: &mut Frame, area: Rect, form: &WorldGenForm) {
    let hint = format!("field {} of {}", form.focus + 1, FIELD_COUNT);
    let inner = panel::draw_with_hint(f, area, "New World", &hint, panel::Kind::Outer);
    let mut row = 0u16;

    panel::section(f, inner, row, "World");
    row += 1;
    let wf = [
        field("World name", form.name.clone(), false, "text"),
        field("Seed", form.seed_text.clone(), false, "hex/decimal"),
        field("Size", format!("{} x {}", form.world.width, form.world.height), true, "w x h cells"),
        field("Water %", form.world.water_pct.to_string(), true, "lakes + rivers"),
        field("Forest %", form.world.forest_pct.to_string(), true, "predator cover"),
        field("Rock %", form.world.rock_pct.to_string(), true, "impassable"),
        field("Rainfall", rainfall_name(form.world.rainfall).to_string(), true, "dry/normal/wet"),
        field("Season length", format!("{} days", form.season_days), true, "30 - 180 days"),
    ];
    for (i, (label, value, adjustable, hint)) in wf.iter().enumerate() {
        draw_field(f, inner, row, label.as_str(), value.as_str(), *adjustable, hint, i == form.focus);
        row += 1;
    }
    row += 1;

    panel::section(f, inner, row, "Initial species");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        Span::styled("   species    kind       count   ", theme::dim_text()),
        Span::styled("share of starting population", theme::dim_text()),
    ]));
    row += 1;
    let total: u32 = form.counts.iter().sum();
    for (i, id) in SpeciesId::ALL.iter().enumerate() {
        let focused = form.focus == 9 + i;
        let y = inner.y + row;
        let buf = f.buffer_mut();
        let bg = if focused { theme::SELECT_BG } else { theme::PANEL_BG };
        if focused {
            for x in inner.x..inner.right() {
                if let Some(c) = buf.cell_mut((x, y)) {
                    c.set_bg(bg);
                }
            }
        }
        let base = if focused { theme::selected() } else { theme::text() };
        buf.set_stringn(inner.x + 1, y, format!("{} ", id.glyph().to_ascii_uppercase()), 2, Style::default().fg(id.color()).bg(bg).add_modifier(Modifier::BOLD));
        buf.set_stringn(inner.x + 3, y, format!("{:<10}", id.plural()), 10, base);
        let kind = if id.kind() == crate::sim::Kind::Prey { "prey" } else { "predator" };
        buf.set_stringn(inner.x + 13, y, format!("{:<9}", kind), 9, Style::default().fg(theme::DIM).bg(bg));
        let arrows = if focused { Style::default().fg(theme::KEY).bg(bg).add_modifier(Modifier::BOLD) } else { Style::default().fg(theme::DIM).bg(bg) };
        buf.set_stringn(inner.x + 22, y, glyphs::REWIND.to_string(), 1, arrows);
        buf.set_stringn(inner.x + 23, y, format!("{:>5} ", form.counts[i]), 6, base);
        buf.set_stringn(inner.x + 29, y, glyphs::PLAY.to_string(), 1, arrows);
        let share = form.counts[i] as f32 / total.max(1) as f32;
        bars::bar(buf, inner.x + 33, y, 22, share, id.color());
        buf.set_stringn(inner.x + 56, y, format!("{:>3}%", (share * 100.0).round() as u32), 4, Style::default().fg(theme::TEXT).bg(bg));
        row += 1;
    }
    util::line(f, inner, row, Line::from(vec![
        Span::styled(format!("   total {}   prey {}   predators {}", total, prey_total(&form.counts), pred_total(&form.counts)), theme::dim_text()),
    ]));
    row += 2;

    panel::section(f, inner, row, "Evolution");
    row += 1;
    let evo = [
        field("Mutation rate", format!("{:.2}", form.evolution.mutation_rate), true, "per trait/birth"),
        field("Mutation strength", format!("{:.2}", form.evolution.mutation_strength), true, "mutation sd"),
        field("Predation difficulty", difficulty_name(form.evolution.predation_difficulty).to_string(), true, "easy/norm/hard"),
        field("Regrowth rate", format!("{:.1}", form.evolution.regrowth_rate), true, "veg multiplier"),
    ];
    for (i, (label, value, adjustable, hint)) in evo.iter().enumerate() {
        draw_field(f, inner, row, label.as_str(), value.as_str(), *adjustable, hint, form.focus == 15 + i);
        row += 1;
    }
    row += 2;

    panel::section(f, inner, row, "Presets");
    row += 1;
    let presets: [(&str, &str); 5] = [
        ("Balanced", "default values, gentle seasons"),
        ("Harsh winter", "180-day seasons, regrowth 0.6"),
        ("Lush", "forest 30%, regrowth 1.4, predation hard"),
        ("Archipelago", "water 55%, islands isolate lineages"),
        ("Fast evolution", "mutation rate 0.10, strength 0.12"),
    ];
    for (i, (name, desc)) in presets.iter().enumerate() {
        let focused = form.focus == 19 + i;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} ", glyphs::DOT), if focused { theme::label() } else { theme::dim_text() }),
            Span::styled(format!("{:<16}", name), if focused { theme::title() } else { theme::text() }),
            Span::styled(*desc, theme::dim_text()),
        ]));
        row += 1;
    }

    // Buttons pinned to the bottom.
    let brow = inner.height - 1;
    let y = inner.y + brow;
    let buf = f.buffer_mut();
    let buttons: [(&str, bool, usize); 3] = [("[ Generate ]", true, 24), ("[ Randomize seed ]", false, 25), ("[ Back ]", false, 26)];
    let mut x = inner.x + 4;
    for (label, primary, fi) in buttons {
        let focused = form.focus == fi;
        let st = if primary {
            Style::default().fg(theme::CURSOR_FG).bg(if focused { theme::ACCENT } else { theme::WARN }).add_modifier(Modifier::BOLD)
        } else if focused {
            Style::default().fg(theme::CURSOR_FG).bg(theme::ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::TEXT_BRIGHT).bg(theme::HEADER_BG)
        };
        buf.set_stringn(x, y, label, label.len(), st);
        x += label.len() as u16 + 3;
    }
}

fn prey_total(c: &[u32; 6]) -> u32 {
    c[0] + c[1] + c[2]
}
fn pred_total(c: &[u32; 6]) -> u32 {
    c[3] + c[4] + c[5]
}

fn rainfall_name(r: Rainfall) -> &'static str {
    match r {
        Rainfall::Dry => "dry",
        Rainfall::Normal => "normal",
        Rainfall::Wet => "wet",
    }
}
fn difficulty_name(d: Difficulty) -> &'static str {
    match d {
        Difficulty::Easy => "easy",
        Difficulty::Normal => "normal",
        Difficulty::Hard => "hard",
    }
}

// ---------------------------------------------------------------- preview

fn preview_panel(f: &mut Frame, area: Rect, form: &WorldGenForm) {
    let world = &form.preview;
    let scale = (2..=4).find(|&k| world.width().div_ceil(k) <= 75 && world.height().div_ceil(k) <= 20).unwrap_or(4);
    let hint = format!("seed {}, {}x{} at 1:{}", form.seed_text, world.width(), world.height(), scale);
    let inner = panel::draw_with_hint(f, area, "Preview", &hint, panel::Kind::Outer);

    let iw = world.width().div_ceil(scale) as u16;
    let ih = world.height().div_ceil(scale) as u16;
    let px = inner.x + (inner.width.saturating_sub(iw)) / 2;
    let py = inner.y + 1;
    {
        let buf = f.buffer_mut();
        for sy in 0..ih {
            for sx in 0..iw {
                let wx = (sx as usize * scale).min(world.width() - 1);
                let wy = (sy as usize * scale).min(world.height() - 1);
                let cell = world.cell(wx, wy);
                let (g, fg, bg) = map::terrain_cell(cell, false);
                if let Some(c) = buf.cell_mut((px + sx, py + sy)) {
                    c.set_char(g);
                    c.set_style(Style::default().fg(fg).bg(bg));
                }
            }
        }
    }
    let mut row = ih + 3;

    panel::section(f, inner, row, "Terrain summary");
    row += 1;
    let c = terrain_counts(world);
    let total = world.cells.len().max(1) as f32;
    let groups: Vec<(&str, char, ratatui::style::Color, usize)> = vec![
        ("water", glyphs::DEEP_WATER, theme::SHALLOW_FG, c[0] + c[1]),
        ("sand / dirt", glyphs::SAND, theme::SAND_FG, c[2] + c[3]),
        ("grassland", glyphs::GRASS, theme::GRASS_FG, c[4] + c[5]),
        ("meadow", glyphs::GRASS_DENSE, theme::GRASS_DENSE_FG, c[6]),
        ("forest", glyphs::FOREST, theme::FOREST_FG, c[7]),
        ("rock", glyphs::ROCK, theme::ROCK_FG, c[8]),
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
    row += 4;

    let forage = c[4] + c[5] + c[6] + c[7];
    let prey_cap = (forage as f32 * 0.35) as u32;
    let pred_cap = prey_cap / 8;
    util::line(f, inner, row, Line::from(vec![
        Span::styled(format!(" forage cells {}  ", forage), theme::text()),
        Span::styled(format!("{} supports about {} prey and {} predators", glyphs::RIGHT, prey_cap, pred_cap), theme::dim_text()),
    ]));
}

fn terrain_counts(world: &World) -> [usize; 9] {
    let mut c = [0usize; 9];
    for cell in &world.cells {
        c[cell.terrain as usize] += 1;
    }
    c
}
