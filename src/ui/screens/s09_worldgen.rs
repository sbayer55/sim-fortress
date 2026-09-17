//! S09: the functional world-generation form with a live preview.

use std::cell::RefCell;

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::sim::params::{Difficulty, EcologyParams, GeneticsParams, PredationParams, Rainfall, Roster, TimeParams, WorldParams};
use crate::sim::{Params, Sim, World, PRESETS};
use crate::ui::app::AppState;
use crate::ui::screens::s01_map::WorldMap;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::SpeciesStyle;
use crate::widgets::{bars, panel, status, util};
use crate::{glyphs, theme};

// Focus indices in Tab order. The world-shape fields are plain adjustable
// fields (Left/Right), so Map width and Map height each get their own row.
const F_NAME: usize = 0;
const F_SEED: usize = 1;
const F_WIDTH: usize = 2;
const F_HEIGHT: usize = 3;
const F_WATER: usize = 4;
const F_FOREST: usize = 5;
const F_ROCK: usize = 6;
const F_AGE: usize = 7;
const F_RAINFALL: usize = 8;
const F_SEASON: usize = 9;
const F_SPECIES: usize = 10; // one row per roster species, then the tail fields
/// Tail fields sit after the species rows: `form.tail() + T_*`.
const T_MUTATION_RATE: usize = 0;
const T_MUTATION_STRENGTH: usize = 1;
const T_DIFFICULTY: usize = 2;
const T_REGROWTH: usize = 3;
const T_PRESETS: usize = 4; // 4..=8, five presets
const T_GENERATE: usize = 9;
const T_RANDOMIZE: usize = 10;
const T_BACK: usize = 11;
/// The species-designer button (C9); present only while the feature is on.
const T_DESIGN: usize = 12;
const T_COUNT: usize = 13;
const FORM_W: u16 = 66;

#[cfg(feature = "ai")]
mod designer;
mod preview;
#[cfg(test)]
mod tests;
use preview::preview_panel;

#[derive(Debug)]
pub struct WorldGen {
    form: RefCell<WorldGenForm>,
}

#[derive(Debug)]
struct WorldGenForm {
    name: String,
    seed_text: String,
    world: WorldParams,
    season_days: u32,
    /// The roster the form was opened with; `counts` overrides its founders.
    species: Roster,
    counts: Vec<u32>,
    genetics: GeneticsParams,
    predation: PredationParams,
    regrowth_rate: f32,
    focus: usize,
    preview: World,
    dirty: bool,
    text_edited: bool,
    /// Typed replacement for the focused numeric field (Space opens it,
    /// Enter applies it, Esc cancels it).
    edit: Option<String>,
    /// The designer feature is on, so the `[ Design species ]` button exists.
    designer: bool,
}

impl WorldGenForm {
    /// Focus index of the first tail field (after the species rows).
    fn tail(&self) -> usize {
        F_SPECIES + self.counts.len()
    }

    fn field_count(&self) -> usize {
        self.tail() + T_COUNT - usize::from(!self.designer)
    }

    /// Apply a validated `[[species]]` overlay (C9 designer): the roster and
    /// the founder counts are replaced, the focus is clamped because the tail
    /// indices move. The preview never reads the roster, so `dirty` is untouched.
    fn apply_species_overlay(&mut self, overlay: &str) -> Result<(), String> {
        let mut p = self.build_params();
        p.apply_overlay(overlay)?;
        self.species = p.species;
        self.counts = self.species.0.iter().map(|s| s.initial_count).collect();
        self.focus = self.focus.min(self.field_count().saturating_sub(1));
        Ok(())
    }

    /// Refresh the designer flag from the handle and keep the focus in range.
    fn sync_designer(&mut self, app: &AppState) {
        self.designer = app.ai.feature_on(crate::ai::Feature::Designer);
        self.focus = self.focus.min(self.field_count().saturating_sub(1));
    }

    /// The focused text field: `name` at focus 0, `seed_text` at focus 1.
    const fn field_mut(&mut self, is_name: bool) -> &mut String {
        if is_name {
            &mut self.name
        } else {
            &mut self.seed_text
        }
    }

    fn new(params: &Params) -> Self {
        let world = params.world.clone();
        let preview = World::generate(parse_seed("0xC0FFEE").unwrap_or(0), &world);
        let counts = params.species.0.iter().map(|s| s.initial_count).collect();
        Self {
            name: "The Valley of Sunfall".to_string(),
            seed_text: "0xC0FFEE".to_string(),
            world,
            season_days: params.time.season_days,
            species: params.species.clone(),
            counts,
            genetics: params.genetics.clone(),
            predation: params.predation.clone(),
            regrowth_rate: params.ecology.regrowth_rate,
            // Start on Map width, matching the documented default highlight and
            // so a fresh S09 quits on `q` (FR8: `q` quits when no text field is focused).
            focus: F_WIDTH,
            preview,
            dirty: false,
            text_edited: false,
            edit: None,
            designer: false,
        }
    }

    fn seed(&self) -> Option<u64> {
        parse_seed(&self.seed_text)
    }

    fn build_params(&self) -> Params {
        Params {
            world: self.world.clone(),
            time: TimeParams { season_days: self.season_days, ..TimeParams::default() },
            species: {
                let mut species = self.species.clone();
                for (s, &n) in species.0.iter_mut().zip(&self.counts) {
                    s.initial_count = n;
                }
                species
            },
            genetics: self.genetics.clone(),
            ecology: EcologyParams { regrowth_rate: self.regrowth_rate, ..EcologyParams::default() },
            predation: self.predation.clone(),
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
        Self::from_params(&Params::default())
    }

    pub fn from_params(params: &Params) -> Self {
        Self { form: RefCell::new(WorldGenForm::new(params)) }
    }

    /// The parameters the form would generate with right now (used by tests).
    #[cfg(test)]
    pub fn form_params(&self) -> Params {
        self.form.borrow().build_params()
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
    app.world_name = Some(name.clone());
    app.last_saved_tick = None;
    app.viewport_origin = (0, 0);
    Action::Replace(Box::new(WorldMap::new(name)))
}

impl Screen for WorldGen {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        let mut form = self.form.borrow_mut();
        let code = key.code;
        form.sync_designer(app);
        if let Some(overlay) = app.pending_species_overlay.take() {
            if let Err(e) = form.apply_species_overlay(&overlay) {
                eprintln!("designer overlay error: {e}");
            }
        }

        // Typed numeric entry (opened with Space) captures every key until it
        // is applied (Enter/Tab) or cancelled (Esc).
        if form.edit.is_some() {
            handle_typed_entry(&mut form, code);
            return Action::None;
        }

        if code == KeyCode::Tab || code == KeyCode::BackTab {
            form.focus = (form.focus + field_step(&form, code)) % form.field_count();
            form.text_edited = false;
            return Action::None;
        }

        if code == KeyCode::Esc {
            return Action::Pop; // back to the title screen (C6 title flow)
        }

        let focus = form.focus;

        if focus == F_NAME || focus == F_SEED {
            return handle_text_field(&mut form, focus, code);
        }

        let tail = form.tail();
        if focus >= tail + T_GENERATE {
            return match (focus - tail, code) {
                (T_GENERATE, KeyCode::Enter) => generate(&form, app),
                (T_RANDOMIZE, KeyCode::Enter) => {
                    let s = rand_seed();
                    form.seed_text = format!("{s}");
                    form.text_edited = true;
                    form.dirty = true;
                    Action::None
                }
                (T_BACK, KeyCode::Enter) => Action::Pop,
                (T_DESIGN, KeyCode::Enter) => open_designer(&form),
                _ => Action::None,
            };
        }

        if focus >= tail + T_PRESETS {
            if code == KeyCode::Enter {
                apply_preset(&mut form, focus - tail - T_PRESETS);
            }
            return Action::None;
        }

        if code == KeyCode::Char('q') {
            return Action::Pop;
        }

        if code == KeyCode::Char(' ') && is_numeric_field(&form, focus) {
            form.edit = Some(String::new());
            return Action::None;
        }

        handle_adjust(&mut form, focus, code, app)
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let mut form = self.form.borrow_mut();
        form.sync_designer(app);
        form.regenerate_if_needed();

        let status_row = area.y + area.height - 1;
        let body_h = area.height - 1;
        let form_area = Rect::new(area.x, area.y, FORM_W, body_h);
        let preview_area = Rect::new(area.x + FORM_W, area.y, area.width.saturating_sub(FORM_W), body_h);

        form_panel(f, form_area, &form);
        preview_panel(f, preview_area, &form);

        let seed_hint = if form.seed().is_some() { format!("seed {}  preview is live", form.seed_text) } else { "invalid seed".to_string() };
        if form.edit.is_some() {
            let hint = match form.focus {
                F_WIDTH => "type a width in cells",
                F_HEIGHT => "type a height in cells",
                _ => "type a number",
            };
            status::render(
                f,
                Rect::new(area.x, status_row, area.width, 1),
                &[("0-9", "type value"), ("Enter", "apply"), ("Esc", "cancel")],
                hint,
            );
        } else {
            status::render(
                f,
                Rect::new(area.x, status_row, area.width, 1),
                &[("Tab", "next field"), ("←→", "adjust"), ("Space", "type value"), ("Enter", "generate"), ("Esc", "back")],
                &seed_hint,
            );
        }
    }
}

#[cfg(feature = "ai")]
fn open_designer(form: &WorldGenForm) -> Action {
    Action::Push(Box::new(designer::Designer::new(form.build_params())))
}

#[cfg(not(feature = "ai"))]
const fn open_designer(_form: &WorldGenForm) -> Action {
    Action::None
}

/// Apply a preset (C6 FR7): write its values into the form fields. Balanced
/// (index 0) resets those fields to the defaults; the name, seed and species
/// counts are never touched by presets.
fn apply_preset(form: &mut WorldGenForm, idx: usize) {
    let preset = &PRESETS[idx];
    if idx == 0 {
        let d = Params::default();
        form.world = d.world;
        form.season_days = d.time.season_days;
        form.genetics = d.genetics;
        form.predation = d.predation;
        form.regrowth_rate = d.ecology.regrowth_rate;
    } else {
        let mut p = form.build_params();
        if let Err(e) = p.apply_overlay(preset.overlay) {
            eprintln!("preset error: {e}");
            return;
        }
        form.world = p.world;
        form.season_days = p.time.season_days;
        form.genetics = p.genetics;
        form.predation = p.predation;
        form.regrowth_rate = p.ecology.regrowth_rate;
    }
    form.dirty = true;
}

fn adjust(form: &mut WorldGenForm, focus: usize, dir: i32) {
    let d = i64::from(dir);
    let w = &mut form.world;
    match focus {
        // Left/Right adjust the width and height fields, like every other field.
        F_WIDTH => w.width = crate::cast!(clamp_i64(crate::cast!(w.width => i64) + d * 10, 100, 1000) => usize),
        F_HEIGHT => w.height = crate::cast!(clamp_i64(crate::cast!(w.height => i64) + d * 5, 30, 1000) => usize),
        F_WATER => w.water_pct = crate::cast!(clamp_i64(i64::from(w.water_pct) + d, 0, 60) => u8),
        F_FOREST => w.forest_pct = crate::cast!(clamp_i64(i64::from(w.forest_pct) + d, 0, 50) => u8),
        F_ROCK => w.rock_pct = crate::cast!(clamp_i64(i64::from(w.rock_pct) + d, 0, 30) => u8),
        F_AGE => w.age = crate::cast!(clamp_i64(i64::from(w.age) + d, 0, 30) => u8),
        F_RAINFALL => cycle_rainfall(&mut w.rainfall, dir),
        F_SEASON => form.season_days = crate::cast!(clamp_i64(i64::from(form.season_days) + d * 10, 30, 180) => u32),
        F_SPECIES.. if focus < form.tail() => {
            let i = focus - F_SPECIES;
            form.counts[i] = crate::cast!(clamp_i64(i64::from(form.counts[i]) + d * 10, 0, 999) => u32);
        }
        _ => match focus - form.tail() {
            T_MUTATION_RATE => form.genetics.mutation_rate = clamp_f32(form.genetics.mutation_rate + crate::cast!(dir => f32) * 0.01, 0.0, 0.2),
            T_MUTATION_STRENGTH => form.genetics.mutation_strength = clamp_f32(form.genetics.mutation_strength + crate::cast!(dir => f32) * 0.01, 0.0, 0.2),
            T_DIFFICULTY => cycle_difficulty(&mut form.predation.difficulty, dir),
            T_REGROWTH => form.regrowth_rate = clamp_f32(form.regrowth_rate + crate::cast!(dir => f32) * 0.1, 0.2, 2.0),
            _ => {}
        },
    }
}

/// Fields whose value can be typed in directly (Space opens the entry).
fn is_numeric_field(form: &WorldGenForm, focus: usize) -> bool {
    let tail = form.tail();
    matches!(focus, F_WIDTH..=F_AGE) || (F_SEASON..=tail + T_MUTATION_STRENGTH).contains(&focus) || focus == tail + T_REGROWTH
}

/// Apply a typed value to a numeric field, clamped to the same range the
/// arrow keys use. Empty or unparseable input leaves the field unchanged.
fn commit_edit(form: &mut WorldGenForm, focus: usize, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    let int = |t: &str| t.parse::<i64>().ok();
    let flt = |t: &str| t.parse::<f32>().ok().filter(|v| v.is_finite());
    let w = &mut form.world;
    let changed = match focus {
        F_WIDTH => int(text).map(|v| w.width = crate::cast!(clamp_i64(v, 100, 1000) => usize)).is_some(),
        F_HEIGHT => int(text).map(|v| w.height = crate::cast!(clamp_i64(v, 30, 1000) => usize)).is_some(),
        F_WATER => int(text).map(|v| w.water_pct = crate::cast!(clamp_i64(v, 0, 60) => u8)).is_some(),
        F_FOREST => int(text).map(|v| w.forest_pct = crate::cast!(clamp_i64(v, 0, 50) => u8)).is_some(),
        F_ROCK => int(text).map(|v| w.rock_pct = crate::cast!(clamp_i64(v, 0, 30) => u8)).is_some(),
        F_AGE => int(text).map(|v| w.age = crate::cast!(clamp_i64(v, 0, 30) => u8)).is_some(),
        F_SEASON => int(text).map(|v| form.season_days = crate::cast!(clamp_i64(v, 30, 180) => u32)).is_some(),
        F_SPECIES.. if focus < form.tail() => int(text).map(|v| form.counts[focus - F_SPECIES] = crate::cast!(clamp_i64(v, 0, 999) => u32)).is_some(),
        _ => match focus - form.tail() {
            T_MUTATION_RATE => flt(text).map(|v| form.genetics.mutation_rate = clamp_f32(v, 0.0, 0.2)).is_some(),
            T_MUTATION_STRENGTH => flt(text).map(|v| form.genetics.mutation_strength = clamp_f32(v, 0.0, 0.2)).is_some(),
            T_REGROWTH => flt(text).map(|v| form.regrowth_rate = clamp_f32(v, 0.2, 2.0)).is_some(),
            _ => false,
        },
    };
    if changed {
        form.dirty = true;
    }
}

/// The text shown in a numeric field: the typed buffer with a caret while
/// editing, otherwise the formatted value.
fn shown(form: &WorldGenForm, focus: usize, value: String) -> String {
    match &form.edit {
        Some(buf) if form.focus == focus => format!("{buf}_"),
        _ => value,
    }
}

fn clamp_i64(v: i64, lo: i64, hi: i64) -> i64 {
    v.clamp(lo, hi)
}

const fn clamp_f32(v: f32, lo: f32, hi: f32) -> f32 {
    v.clamp(lo, hi)
}

fn cycle_rainfall(r: &mut Rainfall, dir: i32) {
    let all = [Rainfall::Dry, Rainfall::Normal, Rainfall::Wet];
    let i = crate::cast!(all.iter().position(|x| x == r).unwrap_or(1) => i32);
    *r = all[crate::cast!(((i + dir).rem_euclid(3)) => usize)];
}

fn cycle_difficulty(d: &mut Difficulty, dir: i32) {
    let all = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];
    let i = crate::cast!(all.iter().position(|x| x == d).unwrap_or(1) => i32);
    *d = all[crate::cast!(((i + dir).rem_euclid(3)) => usize)];
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
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| crate::cast!(d.as_nanos() => u64)).unwrap_or(0x00C0_FFEE) | 1
}

// ------------------------------------------------------------------ form

fn field(label: &'static str, value: impl Into<String>, adjustable: bool, hint: &'static str) -> (String, String, bool, &'static str) {
    (label.to_string(), value.into(), adjustable, hint)
}

fn draw_field(f: &mut Frame<'_>, area: Rect, row: u16, label: &str, value: &str, adjustable: bool, hint: &str, focused: bool) {
    if row >= area.height {
        return;
    }
    let y = area.y + row;
    let buf = f.buffer_mut();
    let label_style = if focused { theme::label().add_modifier(Modifier::BOLD) } else { theme::text() };
    buf.set_stringn(area.x + 1, y, format!("{label:<20}"), 20, label_style);
    let box_w = 26u16;
    let bx = area.x + 21;
    let (l, r) = if adjustable { (glyphs::REWIND, glyphs::PLAY) } else { ('[', ']') };
    let arrow_style = if focused {
        Style::default().fg(theme::KEY).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD)
    } else {
        theme::dim_text()
    };
    buf.set_stringn(bx, y, l.to_string(), 1, arrow_style);
    let val = format!(" {:<w$}", value, w = crate::cast!(box_w => usize) - 3);
    let val_style = if focused { theme::selected() } else { Style::default().fg(theme::TEXT_BRIGHT).bg(theme::BG) };
    buf.set_stringn(bx + 1, y, &val, crate::cast!(box_w => usize) - 2, val_style);
    buf.set_stringn(bx + box_w - 1, y, r.to_string(), 1, arrow_style);
    buf.set_stringn(bx + box_w + 1, y, hint, crate::cast!((area.width.saturating_sub(box_w + 22)) => usize), theme::dim_text());
}

fn form_panel(f: &mut Frame<'_>, area: Rect, form: &WorldGenForm) {
    let hint = format!("field {} of {}", form.focus + 1, form.field_count());
    let inner = panel::draw_with_hint(f, area, "New World", &hint, panel::Kind::Outer);
    let row = draw_world_section(f, inner, form, 0);
    let row = draw_species_section(f, inner, form, row);
    let row = draw_evolution_section(f, inner, form, row);
    draw_presets_section(f, inner, form, row);
    draw_form_buttons(f, inner, form);
}

/// World shape and season fields.
fn draw_world_section(f: &mut Frame<'_>, inner: Rect, form: &WorldGenForm, row: u16) -> u16 {
    let mut row = row;
    panel::section(f, inner, row, "World");
    row += 1;
    let wf = [
        field("World name", form.name.clone(), false, "text"),
        field("Seed", form.seed_text.clone(), false, "hex/decimal"),
        field("Map width", shown(form, F_WIDTH, form.world.width.to_string()), true, "100 - 1000 cells"),
        field("Map height", shown(form, F_HEIGHT, form.world.height.to_string()), true, "30 - 1000 cells"),
        field("Water %", shown(form, F_WATER, form.world.water_pct.to_string()), true, "lakes + rivers"),
        field("Forest %", shown(form, F_FOREST, form.world.forest_pct.to_string()), true, "predator cover"),
        field("Rock %", shown(form, F_ROCK, form.world.rock_pct.to_string()), true, "impassable"),
        field("World age", shown(form, F_AGE, form.world.age.to_string()), true, "erosion 0 - 30"),
        field("Rainfall", rainfall_name(form.world.rainfall).to_string(), true, "dry/normal/wet"),
        field("Season length", shown(form, F_SEASON, format!("{} days", form.season_days)), true, "30 - 180 days"),
    ];
    // The world rows are drawn in Tab order starting at focus 0, so the array
    // index is the focus index.
    for (i, (label, value, adjustable, hint)) in wf.iter().enumerate() {
        draw_field(f, inner, row, label.as_str(), value.as_str(), *adjustable, hint, i == form.focus);
        row += 1;
    }
    row += 1;
    row
}

/// The initial-species roster with counts and population shares.
fn draw_species_section(f: &mut Frame<'_>, inner: Rect, form: &WorldGenForm, row: u16) -> u16 {
    let mut row = row;
    panel::section(f, inner, row, "Initial species");
    row += 1;
    util::line(f, inner, row, Line::from(vec![
        Span::styled("   species    kind       count   ", theme::dim_text()),
        Span::styled("share of starting population", theme::dim_text()),
    ]));
    row += 1;
    let total: u32 = form.counts.iter().sum();
    for id in form.species.ids() {
        let i = id.index();
        let focused = form.focus == F_SPECIES + i;
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
        buf.set_stringn(inner.x + 1, y, format!("{} ", form.species.adult_glyph(id)), 2, Style::default().fg(form.species.color(id)).bg(bg).add_modifier(Modifier::BOLD));
        buf.set_stringn(inner.x + 3, y, format!("{:<10}", form.species.plural(id)), 10, base);
        let kind = if form.species.kind(id) == crate::sim::Kind::Prey { "prey" } else { "predator" };
        buf.set_stringn(inner.x + 13, y, format!("{kind:<9}"), 9, Style::default().fg(theme::DIM).bg(bg));
        let arrows = if focused { Style::default().fg(theme::KEY).bg(bg).add_modifier(Modifier::BOLD) } else { Style::default().fg(theme::DIM).bg(bg) };
        buf.set_stringn(inner.x + 22, y, glyphs::REWIND.to_string(), 1, arrows);
        buf.set_stringn(inner.x + 23, y, format!("{:>5} ", shown(form, F_SPECIES + i, form.counts[i].to_string())), 6, base);
        buf.set_stringn(inner.x + 29, y, glyphs::PLAY.to_string(), 1, arrows);
        let share = crate::cast!(form.counts[i] => f32) / crate::cast!(total.max(1) => f32);
        bars::bar(buf, inner.x + 33, y, 22, share, form.species.color(id));
        buf.set_stringn(inner.x + 56, y, format!("{:>3}%", crate::cast!((share * 100.0).round() => u32)), 4, Style::default().fg(theme::TEXT).bg(bg));
        row += 1;
    }
    util::line(f, inner, row, Line::from(vec![
        Span::styled(format!("   total {}   prey {}   predators {}", total, kind_total(form, crate::sim::Kind::Prey), kind_total(form, crate::sim::Kind::Predator)), theme::dim_text()),
    ]));
    row += 2;
    row
}

/// The evolution-tuning fields.
fn draw_evolution_section(f: &mut Frame<'_>, inner: Rect, form: &WorldGenForm, row: u16) -> u16 {
    let mut row = row;
    let tail = form.tail();
    panel::section(f, inner, row, "Evolution");
    row += 1;
    let evo = [
        field("Mutation rate", shown(form, tail + T_MUTATION_RATE, format!("{:.2}", form.genetics.mutation_rate)), true, "per trait/birth"),
        field("Mutation strength", shown(form, tail + T_MUTATION_STRENGTH, format!("{:.2}", form.genetics.mutation_strength)), true, "mutation sd"),
        field("Predation difficulty", difficulty_name(form.predation.difficulty).to_string(), true, "easy/norm/hard"),
        field("Regrowth rate", shown(form, tail + T_REGROWTH, format!("{:.1}", form.regrowth_rate)), true, "veg multiplier"),
    ];
    for (i, (label, value, adjustable, hint)) in evo.iter().enumerate() {
        draw_field(f, inner, row, label.as_str(), value.as_str(), *adjustable, hint, form.focus == tail + T_MUTATION_RATE + i);
        row += 1;
    }
    util::line(f, inner, row, Line::from(vec![
        Span::styled(format!(" {} ", glyphs::NOTE), theme::dim_text()),
        Span::styled("Higher mutation strength speeds adaptation but raises the", theme::dim_text()),
    ]));
    util::line(f, inner, row + 1, Line::from(Span::styled("   chance of unviable offspring.", theme::dim_text())));
    row += 3;
    row
}

/// The preset list.
fn draw_presets_section(f: &mut Frame<'_>, inner: Rect, form: &WorldGenForm, row: u16) {
    let mut row = row;
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
        let focused = form.focus == form.tail() + T_PRESETS + i;
        util::line(f, inner, row, Line::from(vec![
            Span::styled(format!(" {} ", glyphs::DOT), if focused { theme::label() } else { theme::dim_text() }),
            Span::styled(format!("{name:<16}"), if focused { theme::title() } else { theme::text() }),
            Span::styled(*desc, theme::dim_text()),
        ]));
        row += 1;
    }
}

/// The action buttons pinned to the bottom of the panel (a fourth, the species
/// designer, only while that feature is on).
fn draw_form_buttons(f: &mut Frame<'_>, inner: Rect, form: &WorldGenForm) {
    // Buttons pinned to the bottom.
    let brow = inner.height - 1;
    let y = inner.y + brow;
    let buf = f.buffer_mut();
    let tail = form.tail();
    let mut buttons: Vec<(&str, bool, usize)> = vec![("[ Generate ]", true, tail + T_GENERATE), ("[ Randomize ]", false, tail + T_RANDOMIZE), ("[ Back ]", false, tail + T_BACK)];
    if form.designer {
        buttons.push(("[ Design species ]", false, tail + T_DESIGN));
    }
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
        x += crate::cast!(label.len() => u16) + 3;
    }
}


/// How far Tab moves the focus (`BackTab` wraps backwards).
fn field_step(form: &WorldGenForm, code: KeyCode) -> usize {
    if code == KeyCode::BackTab {
        form.field_count() - 1
    } else {
        1
    }
}

/// Consume a key while a numeric field is in typed-entry mode.
fn handle_typed_entry(form: &mut WorldGenForm, code: KeyCode) {
    match code {
        KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
            if let Some(buf) = form.edit.as_mut().filter(|b| b.len() < 9) {
                buf.push(c);
            }
        }
        KeyCode::Backspace => {
            if let Some(buf) = form.edit.as_mut() {
                buf.pop();
            }
        }
        KeyCode::Enter => {
            let focus = form.focus;
            if let Some(text) = form.edit.take() {
                commit_edit(form, focus, &text);
            }
        }
        KeyCode::Tab | KeyCode::BackTab => {
            let focus = form.focus;
            if let Some(text) = form.edit.take() {
                commit_edit(form, focus, &text);
            }
            form.focus = (form.focus + field_step(form, code)) % form.field_count();
        }
        KeyCode::Esc => form.edit = None,
        _ => {}
    }
}

/// The name (0) and seed (1) text fields: the first keystroke replaces the
/// placeholder, then `common::edit_text` does the typing.
fn handle_text_field(form: &mut WorldGenForm, focus: usize, code: KeyCode) -> Action {
    let is_name = focus == 0;
    let max = if is_name { 23 } else { 20 };
    match code {
        KeyCode::Char(c) if !c.is_control() => {
            if !form.text_edited {
                form.text_edited = true;
                form.field_mut(is_name).clear();
            }
            crate::ui::screens::common::edit_text(form.field_mut(is_name), code, max);
        }
        KeyCode::Backspace => {
            crate::ui::screens::common::edit_text(form.field_mut(is_name), code, max);
            form.text_edited = true;
        }
        _ => return Action::None,
    }
    if !is_name {
        form.dirty = true;
    }
    Action::None
}

/// Left/Right adjust the focused field; Enter generates.
fn handle_adjust(form: &mut WorldGenForm, focus: usize, code: KeyCode, app: &mut AppState) -> Action {
    match code {
        KeyCode::Left | KeyCode::Right => {
            let dir: i32 = if code == KeyCode::Right { 1 } else { -1 };
            adjust(form, focus, dir);
            form.dirty = true;
            Action::None
        }
        KeyCode::Enter => generate(form, app),
        _ => Action::None,
    }
}

fn kind_total(form: &WorldGenForm, kind: crate::sim::Kind) -> u32 {
    form.species.ids().filter(|&id| form.species.kind(id) == kind).map(|id| form.counts[id.index()]).sum()
}

const fn rainfall_name(r: Rainfall) -> &'static str {
    match r {
        Rainfall::Dry => "dry",
        Rainfall::Normal => "normal",
        Rainfall::Wet => "wet",
    }
}
const fn difficulty_name(d: Difficulty) -> &'static str {
    match d {
        Difficulty::Easy => "easy",
        Difficulty::Normal => "normal",
        Difficulty::Hard => "hard",
    }
}

// ---------------------------------------------------------------- preview
