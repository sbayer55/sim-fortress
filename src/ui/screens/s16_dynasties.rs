//! S16: Top Dynasties (S16a, the race chart).
//!
//! One region at a time, its four best dynasties by kills, a banner for the
//! selected line with a portrait and six stats against the valley mean, six
//! cumulative race charts against the region's other lines, the living
//! members, a Top-member sidebar and a Watch strip of pinned lines and animals.
//! The numbers come from `sim::lineage::dynasties`; see
//! `docs/screens/s16-top-dynasties.md`.

use std::cell::{Cell, RefCell};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::Frame;

use crate::sim::lineage::dynasties::{DynastyMember, DynastyView, Tally, YearRow};
use crate::sim::{CreatureId, Sex, Sim, SpeciesId};
use crate::ui::app::AppState;
use crate::ui::screens::s08_lineage::LineageScreen;
use crate::ui::screens::{Action, Screen};
use crate::ui::style::SpeciesStyle;
use crate::widgets::{Component, Kind, Panel, StatusBar, Text};
use crate::{glyphs, theme};

use rank::Ranked;

mod banner;
mod header;
mod members;
mod portraits;
mod race;
mod rank;
mod sidebar;
mod watch;
#[cfg(test)]
mod tests;

const MAIN_W: u16 = 112;
const WATCH_H: u16 = 3;
/// The ninth stop of the region cycle: every region at once.
const ALL: usize = crate::sim::world::REGION_COUNT;
/// Rows of the main panel that are not member rows.
const FIXED_ROWS: u16 = 34;
const MAX_MEMBER_ROWS: u16 = 5;
/// Pins the Watch strip holds.
pub const MAX_PINS: usize = 4;

/// A pinned Watch item; session only, on `AppState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pin {
    Dynasty(CreatureId),
    Member(CreatureId),
}

/// Which list the cursor is in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Focus {
    #[default]
    Dynasties,
    Members,
}

/// The six ranked stats, in column order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stat {
    Kills,
    Terr,
    Young,
    Surv,
    Age,
    Muts,
}

impl Stat {
    const ALL: [Self; 6] = [Self::Kills, Self::Terr, Self::Young, Self::Surv, Self::Age, Self::Muts];

    const fn index(self) -> usize {
        match self {
            Self::Kills => 0,
            Self::Terr => 1,
            Self::Young => 2,
            Self::Surv => 3,
            Self::Age => 4,
            Self::Muts => 5,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Kills => "kills",
            Self::Terr => "territory",
            Self::Young => "young",
            Self::Surv => "survival",
            Self::Age => "age",
            Self::Muts => "mutations",
        }
    }

    /// The value of a tally; age in days.
    const fn of(self, t: &Tally) -> f32 {
        let v = match self {
            Self::Kills => t.kills,
            Self::Terr => t.terr,
            Self::Young => t.young,
            Self::Surv => t.surv,
            Self::Age => t.age,
            Self::Muts => t.muts,
        };
        crate::cast!(v => f32)
    }

    /// `6y` for an age in days, else the number.
    fn fmt(self, v: f32, year_days: u32) -> String {
        if self == Self::Age {
            format!("{}y", (v / crate::cast!(year_days.max(1) => f32)).floor())
        } else {
            format!("{v:.0}")
        }
    }
}

/// The screen: region, filter, focus, and the selection by identity.
#[derive(Debug, Default)]
pub struct DynastiesScreen {
    region: usize,
    species: Option<SpeciesId>,
    focus: Focus,
    /// The selected line's root; sticky when the ranking shifts.
    sel_dynasty: Option<CreatureId>,
    sel_member: Option<CreatureId>,
    /// Member rows the last draw showed, so `↑↓` stay on visible rows.
    shown_members: Cell<usize>,
    /// The rankings, rebuilt once per sim day.
    cache: RefCell<Option<Ranked>>,
}

/// What every panel is drawn from.
struct View<'a> {
    sim: &'a Sim,
    ranked: &'a Ranked,
    region: usize,
    species: Option<SpeciesId>,
    focus: Focus,
    /// Indices into `ranked.dynasties` for the region and filter, in rank order.
    rows: Vec<usize>,
    /// The selected line, as an index into `ranked.dynasties`.
    sel: Option<usize>,
    /// The selected member's index in the selected line's members.
    member_ix: usize,
    pins: &'a [Pin],
    year_days: u32,
}

impl View<'_> {
    fn dynasty(&self) -> Option<&DynastyView> {
        self.sel.and_then(|i| self.ranked.dynasties.get(i))
    }

    fn standing(&self) -> Option<&rank::Standing> {
        self.sel.and_then(|i| self.ranked.standing.get(i))
    }

    /// The sidebar's subject: the selected member, else the carrier.
    fn member(&self) -> Option<&DynastyMember> {
        let d = self.dynasty()?;
        d.members.get(self.member_ix).or_else(|| d.members.first())
    }

    fn color(&self, sp: SpeciesId) -> Color {
        self.sim.roster().color(sp)
    }

    fn region_name(&self) -> String {
        region_name(self.sim, self.region)
    }

    /// The other lines of the list, best first, for the race charts.
    fn rivals(&self) -> Vec<usize> {
        self.rows.iter().copied().filter(|&i| Some(i) != self.sel).take(3).collect()
    }

    fn is_pinned(&self, pin: Pin) -> bool {
        self.pins.contains(&pin)
    }
}

impl DynastiesScreen {
    pub fn new() -> Self {
        Self::default()
    }

    fn ranked<'a>(&'a self, sim: &Sim) -> std::cell::Ref<'a, Option<Ranked>> {
        let stale = self.cache.borrow().as_ref().is_none_or(|r| r.day != sim.time.day_index());
        if stale {
            *self.cache.borrow_mut() = Some(Ranked::build(sim));
        }
        self.cache.borrow()
    }

    /// The listed lines for the region and filter, best first.
    fn rows(&self, ranked: &Ranked) -> Vec<usize> {
        ranked
            .dynasties
            .iter()
            .enumerate()
            .filter(|(_, d)| (self.region == ALL || d.region.is_some_and(|r| usize::from(r) == self.region)) && self.species.is_none_or(|s| s == d.species))
            .map(|(i, _)| i)
            .take(4)
            .collect()
    }

    /// The selection as a row index: the sticky root when listed, else rank 1.
    fn sel_index(&self, ranked: &Ranked, rows: &[usize]) -> Option<usize> {
        rows.iter().copied().find(|&i| ranked.dynasties.get(i).map(|d| d.root) == self.sel_dynasty).or_else(|| rows.first().copied())
    }

    fn member_index(&self, d: &DynastyView) -> usize {
        d.members.iter().position(|m| Some(m.id) == self.sel_member).unwrap_or(0)
    }

    const fn move_region(&mut self, delta: usize) {
        self.region = (self.region + delta) % (ALL + 1);
        self.sel_member = None;
    }

    fn move_cursor(&mut self, ranked: &Ranked, down: bool) {
        let rows = self.rows(ranked);
        match self.focus {
            Focus::Dynasties => {
                let Some(cur) = self.sel_index(ranked, &rows).and_then(|i| rows.iter().position(|&r| r == i)) else { return };
                let n = rows.len();
                let next = if down { (cur + 1) % n } else { (cur + n - 1) % n };
                self.sel_dynasty = rows.get(next).and_then(|&i| ranked.dynasties.get(i)).map(|d| d.root);
                self.sel_member = None;
            }
            Focus::Members => {
                let Some(d) = self.sel_index(ranked, &rows).and_then(|i| ranked.dynasties.get(i)) else { return };
                let n = d.members.len().min(self.shown_members.get().max(1));
                if n == 0 {
                    return;
                }
                let cur = self.member_index(d);
                let next = if down { (cur + 1) % n } else { (cur + n - 1) % n };
                self.sel_member = d.members.get(next).map(|m| m.id);
            }
        }
    }

    fn cycle_species(&mut self, sim: &Sim) {
        let ids: Vec<SpeciesId> = sim.roster().predator_ids().collect();
        self.species = match self.species {
            None => ids.first().copied(),
            Some(cur) => ids.iter().position(|&s| s == cur).and_then(|p| ids.get(p + 1)).copied(),
        };
        self.sel_member = None;
    }

    /// The focused item as a pin.
    fn focused_pin(&self, ranked: &Ranked) -> Option<Pin> {
        let rows = self.rows(ranked);
        let d = self.sel_index(ranked, &rows).and_then(|i| ranked.dynasties.get(i))?;
        match self.focus {
            Focus::Dynasties => Some(Pin::Dynasty(d.root)),
            Focus::Members => d.members.get(self.member_index(d)).map(|m| Pin::Member(m.id)),
        }
    }

    fn toggle_pin(&self, ranked: &Ranked, pins: &mut Vec<Pin>) {
        let Some(pin) = self.focused_pin(ranked) else { return };
        if let Some(at) = pins.iter().position(|&p| p == pin) {
            pins.remove(at);
        } else if pins.len() < MAX_PINS {
            pins.push(pin);
        }
    }

    /// Jump to a pin: its region, its line, and its member when it is an animal.
    fn jump(&mut self, ranked: &Ranked, pin: Pin) {
        let target = match pin {
            Pin::Dynasty(root) => ranked.dynasties.iter().find(|d| d.root == root).map(|d| (d, None)),
            Pin::Member(id) => ranked.dynasties.iter().find(|d| d.members.iter().any(|m| m.id == id)).map(|d| (d, Some(id))),
        };
        let Some((d, member)) = target else { return };
        self.region = d.region.map_or(ALL, usize::from);
        self.species = None;
        self.sel_dynasty = Some(d.root);
        self.sel_member = member;
        self.focus = if member.is_some() { Focus::Members } else { Focus::Dynasties };
    }

    /// The sidebar's subject: the selected member, else the carrier.
    fn subject(&self, ranked: &Ranked) -> Option<CreatureId> {
        let rows = self.rows(ranked);
        let d = self.sel_index(ranked, &rows).and_then(|i| ranked.dynasties.get(i))?;
        d.members.get(self.member_index(d)).or_else(|| d.members.first()).map(|m| m.id)
    }

    fn handle_with_sim(&mut self, code: KeyCode, app: &mut AppState) -> Action {
        let Some(sim) = &app.sim else { return Action::Unhandled };
        let ranked = self.ranked(sim).clone().unwrap_or_else(|| Ranked::build(sim));
        match code {
            KeyCode::Left => self.move_region(ALL),
            KeyCode::Right => self.move_region(1),
            KeyCode::Up => self.move_cursor(&ranked, false),
            KeyCode::Down => self.move_cursor(&ranked, true),
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Dynasties => Focus::Members,
                    Focus::Members => Focus::Dynasties,
                };
            }
            KeyCode::Char('s') => self.cycle_species(sim),
            KeyCode::Char('p') => self.toggle_pin(&ranked, &mut app.pins),
            KeyCode::Char(d @ '1'..='4') => {
                let i = crate::cast!(d.to_digit(10).unwrap_or(1) => usize) - 1;
                if let Some(&pin) = app.pins.get(i) {
                    self.jump(&ranked, pin);
                }
            }
            KeyCode::Char('f') => {
                let Some(id) = self.subject(&ranked) else { return Action::None };
                app.follow = Some(id);
                app.follow_death_tick = None;
                app.leave_look();
                return Action::Pop;
            }
            KeyCode::Char('l') => {
                let Some(id) = self.subject(&ranked) else { return Action::None };
                return Action::Push(Box::new(LineageScreen::new(id)));
            }
            _ => return Action::Unhandled,
        }
        Action::None
    }
}

impl Screen for DynastiesScreen {
    fn opaque(&self) -> bool {
        true
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut AppState) -> Action {
        if key.code == KeyCode::Esc {
            return Action::Pop;
        }
        self.handle_with_sim(key.code, app)
    }

    fn render(&self, app: &AppState, f: &mut Frame<'_>, area: Rect) {
        let Some(sim) = &app.sim else { return };
        if area.height < FIXED_ROWS + WATCH_H + 3 || area.width <= MAIN_W {
            return;
        }
        let cache = self.ranked(sim);
        let Some(ranked) = cache.as_ref() else { return };
        let rows = self.rows(ranked);
        let sel = self.sel_index(ranked, &rows);
        let member_ix = sel.and_then(|i| ranked.dynasties.get(i)).map_or(0, |d| self.member_index(d));
        let view = View { sim, ranked, region: self.region, species: self.species, focus: self.focus, rows, sel, member_ix, pins: &app.pins, year_days: 4 * sim.time.season_days };
        let body_h = area.height - 1;
        let buf = f.buffer_mut();
        watch::draw(buf, Rect::new(area.x, area.y, area.width, WATCH_H), &view);
        let main = Rect::new(area.x, area.y + WATCH_H, MAIN_W, body_h - WATCH_H);
        let info = format!("Year {}, day {} · kills lead the ranking", sim.time.year(), sim.time.day_of_year());
        let inner = Panel::new("Top dynasties by region").kind(Kind::Focus).info(info).render(buf, main);
        let member_rows = inner.height.saturating_sub(FIXED_ROWS).clamp(1, MAX_MEMBER_ROWS);
        self.shown_members.set(usize::from(member_rows));
        let mut y = header::draw(buf, inner, &view);
        if view.dynasty().is_some() {
            y = banner::draw(buf, inner, y, &view);
            y = race::draw(buf, inner, y, &view);
            members::draw(buf, inner, y, member_rows, &view);
        }
        sidebar::draw(buf, Rect::new(area.x + MAIN_W, area.y + WATCH_H, area.width - MAIN_W, body_h - WATCH_H), &view);
        let keys = [("←→", "region"), ("↑↓", "pick"), ("Tab", "members"), ("s", "species"), ("p", "pin"), ("1-4", "watch"), ("f", "follow"), ("l", "lineage"), ("Esc", "back")];
        let line = view.dynasty().map_or_else(String::new, line_name);
        let focus = match self.focus {
            Focus::Dynasties => "dynasties",
            Focus::Members => "members",
        };
        StatusBar::new(&keys).right(format!("{} · {line} · {focus} ", region_short(&view.region_name()))).render(buf, Rect::new(area.x, area.y + body_h, area.width, 1));
    }
}

// ---- shared by the panel modules ----

/// One line of text at `(x, y)`, cut at `w` cells; `style` without a background keeps the cell's.
fn put(buf: &mut Buffer, x: u16, y: u16, w: u16, s: &str, style: Style) {
    let area = Rect::new(x, y, w, 1).intersection(*buf.area());
    if area.is_empty() {
        return;
    }
    Text::new(s).style(style).render(buf, area);
}

fn fg(c: Color) -> Style {
    Style::default().fg(c).bg(theme::PANEL_BG)
}

fn bold(c: Color) -> Style {
    fg(c).add_modifier(Modifier::BOLD)
}

/// `<Founder> line`.
fn line_name(d: &DynastyView) -> String {
    format!("{} line", d.founder.split(' ').next().unwrap_or(&d.founder))
}

/// The region's name, or `All regions` at the ninth stop.
fn region_name(sim: &Sim, region: usize) -> String {
    if region == ALL {
        return "All regions".to_string();
    }
    sim.world.regions.get(region).map_or_else(|| "The Wilds".to_string(), |r| r.0.clone())
}

/// `Northern Taiga` → `N. Taiga`, `Far Eastern Plains` → `F. E. Plains`; `All regions` → `Valley`.
fn region_short(name: &str) -> String {
    if name == "All regions" {
        return "Valley".to_string();
    }
    let words: Vec<&str> = name.split_whitespace().collect();
    let Some((last, init)) = words.split_last() else { return name.to_string() };
    let mut out: Vec<String> = init.iter().filter_map(|w| w.chars().next()).map(|c| format!("{c}.")).collect();
    out.push((*last).to_string());
    out.join(" ")
}

/// `♂` or `♀`.
const fn sex_glyph(sex: Sex) -> char {
    match sex {
        Sex::Male => glyphs::MALE,
        Sex::Female => glyphs::FEMALE,
    }
}

/// `+.07` for a trait delta.
fn signed(d: f32) -> String {
    format!("{d:+.2}").replacen("0.", ".", 1)
}

/// The year a line was founded, from its founder's birth day (negative for founders).
fn founded_year(founded_day: i32, year_days: u32) -> i32 {
    founded_day.max(0).div_euclid(crate::cast!(year_days.max(1) => i32)) + 1
}

/// `1y 41d` from days.
fn age_str(days: u32, year_days: u32) -> String {
    let y = year_days.max(1);
    format!("{}y {}d", days.div_euclid(y), days % y)
}

/// The value of `stat` for the line's race: closed years, then now.
fn race_values(d: &DynastyView, stat: Stat) -> (usize, Vec<f32>) {
    let start = d.years.first().map_or(0, |r: &YearRow| crate::cast!(r.year.saturating_sub(1) => usize));
    let mut vals: Vec<f32> = d.years.iter().map(|r| stat.of(&r.stats)).collect();
    vals.push(stat.of(&d.totals));
    (start, vals)
}
