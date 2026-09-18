//! S15: Traits & Fates (S15a, the matrix).
//!
//! How each of the twelve genome traits relates to age, population and
//! outcomes for one species over the last 240 days: a trait × outcome
//! correlation matrix, the selected trait followed across age, and a sidebar
//! that explains the selected cell. The numbers come from
//! `sim::stats::outcomes`; see `docs/screens/s15-traits-fates.md`.

use std::cell::RefCell;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::Frame;

use crate::sim::stats::outcomes::{collect_lives, matrix, Crowding, Life, Matrix, Outcome, Tone, N_OUTCOMES, OUTCOMES};
use crate::sim::{Cause, Kind, Roster, Sim, SpeciesId};
use crate::sim::species::N_TRAITS;
use crate::ui::app::AppState;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::SpeciesStyle;
use crate::widgets::{Component, StatusBar, Text};
use crate::theme;

mod ages;
mod matrix_panel;
mod sidebar;

const MAIN_W: u16 = 112;

/// The screen: which species, which cell, which days.
#[derive(Debug, Default)]
pub struct TraitsScreen {
    species: usize,
    trait_ix: usize,
    outcome: usize,
    crowding: Crowding,
    /// The lives and matrices for one species, rebuilt once per sim day.
    cache: RefCell<Option<Analysis>>,
}

/// Everything the three panels read, for all three crowding filters.
#[derive(Debug)]
struct Analysis {
    day: u64,
    species: usize,
    lives: [Vec<Life>; 3],
    matrices: [Matrix; 3],
}

impl Analysis {
    fn build(sim: &Sim, species: usize) -> Self {
        let id = SpeciesId::from_index(species);
        let lives = Crowding::ALL.map(|c| collect_lives(sim, id, c));
        let matrices = [0, 1, 2].map(|i| matrix(lives.get(i).map_or(&[][..], Vec::as_slice)));
        Self { day: sim.time.day_index(), species, lives, matrices }
    }
}

/// What the panels are drawn from: the selection plus the current filter's view.
struct View<'a> {
    sim: &'a Sim,
    species: SpeciesId,
    kind: Kind,
    color: Color,
    trait_ix: usize,
    outcome: usize,
    crowding: Crowding,
    lives: &'a [Life],
    matrix: &'a Matrix,
    all: &'a Analysis,
}

impl View<'_> {
    fn outcome(&self) -> Outcome {
        OUTCOMES.get(self.outcome).copied().unwrap_or(Outcome::Lifespan)
    }

    const fn roster(&self) -> &Roster {
        self.sim.roster()
    }

    fn plural(&self) -> String {
        self.roster().plural(self.species).to_lowercase()
    }

    /// The species' singular name, as an adjective: "vole lives".
    fn singular(&self) -> String {
        self.roster().name(self.species).to_lowercase()
    }
}

impl TraitsScreen {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open on a given species (roster index).
    pub fn for_species(species: usize) -> Self {
        Self { species, ..Self::default() }
    }

    /// Species picked with `1`–`9` (roster order, first nine only).
    const fn species_keys(n: usize) -> usize {
        if n < 9 {
            n
        } else {
            9
        }
    }
}

const fn crowd_index(c: Crowding) -> usize {
    match c {
        Crowding::All => 0,
        Crowding::Crowded => 1,
        Crowding::Sparse => 2,
    }
}

impl Screen for TraitsScreen {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        let n_species = app.sim.as_ref().map_or(0, |s| s.roster().len());
        match key.code {
            KeyCode::Up => self.trait_ix = (self.trait_ix + N_TRAITS - 1) % N_TRAITS,
            KeyCode::Down => self.trait_ix = (self.trait_ix + 1) % N_TRAITS,
            KeyCode::Left => self.outcome = (self.outcome + N_OUTCOMES - 1) % N_OUTCOMES,
            KeyCode::Right => self.outcome = (self.outcome + 1) % N_OUTCOMES,
            KeyCode::Char('c') => self.crowding = self.crowding.next(),
            KeyCode::Char(d @ '1'..='9') => {
                let i = crate::cast!(d.to_digit(10).unwrap_or(1) => usize) - 1;
                if i < Self::species_keys(n_species) {
                    self.species = i;
                }
            }
            KeyCode::Esc => return Action::Pop,
            _ => return Action::Unhandled,
        }
        Action::None
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let Some(sim) = &app.sim else { return };
        if area.height < 2 || area.width <= MAIN_W {
            return;
        }
        let species = self.species.min(sim.roster().len().saturating_sub(1));
        let stale = self.cache.borrow().as_ref().is_none_or(|a| a.day != sim.time.day_index() || a.species != species);
        if stale {
            *self.cache.borrow_mut() = Some(Analysis::build(sim, species));
        }
        let cache = self.cache.borrow();
        let Some(all) = cache.as_ref() else { return };
        let ci = crowd_index(self.crowding);
        let id = SpeciesId::from_index(species);
        let view = View {
            sim,
            species: id,
            kind: sim.roster().kind(id),
            color: sim.roster().color(id),
            trait_ix: self.trait_ix,
            outcome: self.outcome,
            crowding: self.crowding,
            lives: all.lives.get(ci).map_or(&[][..], Vec::as_slice),
            matrix: all.matrices.get(ci).unwrap_or(&all.matrices[0]),
            all,
        };
        let body_h = area.height - 1;
        let buf = f.buffer_mut();
        matrix_panel::draw(buf, Rect::new(area.x, area.y, MAIN_W, body_h), &view);
        sidebar::draw(buf, Rect::new(area.x + MAIN_W, area.y, area.width - MAIN_W, body_h), &view);
        let range = format!("1-{}", Self::species_keys(sim.roster().len()).max(1));
        let keys = [("↑↓", "trait"), ("←→", "outcome"), (range.as_str(), "species"), ("c", "crowding"), ("Esc", "back")];
        let name = sim.roster().name(id);
        StatusBar::new(&keys)
            .right(format!("{} · {} ", capital(name), self.crowding.label()))
            .render(buf, Rect::new(area.x, area.y + body_h, area.width, 1));
    }
}

// ---- shared by the panel modules ----

/// One line of text at `(x, y)`, cut at `w` cells; `style` without a background keeps the cell's.
fn put(buf: &mut Buffer, x: u16, y: u16, w: u16, s: &str, style: Style) {
    let area = Rect::new(x, y, w, 1).intersection(*buf.area());
    if !area.is_empty() {
        Text::new(s).style(style).render(buf, area);
    }
}

fn fg(c: Color) -> Style {
    Style::default().fg(c)
}

fn bold(c: Color) -> Style {
    fg(c).add_modifier(Modifier::BOLD)
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// `+.42`, `-.05`, `+1.0`: a signed correlation in four cells.
fn signed(r: f32) -> String {
    let sign = if r < 0.0 { '-' } else { '+' };
    let a = r.abs();
    if a >= 0.995 {
        format!("{sign}1.0")
    } else {
        format!("{sign}.{:02}", crate::cast!((a * 100.0).round() => u32))
    }
}

/// `.55`: an unsigned trait value in three cells.
fn frac(v: f32) -> String {
    if v >= 0.995 {
        "1.0".to_string()
    } else {
        format!(".{:02}", crate::cast!((v.max(0.0) * 100.0).round() => u32))
    }
}

/// `85d` under a thousand days, else `2.7y`.
fn days(d: f32) -> String {
    if d < 999.5 {
        format!("{}d", crate::cast!(d.max(0.0).round() => u32))
    } else {
        format!("{:.1}y", d / 365.0)
    }
}

/// How strong a correlation reads, as the mockup words it.
fn strength(r: f32) -> &'static str {
    match r.abs() {
        a if a < 0.1 => "no link",
        a if a < 0.2 => "weak",
        a if a < 0.35 => "moderate",
        _ => "strong",
    }
}

/// 0..=1: how much colour a cell gets (|r| of .05 is none, .45 is full).
fn intensity(r: f32) -> f32 {
    ((r.abs() - 0.05) / 0.4).clamp(0.0, 1.0)
}

/// `Some(true)` when the trait helps (more of a good outcome, less of a bad
/// one), `Some(false)` when it hurts, `None` for a neutral outcome (old age).
const fn helps(r: f32, o: Outcome) -> Option<bool> {
    match o.tone() {
        Tone::Good => Some(r > 0.0),
        Tone::Bad => Some(r < 0.0),
        Tone::Neutral => None,
    }
}

/// `GOOD` when the trait helps, `BAD` when it hurts, `TEXT` for a neutral outcome.
const fn effect_color(r: f32, o: Outcome) -> Color {
    match helps(r, o) {
        Some(true) => theme::GOOD,
        Some(false) => theme::BAD,
        None => theme::TEXT,
    }
}

/// A cause's letter and colour, shared by the age strip, key and sidebar.
const fn cause_mark(c: Cause) -> (char, Color) {
    match c {
        Cause::Starved => ('s', theme::WARN),
        Cause::Thirst => ('t', theme::INFO),
        Cause::Age => ('a', theme::ROCK_FG),
        Cause::Predation => ('p', theme::BAD),
        Cause::Disease => ('d', theme::SICK),
        Cause::Injury => ('i', theme::MAGENTA),
    }
}

/// Causes in key order.
const CAUSES: [Cause; 6] = [Cause::Starved, Cause::Thirst, Cause::Age, Cause::Predation, Cause::Disease, Cause::Injury];

/// Low / mid / high third colours: the species colour darkened, as is, and lightened.
fn third_color(species: Color, k: usize) -> Color {
    match k {
        0 => theme::dim(species, 0.42),
        1 => species,
        _ => theme::lerp(species, theme::TEXT_BRIGHT, 0.55),
    }
}

#[cfg(test)]
mod tests;
