//! S17: Hunt Watch (S17a with the details panel, S17b lanes only).
//!
//! One row per predator that is hunting or has just failed: a hunter column,
//! the chase ribbon, a prey column, and below them the details of the selected
//! row. Rows belong to predators and never shift; pinned rows sort first. The
//! facts come from `sim.hunts` and the creatures; see
//! `docs/screens/s17-hunt-watch.md`.

use std::cell::RefCell;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::Frame;

use crate::sim::{CreatureId, Sim};
use crate::ui::app::AppState;
use crate::ui::screens::s03_inspector::Inspector;
use crate::ui::screens::s16_dynasties::{Pin, MAX_PINS};
use crate::ui::screens::{Action, Screen};
use crate::widgets::{Component, Kind, Panel, StatusBar, Text};
use crate::{glyphs, theme};

use model::{HuntView, Status};

mod beats;
mod details;
mod lane;
mod model;
#[cfg(test)]
mod tests;

/// Rows per lane.
pub(crate) const LANE_H: u16 = 5;
/// Lane slots kept; the visible window is five with the details panel, eight without.
pub(crate) const MAX_LANES: usize = 8;
const LANES_WITH_DETAILS: usize = 5;
/// Rows the details panel needs under the lanes: its divider and at least eight lines.
const DETAILS_MIN: u16 = 9;
/// Ticks a dead hunter's row stays unless pinned.
pub(crate) const DEAD_LINGER: u64 = 4;
/// Ticks a fed hunter's row stays after `eat_until` unless pinned.
pub(crate) const FED_LINGER: u64 = 4;
// Lane columns, from the panel's inner left edge (screen column 1).
pub(crate) const LANE_W: u16 = 153;
pub(crate) const HUNTER_X: u16 = 2;
pub(crate) const RULE_L: u16 = 38;
pub(crate) const RIBBON_X: u16 = 40;
pub(crate) const RIBBON_W: u16 = 74;
pub(crate) const RULE_R: u16 = 115;
pub(crate) const PREY_X: u16 = 116;
const KEY_LINE: &str = "key: ribbon: the rope marker per chase tick, ESCAPE top, KILL bottom, ┤ the clock out; grey = stalk · ♦ pinned: row held";

/// Which hunter each slot shows. A slot is taken by the first hunter that
/// needs one and kept until that hunter leaves the board.
#[derive(Debug, Default)]
struct Lanes {
    slots: [Option<CreatureId>; MAX_LANES],
}

impl Lanes {
    /// Free the slots of hunters no longer on the board, then give newcomers
    /// (by trace start, then id) the lowest free slots.
    fn refresh(&mut self, views: &[HuntView<'_>]) {
        for s in &mut self.slots {
            if s.is_some_and(|id| !views.iter().any(|v| v.id() == id)) {
                *s = None;
            }
        }
        let mut newcomers: Vec<&HuntView<'_>> = views.iter().filter(|v| !self.slots.contains(&Some(v.id()))).collect();
        newcomers.sort_by_key(|v| (v.trace.start_tick(), v.id()));
        for v in newcomers {
            if let Some(free) = self.slots.iter_mut().find(|s| s.is_none()) {
                *free = Some(v.id());
            }
        }
    }

    /// Display order: pinned occupied slots, the other occupied slots, then the free ones.
    fn order(&self, pins: &[Pin]) -> Vec<Option<CreatureId>> {
        let pinned = self.slots.iter().flatten().copied().filter(|&id| model::is_pinned(pins, id));
        let rest = self.slots.iter().flatten().copied().filter(|&id| !model::is_pinned(pins, id));
        let free = self.slots.iter().filter(|s| s.is_none()).map(|_| None);
        pinned.chain(rest).map(Some).chain(free).collect()
    }

    fn occupied(&self, pins: &[Pin]) -> Vec<CreatureId> {
        self.order(pins).into_iter().flatten().collect()
    }
}

/// The screen: the selected hunter, the details toggle, the lane slots.
#[derive(Debug)]
pub struct HuntWatch {
    /// Selected by hunter id; falls back to the first occupied lane when it has left.
    selected: Option<CreatureId>,
    /// `d`: five lanes with the details panel, eight without.
    details: bool,
    /// A one-key note for the status bar's right side.
    note: Option<&'static str>,
    lanes: RefCell<Lanes>,
}

impl Default for HuntWatch {
    fn default() -> Self {
        Self::new()
    }
}

impl HuntWatch {
    pub fn new() -> Self {
        Self { selected: None, details: true, note: None, lanes: RefCell::default() }
    }

    /// S17b: the details panel hidden, eight lanes.
    pub fn lanes_only() -> Self {
        Self { details: false, ..Self::new() }
    }

    /// The selected hunter among the occupied lanes, else the first of them.
    fn selection(&self, occupied: &[CreatureId]) -> Option<CreatureId> {
        self.selected.filter(|id| occupied.contains(id)).or_else(|| occupied.first().copied())
    }

    fn step(&mut self, occupied: &[CreatureId], forward: bool) {
        let Some(cur) = self.selection(occupied) else { return };
        let i = occupied.iter().position(|&id| id == cur).unwrap_or(0);
        let n = occupied.len();
        let next = if forward { (i + 1) % n } else { (i + n - 1) % n };
        self.selected = occupied.get(next).copied();
    }

    fn toggle_pin(&mut self, id: CreatureId, pins: &mut Vec<Pin>) {
        let pin = Pin::Member(id);
        if let Some(at) = pins.iter().position(|&p| p == pin) {
            pins.remove(at);
        } else if pins.len() < MAX_PINS {
            pins.push(pin);
        } else {
            self.note = Some("4 pinned already");
        }
    }

    /// Lanes that fit: five above the details panel, else as many as the height allows.
    fn lane_cap(&self, inner_h: u16) -> usize {
        let rows = if self.details { inner_h.saturating_sub(DETAILS_MIN + 1) } else { inner_h.saturating_sub(1) };
        let fit = usize::from(rows.div_euclid(LANE_H));
        fit.min(if self.details { LANES_WITH_DETAILS } else { MAX_LANES })
    }

    fn prey_of(app: &AppState, id: CreatureId) -> Option<CreatureId> {
        let sim = app.sim.as_ref()?;
        let prey = sim.hunts.trace(id)?.prey();
        sim.creatures.get(prey).map(|_| prey)
    }

    fn status_right(&self, app: &AppState, sim: &Sim) -> (String, Color) {
        if let Some(note) = self.note {
            return (format!("{note} "), theme::WARN);
        }
        let sky = if sim.time.is_night() { glyphs::MOON } else { glyphs::SUN };
        let speed = if app.paused { format!("{} paused", glyphs::PAUSE_STR) } else { format!("{} x{}", glyphs::FAST_STR, app.speed()) };
        (format!("{} {sky} · tick {} · {speed} ", sim.time.hour_label(), sim.time.tick), if app.paused { theme::WARN } else { theme::ACCENT })
    }
}

impl Screen for HuntWatch {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        self.note = None;
        let occupied = self.lanes.borrow().occupied(&app.pins);
        let sel = self.selection(&occupied);
        match key.code {
            KeyCode::Esc => return Action::Pop,
            KeyCode::Down => self.step(&occupied, true),
            KeyCode::Up => self.step(&occupied, false),
            KeyCode::Char('d') => self.details = !self.details,
            KeyCode::Char('p') => {
                if let Some(id) = sel {
                    self.selected = Some(id);
                    self.toggle_pin(id, &mut app.pins);
                }
            }
            KeyCode::Enter => {
                let Some(id) = sel else { return Action::None };
                app.follow = Some(id);
                app.follow_death_tick = None;
                app.leave_look();
                return Action::Pop;
            }
            KeyCode::Char('i') => {
                let Some(id) = sel.filter(|&id| app.sim.as_ref().is_some_and(|s| s.creatures.get(id).is_some())) else { return Action::None };
                return Action::Push(Box::new(Inspector::new(id)));
            }
            KeyCode::Char('o') => {
                let Some(prey) = sel.and_then(|id| Self::prey_of(app, id)) else { return Action::None };
                return Action::Push(Box::new(Inspector::new(prey)));
            }
            _ => return Action::Unhandled,
        }
        Action::None
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let Some(sim) = &app.sim else { return };
        if area.height < LANE_H + 4 || area.width < LANE_W + 2 {
            return;
        }
        let views = model::collect(sim, &app.pins);
        let order = {
            let mut lanes = self.lanes.borrow_mut();
            lanes.refresh(&views);
            lanes.order(&app.pins)
        };
        let occupied: Vec<CreatureId> = order.iter().flatten().copied().collect();
        let sel = self.selection(&occupied);
        let body_h = area.height - 1;
        let buf = f.buffer_mut();
        let hunting = views.iter().filter(|v| v.status == Status::Hunting).count();
        let pinned = app.pins.iter().filter(|p| matches!(p, Pin::Member(_))).count();
        let pin_note = if pinned > 0 { format!(" · {} {pinned} pinned", glyphs::DIAMOND) } else { String::new() };
        let info = format!("{hunting} hunting · {} tracked · {}{pin_note}", occupied.len(), clock_info(sim));
        let inner = Panel::new("Chase Lanes · Scoreboard · hunter | ribbon | prey").kind(Kind::Focus).info(info).render(buf, Rect::new(area.x, area.y, area.width, body_h));
        let cap = self.lane_cap(inner.height);
        for (i, slot) in order.iter().take(cap).enumerate() {
            let y0 = inner.y + crate::cast!(i => u16) * LANE_H;
            match slot.and_then(|id| views.iter().find(|v| v.id() == id)) {
                Some(v) => lane::draw(buf, inner, y0, v, Some(v.id()) == sel),
                None => lane::draw_free(buf, inner, y0, i),
            }
        }
        let key_y = inner.bottom() - 1;
        if self.details {
            let dy = inner.y + crate::cast!(cap => u16) * LANE_H;
            details::draw(buf, inner, dy, key_y, sel.and_then(|id| views.iter().find(|v| v.id() == id)));
        }
        let hidden = occupied.len().saturating_sub(cap);
        let key_w = if hidden > 0 { 116 } else { inner.width.saturating_sub(HUNTER_X + 1) };
        put(buf, inner.x + HUNTER_X, key_y, key_w, KEY_LINE, theme::dim_text());
        if hidden > 0 {
            put(buf, inner.x + 119, key_y, 30, &format!("+{hidden} more: hide details"), fg(theme::WARN));
        }
        let keys = [("↑↓", "row"), ("p", "pin"), ("d", "details"), ("Enter", "follow"), ("i/o", "inspect"), ("Space", "pause"), (".", "step"), ("-/=", "speed"), ("Esc", "back")];
        let (right, right_fg) = self.status_right(app, sim);
        StatusBar::new(&keys).right(right).right_color(right_fg).render(buf, Rect::new(area.x, area.y + body_h, area.width, 1));
    }
}

/// `Y4 D213 21:00 ○ night` for the panel's info slot.
fn clock_info(sim: &Sim) -> String {
    let (sky, word) = if sim.time.is_night() { (glyphs::MOON, "night") } else { (glyphs::SUN, "day") };
    format!("Y{} D{} {} {sky} {word}", sim.time.year(), sim.time.day_of_year(), sim.time.hour_label())
}

// ---- shared by the lane and details modules ----

/// One line of text at `(x, y)`, cut at `w` cells; a style without a background keeps the cell's. Returns the cells written.
fn put(buf: &mut Buffer, x: u16, y: u16, w: u16, s: &str, style: Style) -> u16 {
    let area = Rect::new(x, y, w, 1).intersection(*buf.area());
    if area.is_empty() {
        return 0;
    }
    Text::new(s).style(style).render(buf, area);
    crate::cast!(s.chars().count() => u16).min(area.width)
}

fn fg(c: Color) -> Style {
    Style::default().fg(c)
}

fn bold(c: Color) -> Style {
    fg(c).add_modifier(Modifier::BOLD)
}
