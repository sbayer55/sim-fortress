//! S09: the functional world-generation form with a live preview.

use std::cell::RefCell;

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::sim::params::{EcologyParams, GeneticsParams, PredationParams, Roster, TimeParams, WorldParams};
use crate::sim::{Params, Sim, World};
use crate::ui::app::AppState;
use crate::ui::screens::s01_map::WorldMap;
use crate::ui::screens::{Action, Screen};
use crate::widgets::{Component, StatusBar};

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

mod chronicle;
#[cfg(feature = "ai")]
mod designer;
mod form;
mod input;
mod preview;
#[cfg(test)]
mod tests;
use form::form_panel;
use input::{apply_preset, field_step, handle_adjust, handle_text_field, handle_typed_entry, is_numeric_field};
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
            StatusBar::new(&[("0-9", "type value"), ("Enter", "apply"), ("Esc", "cancel")]).right(hint).render(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1));
        } else {
            StatusBar::new(&[("Tab", "next field"), ("←→", "adjust"), ("Space", "type value"), ("Enter", "generate"), ("Esc", "back")]).right(&seed_hint).render(f.buffer_mut(), Rect::new(area.x, status_row, area.width, 1));
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

/// The text shown in a numeric field: the typed buffer with a caret while
/// editing, otherwise the formatted value.
fn shown(form: &WorldGenForm, focus: usize, value: String) -> String {
    match &form.edit {
        Some(buf) if form.focus == focus => format!("{buf}_"),
        _ => value,
    }
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
